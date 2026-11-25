//! One person, no sign-in: requests act as the owner, the owner's profile starts empty, the local
//! server's key keeps other programs out, and without a mail server sending a document says so.

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use common::{Auth, TestApp};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn the_owner_is_created_once_and_kept() {
    let app = TestApp::new().await;
    let first = solo_api::services::users::ensure_owner(&app.state.db)
        .await
        .expect("owner");
    let again = solo_api::services::users::ensure_owner(&app.state.db)
        .await
        .expect("owner");
    assert_eq!(first, again);
}

#[tokio::test]
async fn the_profile_starts_empty_and_can_be_named() {
    let app = TestApp::new().await;
    let mut tx = app.state.db.begin().await.expect("database");
    let fresh = solo_api::repositories::users::insert(&mut tx, uuid::Uuid::now_v7())
        .await
        .expect("owner");
    tx.commit().await.expect("commit");
    let owner = common::TestUser { id: fresh.id };
    let me = app.get("/api/v1/users/me", &owner.auth()).await;
    assert_eq!(me.status, StatusCode::OK, "{}", me.body);
    assert_eq!(me.body["first_name"], "", "nobody has typed a name yet");
    assert!(me.body.get("email").is_none(), "there is no account email");
    let preferences = me.body["preferences"].clone();
    let named = app
        .put(
            "/api/v1/users/me",
            &owner.auth(),
            json!({ "first_name": " Aroha ", "last_name": "Ngata", "preferences": preferences }),
        )
        .await;
    assert_eq!(named.status, StatusCode::OK, "{}", named.body);
    assert_eq!(named.body["first_name"], "Aroha");
    let too_long = app
        .put(
            "/api/v1/users/me",
            &owner.auth(),
            json!({ "first_name": "x".repeat(101), "last_name": "", "preferences": preferences }),
        )
        .await;
    assert_eq!(
        too_long.field_error("first_name"),
        Some("Use up to 100 characters.")
    );
}

#[tokio::test]
async fn the_app_acts_as_its_owner_without_signing_in() {
    let app = TestApp::new().await;
    let owner = app.register("owner-app").await;
    let business = app.create_business(&owner, "Owner Studio").await;
    let router = solo_api::app(app.state.clone(), owner.id);
    let response = router
        .oneshot(
            Request::builder()
                .uri("/api/v1/businesses")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = http_body_util::BodyExt::collect(response.into_body())
        .await
        .expect("body")
        .to_bytes();
    let list: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(list[0]["id"], business.to_string());
}

#[tokio::test]
async fn the_local_server_answers_only_with_its_key() {
    let app = TestApp::with(&[("API_KEY", "local-secret")]).await;
    let owner = app.register("keyed").await;
    let router = solo_api::app(app.state.clone(), owner.id);
    let call = |key: Option<&str>, path: &str| {
        let mut builder = Request::builder().method(Method::GET).uri(path);
        if let Some(key) = key {
            builder = builder.header("x-api-key", key);
        }
        router
            .clone()
            .oneshot(builder.body(Body::empty()).expect("request"))
    };
    let missing = call(None, "/api/v1/users/me").await.expect("response");
    assert_eq!(missing.status(), StatusCode::FORBIDDEN);
    let wrong = call(Some("guess"), "/api/v1/users/me")
        .await
        .expect("response");
    assert_eq!(wrong.status(), StatusCode::FORBIDDEN);
    let right = call(Some("local-secret"), "/api/v1/users/me")
        .await
        .expect("response");
    assert_eq!(right.status(), StatusCode::OK);
    let health = call(None, "/health/live").await.expect("response");
    assert_eq!(health.status(), StatusCode::OK, "health checks stay open");
}

#[tokio::test]
async fn without_a_mail_server_sending_a_document_explains_what_to_do_instead() {
    let app = TestApp::with(&[("EMAIL_PROVIDER", "log")]).await;
    let meta = app.get("/api/v1/meta", &Auth::None).await.body;
    assert_eq!(meta["email_enabled"], false);
    let user = app.register("local-send").await;
    let business = app.create_business(&user, "Local Studio").await;
    let base = format!("/api/v1/businesses/{business}");
    let customer = app
        .post(
            &format!("{base}/customers"),
            &user.auth(),
            json!({ "name": "Tūī Media", "email": "accounts@tui.example" }),
        )
        .await;
    let invoice = app
        .post(
            &format!("{base}/invoices"),
            &user.auth(),
            json!({ "customer_id": customer.body["id"], "issue_date": "2026-10-01",
                    "lines": [{ "description": "Design", "quantity": "1", "unit_price": 10_000, "tax_treatment": "standard" }] }),
        )
        .await;
    let id = invoice.body["id"].as_str().expect("id");
    app.post(
        &format!("{base}/invoices/{id}/issue"),
        &user.auth(),
        json!({}),
    )
    .await;
    let sent = app
        .post(
            &format!("{base}/invoices/{id}/send"),
            &user.auth(),
            json!({}),
        )
        .await;
    assert_eq!(sent.status, StatusCode::CONFLICT, "{}", sent.body);
    assert!(
        sent.body["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("download the PDF"),
        "{}",
        sent.body
    );
    // The PDF is still there to send yourself.
    let pdf = app
        .get(&format!("{base}/invoices/{id}/pdf"), &user.auth())
        .await;
    assert_eq!(pdf.status, StatusCode::OK);
}
