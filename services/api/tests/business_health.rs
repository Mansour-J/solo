//! Business health against real records: what a new business is missing, the GST checks, customers
//! who owe money without an email address, expenses without receipts, and isolation between owners.

mod common;

use axum::http::StatusCode;
use common::{TestApp, TestUser};
use serde_json::{Value, json};
use uuid::Uuid;

async fn health(app: &TestApp, user: &TestUser, business: Uuid) -> Value {
    let response = app
        .get(
            &format!("/api/v1/businesses/{business}/health"),
            &user.auth(),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    response.body
}

fn check<'a>(health: &'a Value, key: &str) -> &'a Value {
    health["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|c| c["key"] == key)
        .unwrap_or_else(|| panic!("no {key} check"))
}

async fn post(app: &TestApp, user: &TestUser, business: Uuid, rest: &str, body: Value) -> Value {
    let response = app
        .post(
            &format!("/api/v1/businesses/{business}{rest}"),
            &user.auth(),
            body,
        )
        .await;
    assert!(response.status.is_success(), "{rest}: {}", response.body);
    response.body
}

#[tokio::test]
async fn a_new_business_is_told_what_its_invoices_are_missing() {
    let app = TestApp::new().await;
    let user = app.register("health").await;
    let business = app.create_business(&user, "Health Ltd").await;
    let h = health(&app, &user, business).await;

    assert!(h["checked_at"].is_string());
    assert_eq!(check(&h, "address")["status"], "recommendation");
    assert_eq!(check(&h, "address")["link"], "/settings/business");
    assert_eq!(check(&h, "contact")["status"], "recommendation");
    let pay = check(&h, "payment-details");
    assert_eq!(pay["status"], "warning");
    assert_eq!(pay["area"], "getting_paid");
    assert_eq!(pay["link"], "/settings/invoicing");
    assert!(pay["why"].as_str().is_some_and(|w| !w.is_empty()));
    assert_eq!(check(&h, "gst-threshold")["status"], "passed");
    assert_eq!(check(&h, "gst-number")["status"], "not_applicable");
    assert_eq!(check(&h, "customer-emails")["status"], "not_applicable");
    assert_eq!(check(&h, "receipts")["status"], "passed");
    assert!(check(&h, "receipts")["link"].is_null());

    // Payment instructions fix the warning.
    let path = format!("/api/v1/businesses/{business}/settings");
    let mut settings = app.get(&path, &user.auth()).await.body;
    settings["payment_instructions"] = json!("Pay to 12-3456-7890123-00");
    let saved = app.put(&path, &user.auth(), settings).await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    let h = health(&app, &user, business).await;
    assert_eq!(check(&h, "payment-details")["status"], "passed");
    assert!(check(&h, "payment-details")["action"].is_null());
}

#[tokio::test]
async fn gst_customers_and_receipts() {
    let app = TestApp::new().await;
    let user = app.register("health-gst").await;
    let created = app
        .post(
            "/api/v1/businesses",
            &user.auth(),
            json!({ "name": "Kererū Test", "email": "hello@example.com",
                    "address": { "line1": "1 Test Street", "city": "Wellington", "country": "nz" },
                    "gst_registered": true, "complete_onboarding": true }),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let business: Uuid = created.body["id"]
        .as_str()
        .expect("id")
        .parse()
        .expect("uuid");
    let today = created.body["today"].as_str().expect("today").to_owned();

    // Registered without a GST number or date.
    let h = health(&app, &user, business).await;
    assert_eq!(check(&h, "gst-number")["status"], "critical");
    assert_eq!(check(&h, "gst-registered-from")["status"], "warning");
    assert_eq!(check(&h, "gst-threshold")["status"], "not_applicable");
    assert_eq!(check(&h, "address")["status"], "passed");
    assert_eq!(check(&h, "contact")["status"], "passed");

    // What to fix now is also a dashboard notice, linking here.
    let insights = app
        .get(
            &format!("/api/v1/businesses/{business}/insights"),
            &user.auth(),
        )
        .await;
    assert_eq!(insights.status, StatusCode::OK, "{}", insights.body);
    let notice = insights.body["notices"]
        .as_array()
        .expect("notices")
        .iter()
        .find(|n| n["key"] == "health:gst-number")
        .expect("fix-now notice")
        .clone();
    assert_eq!(notice["message"], check(&h, "gst-number")["finding"]);
    assert_eq!(notice["link"], "/settings/health");
    assert_eq!(notice["tone"], "warning");

    // A customer with no email address owes money; one with an address has paid in full.
    let without = post(
        &app,
        &user,
        business,
        "/customers",
        json!({ "name": "No Email Test" }),
    )
    .await;
    let with = post(
        &app,
        &user,
        business,
        "/customers",
        json!({ "name": "Email Test", "email": "pay@example.com" }),
    )
    .await;
    for (customer, paid) in [(&without, false), (&with, true)] {
        let draft = post(&app, &user, business, "/invoices",
            json!({ "customer_id": customer["id"], "issue_date": today, "due_date": today,
                    "lines": [{ "description": "Work", "quantity": "1", "unit_price": 10_000, "tax_treatment": "no_gst" }] }))
            .await;
        let id = draft["id"].as_str().expect("id");
        let issued = post(
            &app,
            &user,
            business,
            &format!("/invoices/{id}/issue"),
            json!({}),
        )
        .await;
        if paid {
            post(&app, &user, business, &format!("/invoices/{id}/payments"),
                json!({ "payment_date": today, "amount": issued["total"], "method": "bank_transfer" })).await;
        }
    }
    let h = health(&app, &user, business).await;
    let customers = check(&h, "customer-emails");
    assert_eq!(customers["status"], "recommendation");
    assert_eq!(
        customers["finding"],
        "1 customer who owes you has no email address."
    );

    // Expenses without receipts: a $115 GST claim and a $20 one.
    let categories = app
        .get(
            &format!("/api/v1/businesses/{business}/expense-categories"),
            &user.auth(),
        )
        .await
        .body;
    let category = categories[0]["id"].clone();
    for amount in [11_500, 2_000] {
        post(&app, &user, business, "/expenses",
            json!({ "expense_date": today, "supplier": "Test Supplies", "category_id": category, "gross_amount": amount,
                    "gst_treatment": "standard", "payment_method": "business_account", "classification": "business" }))
            .await;
    }
    let h = health(&app, &user, business).await;
    let receipts = check(&h, "receipts");
    assert_eq!(receipts["status"], "warning");
    assert_eq!(
        receipts["finding"],
        "2 expenses this financial year have no receipt, including one that claims GST over $50."
    );
    assert_eq!(receipts["link"], "/expenses?receipt=missing");
}

#[tokio::test]
async fn another_owner_cannot_see_a_business_health() {
    let app = TestApp::new().await;
    let owner = app.register("health-owner").await;
    let intruder = app.register("health-intruder").await;
    let business = app.create_business(&owner, "Private Health Ltd").await;
    let response = app
        .get(
            &format!("/api/v1/businesses/{business}/health"),
            &intruder.auth(),
        )
        .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
}
