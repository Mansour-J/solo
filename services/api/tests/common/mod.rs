//! Integration test harness: the real router, a real PostgreSQL database (migrated by Flyway), and
//! in-memory email and file storage. The app has one owner; each test makes its own (`TestApp::register`)
//! and names it per request, so tests stay isolated and run in parallel against one database.
//!
//! Database: `TEST_DATABASE_URL`, default `postgres://solo_app:change-me-app-password@127.0.0.1:5432/solo_test`
//! (prepare it with `npm run db:test:prepare`).

#![allow(dead_code)]

use axum::Router;
use axum::body::Body;
use axum::extract::Request as AxumRequest;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use solo_api::context::OwnerId;
use solo_api::services::email::{EmailMessage, MemoryEmailSender};
use solo_api::{AppState, Config};
use tower::ServiceExt;
use uuid::Uuid;

/// The test-only header naming the owner a request acts as (the real app attaches one owner).
const OWNER_HEADER: &str = "x-test-owner";

async fn owner_from_header(mut request: AxumRequest, next: Next) -> Response {
    if let Some(id) = request
        .headers()
        .get(OWNER_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
    {
        request.extensions_mut().insert(OwnerId(id));
    }
    next.run(request).await
}

pub fn database_url() -> String {
    std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| {
        "postgres://solo_app:change-me-app-password@127.0.0.1:5432/solo_test".into()
    })
}

pub struct TestApp {
    pub router: Router,
    pub state: AppState,
    pub mailbox: MemoryEmailSender,
}

pub struct TestResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
    pub bytes: Vec<u8>,
}

impl TestResponse {
    pub fn error_code(&self) -> &str {
        self.body["error"]["code"].as_str().unwrap_or_default()
    }

    pub fn field_error(&self, field: &str) -> Option<&str> {
        self.body["error"]["fields"][field].as_str()
    }
}

/// Whom a test request acts as.
#[derive(Clone, Debug)]
pub enum Auth {
    /// No owner (public routes such as health and meta).
    None,
    Owner(Uuid),
}

impl TestApp {
    pub async fn new() -> Self {
        Self::with(&[]).await
    }

    /// An app with configuration overrides, e.g. `[("EMAIL_PROVIDER", "log")]`.
    pub async fn with(overrides: &[(&str, &str)]) -> Self {
        let mut pairs: Vec<(String, String)> = vec![
            ("APP_ENV".into(), "test".into()),
            ("DATABASE_URL".into(), database_url()),
            ("DATABASE_MAX_CONNECTIONS".into(), "5".into()),
            ("EMAIL_PROVIDER".into(), "memory".into()),
            ("STORAGE_BACKEND".into(), "memory".into()),
            ("APP_URL".into(), "http://app.test".into()),
        ];
        for (key, value) in overrides {
            pairs.retain(|(k, _)| k != key);
            pairs.push(((*key).into(), (*value).into()));
        }
        let config = Config::from_lookup(move |key| {
            pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
        })
        .expect("test configuration is valid");
        let state = AppState::from_config(config)
            .await
            .expect("test database reachable: run `npm run db:up && npm run db:test:prepare` or set TEST_DATABASE_URL");
        // Without the memory provider (e.g. `EMAIL_PROVIDER=log`) nothing is sent, so the mailbox stays empty.
        let mailbox = state.memory_mailbox.clone().unwrap_or_default();
        Self {
            router: solo_api::api::router(state.clone())
                .layer(middleware::from_fn(owner_from_header)),
            state,
            mailbox,
        }
    }

    pub async fn request(
        &self,
        method: Method,
        path: &str,
        auth: &Auth,
        body: Option<Value>,
    ) -> TestResponse {
        self.request_as(method, path, auth, body, "desktop").await
    }

    pub async fn request_as(
        &self,
        method: Method,
        path: &str,
        auth: &Auth,
        body: Option<Value>,
        client: &str,
    ) -> TestResponse {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header("x-client-kind", client);
        builder = match auth {
            Auth::None => builder,
            Auth::Owner(id) => builder.header(OWNER_HEADER, id.to_string()),
        };
        let request = match body {
            Some(body) => builder
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => builder.body(Body::empty()),
        }
        .expect("request builds");
        self.send(request).await
    }

    pub async fn send(&self, request: Request<Body>) -> TestResponse {
        let response = self
            .router
            .clone()
            .oneshot(request)
            .await
            .expect("router responds");
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body reads")
            .to_bytes()
            .to_vec();
        let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        TestResponse {
            status,
            headers,
            body,
            bytes,
        }
    }

    pub async fn get(&self, path: &str, auth: &Auth) -> TestResponse {
        self.request(Method::GET, path, auth, None).await
    }

    pub async fn post(&self, path: &str, auth: &Auth, body: Value) -> TestResponse {
        self.request(Method::POST, path, auth, Some(body)).await
    }

    pub async fn put(&self, path: &str, auth: &Auth, body: Value) -> TestResponse {
        self.request(Method::PUT, path, auth, Some(body)).await
    }

    pub async fn delete(&self, path: &str, auth: &Auth, body: Option<Value>) -> TestResponse {
        self.request(Method::DELETE, path, auth, body).await
    }

    /// Waits for an email and returns it.
    pub async fn email(&self, to: &str, subject_part: &str) -> EmailMessage {
        self.mailbox
            .wait_for(to, subject_part)
            .await
            .unwrap_or_else(|| panic!("expected an email to {to} about {subject_part:?}"))
    }

    /// Makes a new owner, as the app does on first start.
    pub async fn register(&self, _prefix: &str) -> TestUser {
        let mut tx = self.state.db.begin().await.expect("database");
        let user = solo_api::repositories::users::insert(&mut tx, Uuid::now_v7())
            .await
            .expect("owner created");
        let user = solo_api::repositories::users::update_profile(
            &mut tx,
            user.id,
            "Aroha",
            "Test",
            &user.preferences,
        )
        .await
        .expect("owner named");
        tx.commit().await.expect("commit");
        TestUser { id: user.id }
    }

    /// Creates a business for the user and returns its ID.
    pub async fn create_business(&self, user: &TestUser, name: &str) -> Uuid {
        let response = self
            .post(
                "/api/v1/businesses",
                &user.auth(),
                json!({ "name": name, "complete_onboarding": true }),
            )
            .await;
        assert_eq!(
            response.status,
            StatusCode::CREATED,
            "business creation failed: {}",
            response.body
        );
        response.body["id"]
            .as_str()
            .expect("business id")
            .parse()
            .expect("uuid")
    }
}

pub struct TestUser {
    pub id: Uuid,
}

impl TestUser {
    pub fn auth(&self) -> Auth {
        Auth::Owner(self.id)
    }
}
