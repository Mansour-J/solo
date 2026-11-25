//! Email settings (SL-015) and the system status page.

mod common;

use axum::http::StatusCode;
use common::TestApp;
use serde_json::json;

#[tokio::test]
async fn email_settings_are_saved_without_returning_the_password_and_a_failed_test_says_why() {
    let app = TestApp::new().await;
    let user = app.register("email-settings").await;
    let auth = user.auth();

    let empty = app.get("/api/v1/users/me/email-settings", &auth).await;
    assert_eq!(empty.status, StatusCode::OK, "{}", empty.body);
    assert_eq!(empty.body["configured"], false);

    let invalid = app
        .put(
            "/api/v1/users/me/email-settings",
            &auth,
            json!({ "host": "", "port": 0, "security": "starttls", "from_address": "not an email" }),
        )
        .await;
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
    for field in ["host", "port", "from_address"] {
        assert!(
            invalid.field_error(field).is_some(),
            "{field}: {}",
            invalid.body
        );
    }

    // A mail server that isn't there (port 9 on this machine refuses connections).
    let saved = app
        .put(
            "/api/v1/users/me/email-settings",
            &auth,
            json!({ "host": "127.0.0.1", "port": 9, "security": "none", "username": "me", "password": "secret",
                    "from_name": "Test Ltd", "from_address": "Me@Example.com" }),
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    assert_eq!(saved.body["has_password"], true);
    assert_eq!(saved.body["from_address"], "me@example.com");
    assert!(
        saved.body.get("password").is_none(),
        "the password is never returned"
    );

    // Saving again without a password keeps it.
    let kept = app
        .put(
            "/api/v1/users/me/email-settings",
            &auth,
            json!({ "host": "127.0.0.1", "port": 9, "security": "none", "username": "me",
                    "from_address": "me@example.com" }),
        )
        .await;
    assert_eq!(kept.body["has_password"], true);

    let test = app
        .post("/api/v1/users/me/email-settings/test", &auth, json!({}))
        .await;
    assert_eq!(test.status, StatusCode::CONFLICT, "{}", test.body);
    assert!(
        test.body["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("couldn’t be sent"),
        "{}",
        test.body
    );
    let after = app.get("/api/v1/users/me/email-settings", &auth).await;
    assert!(after.body["tested_at"].is_string());
    assert!(
        after.body["test_error"].is_string(),
        "the failure is kept: {}",
        after.body
    );

    let removed = app
        .delete("/api/v1/users/me/email-settings", &auth, None)
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    assert_eq!(
        app.get("/api/v1/users/me/email-settings", &auth).await.body["configured"],
        false
    );
}

#[tokio::test]
async fn system_status_checks_each_part() {
    let app = TestApp::new().await;
    let user = app.register("system-status").await;
    let status = app
        .get("/api/v1/users/me/system-status", &user.auth())
        .await;
    assert_eq!(status.status, StatusCode::OK, "{}", status.body);
    assert!(status.body["schema_version"].as_i64().unwrap_or(0) >= 27);
    let checks = status.body["checks"].as_array().expect("checks");
    let keys: Vec<&str> = checks.iter().filter_map(|c| c["key"].as_str()).collect();
    assert_eq!(keys, ["database", "storage", "documents", "email"]);
    for check in checks {
        // The test server sends email itself (memory), so everything is working.
        assert_eq!(check["status"], "ok", "{check}");
    }
}
