//! Businesses, settings, logo and tenant isolation.

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{Auth, TestApp};
use serde_json::json;

#[tokio::test]
async fn onboarding_creates_a_business_with_nz_defaults() {
    let app = TestApp::new().await;
    let user = app.register("onboard").await;
    let created = app
        .post(
            "/api/v1/businesses",
            &user.auth(),
            json!({
                "name": "Kōwhai Design", "trading_name": "Aroha Test", "business_type": "freelancer",
                "email": "Hello@Kowhai.co.nz", "phone": "+64 21 123 4567",
                "address": { "line1": "1 Queen Street", "city": "Auckland", "postcode": "1010", "country": "nz" },
                "gst_registered": true, "gst_number": "49-091-850", "complete_onboarding": true
            }),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let business = &created.body;
    assert_eq!(business["currency"], "NZD");
    assert_eq!(business["timezone"], "Pacific/Auckland");
    assert_eq!(business["financial_year_start_month"], 4);
    assert_eq!(business["gst_number"], "49091850");
    assert_eq!(business["email"], "hello@kowhai.co.nz");
    assert_eq!(business["address"]["country"], "NZ");
    assert_eq!(business["onboarding_completed"], true);
    assert!(
        business["current_financial_year"]["label"]
            .as_str()
            .is_some_and(|l| l.starts_with("FY"))
    );

    let list = app.get("/api/v1/businesses", &user.auth()).await;
    assert_eq!(list.body.as_array().map(Vec::len), Some(1));

    let id = business["id"].as_str().expect("id");
    let settings = app
        .get(&format!("/api/v1/businesses/{id}/settings"), &user.auth())
        .await;
    assert_eq!(settings.body["invoice_prefix"], "INV-");
    assert_eq!(settings.body["next_invoice_number"], 1);
    assert_eq!(settings.body["default_deductible_percentage"], "100.00");
}

#[tokio::test]
async fn invalid_identifiers_are_rejected_with_field_messages() {
    let app = TestApp::new().await;
    let user = app.register("invalid-biz").await;
    let response = app
        .post(
            "/api/v1/businesses",
            &user.auth(),
            json!({ "name": "", "nzbn": "1234", "gst_registered": true, "gst_number": "136410133", "currency": "XYZ",
                    "financial_year_start_month": 13, "email": "not-an-email", "phone": "call me" }),
        )
        .await;
    assert_eq!(response.status, StatusCode::UNPROCESSABLE_ENTITY);
    for field in [
        "name",
        "nzbn",
        "gst_number",
        "currency",
        "financial_year_start_month",
        "email",
        "phone",
    ] {
        assert!(
            response.field_error(field).is_some(),
            "expected an error for {field}: {}",
            response.body
        );
    }
}

#[tokio::test]
async fn updates_are_audited_with_before_and_after_values() {
    let app = TestApp::new().await;
    let user = app.register("audit").await;
    let id = app.create_business(&user, "Before Ltd").await;
    let updated = app
        .put(
            &format!("/api/v1/businesses/{id}"),
            &user.auth(),
            json!({ "name": "After Ltd" }),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(updated.body["name"], "After Ltd");

    let activity = app
        .get(
            &format!("/api/v1/businesses/{id}/activity?per_page=10"),
            &user.auth(),
        )
        .await;
    assert_eq!(activity.status, StatusCode::OK);
    let entries = activity.body["items"].as_array().expect("items");
    let update = entries
        .iter()
        .find(|e| e["action"] == "business.updated")
        .expect("update entry");
    assert_eq!(update["changes"]["name"]["from"], "Before Ltd");
    assert_eq!(update["changes"]["name"]["to"], "After Ltd");
    assert_eq!(update["actor_name"], "Aroha Test");
    assert!(entries.iter().any(|e| e["action"] == "business.created"));
}

#[tokio::test]
async fn settings_are_validated_and_saved() {
    let app = TestApp::new().await;
    let user = app.register("settings").await;
    let id = app.create_business(&user, "Settings Ltd").await;
    let path = format!("/api/v1/businesses/{id}/settings");
    let mut settings = app.get(&path, &user.auth()).await.body;
    settings["invoice_prefix"] = json!("KD-");
    settings["default_payment_terms_days"] = json!(20);
    settings["payment_instructions"] = json!("Pay to 12-3456-7890123-00");
    settings["default_deductible_percentage"] = json!("50");
    let saved = app.put(&path, &user.auth(), settings.clone()).await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    assert_eq!(saved.body["invoice_prefix"], "KD-");
    assert_eq!(saved.body["default_payment_terms_days"], 20);

    settings["invoice_accent_color"] = json!("red");
    settings["number_padding"] = json!(0);
    let rejected = app.put(&path, &user.auth(), settings).await;
    assert!(rejected.field_error("invoice_accent_color").is_some());
    assert!(rejected.field_error("number_padding").is_some());
}

#[tokio::test]
async fn other_users_cannot_see_or_change_a_business() {
    let app = TestApp::new().await;
    let owner = app.register("owner").await;
    let intruder = app.register("intruder").await;
    let id = app.create_business(&owner, "Private Ltd").await;
    let intruder_auth = intruder.auth();

    for path in [
        format!("/api/v1/businesses/{id}"),
        format!("/api/v1/businesses/{id}/settings"),
        format!("/api/v1/businesses/{id}/activity"),
    ] {
        let response = app.get(&path, &intruder_auth).await;
        assert_eq!(
            response.status,
            StatusCode::NOT_FOUND,
            "{path} must not leak"
        );
    }
    assert_eq!(
        app.put(
            &format!("/api/v1/businesses/{id}"),
            &intruder_auth,
            json!({ "name": "Hacked" })
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.delete(
            &format!("/api/v1/businesses/{id}"),
            &intruder_auth,
            Some(json!({ "confirmation_name": "Private Ltd" }))
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        app.get("/api/v1/businesses", &intruder_auth)
            .await
            .body
            .as_array()
            .map(Vec::len),
        Some(0)
    );
    assert_eq!(
        app.get(&format!("/api/v1/businesses/{id}"), &owner.auth())
            .await
            .body["name"],
        "Private Ltd"
    );
}

#[tokio::test]
async fn row_level_security_blocks_cross_tenant_queries_even_without_application_checks() {
    let app = TestApp::new().await;
    let owner = app.register("rls-owner").await;
    let other = app.register("rls-other").await;
    let private = app.create_business(&owner, "RLS Private").await;
    let others = app.create_business(&other, "RLS Other").await;

    // Acting inside another business, the private business's rows are invisible and unwritable.
    let mut tx = app
        .state
        .db
        .begin_business(other.id, others)
        .await
        .expect("tx");
    let seen: i64 =
        sqlx::query_scalar("SELECT count(*) FROM business_settings WHERE business_id = $1")
            .bind(private)
            .fetch_one(&mut *tx)
            .await
            .expect("query");
    assert_eq!(seen, 0);
    let updated = sqlx::query("UPDATE businesses SET name = 'pwned' WHERE id = $1")
        .bind(private)
        .execute(&mut *tx)
        .await
        .expect("update");
    assert_eq!(updated.rows_affected(), 0);
    let insert = sqlx::query("INSERT INTO audit_logs (business_id, action, entity_type, summary) VALUES ($1, 'x.y', 'x', 'forged')")
        .bind(private)
        .execute(&mut *tx)
        .await;
    assert!(
        insert.is_err(),
        "writing into another tenant violates the policy"
    );
    drop(tx);

    // With no request context at all, tenant tables return nothing.
    let anonymous: i64 = sqlx::query_scalar("SELECT count(*) FROM business_settings")
        .fetch_one(app.state.db.pool())
        .await
        .expect("query");
    assert_eq!(anonymous, 0);
}

fn tiny_png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend_from_slice(&[0; 32]);
    bytes
}

fn multipart(bytes: &[u8], filename: &str) -> (String, Vec<u8>) {
    let boundary = "----test-boundary";
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

#[tokio::test]
async fn logo_upload_accepts_images_only() {
    let app = TestApp::new().await;
    let user = app.register("logo").await;
    let id = app.create_business(&user, "Logo Ltd").await;
    let path = format!("/api/v1/businesses/{id}/logo");
    let upload = |bytes: Vec<u8>, name: &str| {
        let (content_type, body) = multipart(&bytes, name);
        Request::builder()
            .method(Method::PUT)
            .uri(&path)
            .header("x-client-kind", "desktop")
            .header("x-test-owner", user.id.to_string())
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(body))
            .expect("request")
    };

    let svg = app
        .send(upload(
            b"<svg onload=\"alert(1)\"></svg>".to_vec(),
            "logo.png",
        ))
        .await;
    assert_eq!(
        svg.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "content is sniffed, not trusted from the name"
    );

    let ok = app.send(upload(tiny_png(), "logo.png")).await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.body);
    assert_eq!(ok.body["has_logo"], true);

    let fetched = app.get(&path, &user.auth()).await;
    assert_eq!(fetched.status, StatusCode::OK);
    assert_eq!(fetched.headers[header::CONTENT_TYPE], "image/png");
    assert_eq!(fetched.bytes, tiny_png());

    assert_eq!(
        app.delete(&path, &user.auth(), None).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        app.get(&path, &user.auth()).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn deleting_a_business_requires_its_name() {
    let app = TestApp::new().await;
    let user = app.register("delete-biz").await;
    let id = app.create_business(&user, "Doomed Ltd").await;
    let path = format!("/api/v1/businesses/{id}");
    let wrong = app
        .delete(
            &path,
            &user.auth(),
            Some(json!({ "confirmation_name": "doomed" })),
        )
        .await;
    assert!(wrong.field_error("confirmation_name").is_some());
    let deleted = app
        .delete(
            &path,
            &user.auth(),
            Some(json!({ "confirmation_name": "Doomed Ltd" })),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get(&path, &user.auth()).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn reference_data_and_meta_are_public() {
    let app = TestApp::new().await;
    let meta = app.get("/api/v1/meta", &Auth::None).await;
    assert_eq!(meta.status, StatusCode::OK);
    assert!(
        meta.body["brand"]["name"]
            .as_str()
            .is_some_and(|n| !n.is_empty())
    );
    assert_eq!(meta.body["email_enabled"], true);
    let rates = app.get("/api/v1/meta/tax-rates", &Auth::None).await;
    let current = rates
        .body
        .as_array()
        .expect("rates")
        .iter()
        .find(|r| r["effective_to"].is_null())
        .expect("current rate")
        .clone();
    assert_eq!(current["rate"], "0.150000");
    assert_eq!(
        app.get("/health/ready", &Auth::None).await.status,
        StatusCode::OK
    );
    let spec = app.get("/api/openapi.json", &Auth::None).await;
    assert!(spec.body["paths"]["/api/v1/users/me"].is_object());
    assert!(
        spec.body["paths"]["/api/v1/auth/login"].is_null(),
        "there is no sign-in"
    );
}
