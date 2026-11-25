//! Looking up, changing, listing and removing records that the other suites mostly create: expenses,
//! income, invoices, quotes, mileage rates and vehicles, allocation rules, and health checks.

mod common;

use axum::http::StatusCode;
use common::{Auth, TestApp, TestResponse, TestUser};
use serde_json::{Value, json};
use uuid::Uuid;

struct Books {
    app: TestApp,
    user: TestUser,
    business: Uuid,
    customer: String,
}

impl Books {
    async fn new(prefix: &str) -> Self {
        let app = TestApp::new().await;
        let user = app.register(prefix).await;
        let created = app
            .post(
                "/api/v1/businesses",
                &user.auth(),
                json!({ "name": format!("{prefix} Ltd"), "gst_registered": true, "gst_number": "49091850",
                        "gst_registered_from": "2020-01-01", "complete_onboarding": true }),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
        let business = created.body["id"]
            .as_str()
            .expect("id")
            .parse()
            .expect("uuid");
        let mut books = Self {
            app,
            user,
            business,
            customer: String::new(),
        };
        books.customer = books.ok(books
            .post(
                "/customers",
                json!({ "name": "Tūī Media", "email": "accounts@tui.example" }),
            )
            .await)["id"]
            .as_str()
            .expect("id")
            .to_owned();
        books
    }

    fn path(&self, rest: &str) -> String {
        format!("/api/v1/businesses/{}{rest}", self.business)
    }

    async fn get(&self, rest: &str) -> TestResponse {
        self.app.get(&self.path(rest), &self.user.auth()).await
    }

    async fn post(&self, rest: &str, body: Value) -> TestResponse {
        self.app
            .post(&self.path(rest), &self.user.auth(), body)
            .await
    }

    async fn put(&self, rest: &str, body: Value) -> TestResponse {
        self.app
            .put(&self.path(rest), &self.user.auth(), body)
            .await
    }

    async fn delete(&self, rest: &str, body: Option<Value>) -> TestResponse {
        self.app
            .delete(&self.path(rest), &self.user.auth(), body)
            .await
    }

    fn ok(&self, response: TestResponse) -> Value {
        assert!(
            response.status.is_success(),
            "{} {}",
            response.status,
            response.body
        );
        response.body
    }

    async fn category(&self, name: &str) -> String {
        self.ok(self.get("/expense-categories").await)
            .as_array()
            .expect("categories")
            .iter()
            .find(|c| c["name"] == name)
            .and_then(|c| c["id"].as_str())
            .expect("category")
            .to_owned()
    }
}

fn id(value: &Value) -> String {
    value["id"].as_str().expect("id").to_owned()
}

fn lines(unit_price: i64) -> Value {
    json!([{ "description": "Design work", "quantity": "2", "unit_price": unit_price, "tax_treatment": "standard" }])
}

#[tokio::test]
async fn expenses_and_income_can_be_found_changed_and_filtered() {
    let b = Books::new("maint-expenses").await;
    let software = b.category("Software").await;
    let expense = b.ok(
        b.post(
            "/expenses",
            json!({ "expense_date": "2026-04-15", "supplier": "Xero", "category_id": software, "gross_amount": 11_500,
                    "gst_treatment": "standard", "payment_method": "business_account", "classification": "business" }),
        )
        .await,
    );
    let expense_id = id(&expense);
    let fetched = b.ok(b.get(&format!("/expenses/{expense_id}")).await);
    assert_eq!(fetched["supplier"], "Xero");
    let updated = b.ok(
        b.put(
            &format!("/expenses/{expense_id}"),
            json!({ "expense_date": "2026-04-16", "supplier": "Xero Ltd", "category_id": software, "gross_amount": 23_000,
                    "gst_treatment": "standard", "payment_method": "personal_funds", "classification": "partial",
                    "business_percentage": "50", "description": "Subscription" }),
        )
        .await,
    );
    assert_eq!(updated["supplier"], "Xero Ltd");
    assert_eq!(updated["gross_amount"], 23_000);
    let invalid = b
        .put(
            &format!("/expenses/{expense_id}"),
            json!({ "expense_date": "2026-04-16", "supplier": "", "category_id": software, "gross_amount": -1,
                    "gst_treatment": "standard", "payment_method": "business_account", "classification": "partial",
                    "business_percentage": "150" }),
        )
        .await;
    assert_eq!(
        invalid.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        invalid.body
    );
    assert!(
        invalid.field_error("gross_amount").is_some(),
        "{}",
        invalid.body
    );
    let filtered = b
        .get(&format!("/expenses?from=2026-04-01&to=2026-04-30&category_id={software}&search=xero&min_amount=100&max_amount=50000&gst_treatment=standard"))
        .await;
    assert_eq!(filtered.body["total"], 1, "{}", filtered.body);
    assert_eq!(
        b.get(&format!("/expenses/{}", Uuid::new_v4())).await.status,
        StatusCode::NOT_FOUND
    );

    // Categories: names are unique and required.
    let empty = b.post("/expense-categories", json!({ "name": " " })).await;
    assert!(empty.field_error("name").is_some());
    let taken = b
        .post("/expense-categories", json!({ "name": "software" }))
        .await;
    assert!(taken.field_error("name").is_some(), "{}", taken.body);

    let income = b.ok(
        b.post(
            "/income",
            json!({ "income_date": "2026-05-12", "payer": "Market stall", "income_type": "sales", "gross_amount": 23_000,
                    "gst_treatment": "standard", "payment_method": "bank_transfer" }),
        )
        .await,
    );
    let income_id = id(&income);
    assert_eq!(
        b.ok(b.get(&format!("/income/{income_id}")).await)["payer"],
        "Market stall"
    );
    let list = b.ok(b
        .get("/income?from=2026-05-01&to=2026-05-31&search=market")
        .await);
    assert_eq!(list["total"], 1, "{list}");
    let none = b.ok(b.get("/income?search=nothing-like-this").await);
    assert_eq!(none["total"], 0);
    let changed = b.ok(
        b.put(
            &format!("/income/{income_id}"),
            json!({ "income_date": "2026-05-13", "payer": "Craft market", "income_type": "other", "gross_amount": 5_000,
                    "gst_treatment": "exempt", "payment_method": "cash" }),
        )
        .await,
    );
    assert_eq!(changed["payer"], "Craft market");
    assert_eq!(
        b.get(&format!("/income/{}", Uuid::new_v4())).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn invoices_are_calculated_edited_listed_and_marked_sent() {
    let b = Books::new("maint-invoices").await;
    let calc = b.ok(
        b.post("/invoices/calculate", json!({ "issue_date": "2026-04-01", "prices_include_gst": false, "lines": lines(10_000) }))
            .await,
    );
    assert_eq!(calc["totals"]["total"], 23_000, "{calc}");
    let inclusive = b.ok(
        b.post("/invoices/calculate", json!({ "issue_date": "2026-04-01", "prices_include_gst": true, "lines": lines(11_500) }))
            .await,
    );
    assert_eq!(inclusive["totals"]["total"], 23_000, "{inclusive}");
    let bad = b
        .post("/invoices/calculate", json!({ "issue_date": "2026-04-01", "prices_include_gst": false,
            "lines": [{ "description": "", "quantity": "0", "unit_price": -5, "tax_treatment": "standard" }] }))
        .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", bad.body);

    let draft = b.ok(
        b.post("/invoices", json!({ "customer_id": b.customer, "issue_date": "2026-04-01", "due_date": "2026-04-20", "lines": lines(10_000) }))
            .await,
    );
    let invoice = id(&draft);
    let edited = b.ok(
        b.put(
            &format!("/invoices/{invoice}"),
            json!({ "customer_id": b.customer, "issue_date": "2026-04-02", "due_date": "2026-04-30", "reference": "PO-7",
                    "notes": "Thanks", "lines": lines(20_000) }),
        )
        .await,
    );
    assert_eq!(edited["total"], 46_000);
    assert_eq!(edited["reference"], "PO-7");
    b.ok(b
        .post(&format!("/invoices/{invoice}/issue"), json!({}))
        .await);
    // An issued invoice can still be corrected; its postings follow.
    let corrected = b.ok(
        b.put(
            &format!("/invoices/{invoice}"),
            json!({ "customer_id": b.customer, "issue_date": "2026-04-02", "due_date": "2026-04-30", "lines": lines(15_000) }),
        )
        .await,
    );
    assert_eq!(corrected["total"], 34_500);
    let sent = b.ok(b
        .post(&format!("/invoices/{invoice}/mark-sent"), json!({}))
        .await);
    assert!(sent["sent_at"].is_string(), "{sent}");

    for query in [
        "status=overdue,paid&search=INV",
        &format!(
            "customer_id={}&from=2026-04-01&to=2026-04-30&sort=-total",
            b.customer
        ),
        "sort=due_date",
        "sort=number",
        "sort=customer",
        "sort=issue_date",
    ] {
        let list = b.ok(b.get(&format!("/invoices?{query}")).await);
        assert_eq!(list["total"], 1, "{query}: {list}");
    }
    let none = b.ok(b.get("/invoices?status=draft").await);
    assert_eq!(none["total"], 0);
    // A cancelled one can't.
    b.ok(b
        .post(
            &format!("/invoices/{invoice}/cancel"),
            json!({ "reason": "Wrong customer" }),
        )
        .await);
    let locked = b
        .put(
            &format!("/invoices/{invoice}"),
            json!({ "customer_id": b.customer, "issue_date": "2026-04-02", "lines": lines(1) }),
        )
        .await;
    assert_eq!(locked.status, StatusCode::CONFLICT, "{}", locked.body);
}

#[tokio::test]
async fn quotes_are_listed_edited_sent_and_deleted() {
    let b = Books::new("maint-quotes").await;
    let quote = b.ok(b.post("/quotes", json!({ "customer_id": b.customer, "issue_date": "2099-01-01", "lines": lines(50_000) })).await);
    let quote_id = id(&quote);
    let edited = b.ok(
        b.put(
            &format!("/quotes/{quote_id}"),
            json!({ "customer_id": b.customer, "issue_date": "2099-01-02", "expiry_date": "2099-02-01", "notes": "Valid a month",
                    "lines": lines(60_000) }),
        )
        .await,
    );
    assert_eq!(edited["total"], 138_000, "{edited}");
    let invalid = b
        .put(
            &format!("/quotes/{quote_id}"),
            json!({ "customer_id": b.customer, "issue_date": "2099-01-02", "lines": [] }),
        )
        .await;
    assert_eq!(
        invalid.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        invalid.body
    );

    for query in [
        "",
        "?status=draft",
        "?search=QU",
        &format!("?customer_id={}", b.customer),
    ] {
        let list = b.ok(b.get(&format!("/quotes{query}")).await);
        assert_eq!(list["total"], 1, "{query}: {list}");
    }
    let sent = b.ok(b
        .post(
            &format!("/quotes/{quote_id}/send"),
            json!({ "message": "Here’s the quote." }),
        )
        .await);
    assert!(sent["sent_at"].is_string(), "{sent}");
    let email = b.app.email("accounts@tui.example", "Quote QU-0001").await;
    assert!(email.text.contains("Here’s the quote."));
    let sent_list = b.ok(b.get("/quotes?status=sent").await);
    assert_eq!(sent_list["total"], 1, "{sent_list}");

    let second = id(&b.ok(b
        .post(
            "/quotes",
            json!({ "customer_id": b.customer, "issue_date": "2099-01-01", "lines": lines(1_000) }),
        )
        .await));
    let removed = b.delete(&format!("/quotes/{second}"), None).await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{}", removed.body);
    assert_eq!(
        b.get(&format!("/quotes/{second}")).await.status,
        StatusCode::NOT_FOUND
    );
    // A converted quote is kept for the record.
    b.ok(b
        .post(
            &format!("/quotes/{quote_id}/status"),
            json!({ "status": "accepted" }),
        )
        .await);
    b.ok(b
        .post(&format!("/quotes/{quote_id}/convert"), json!({}))
        .await);
    let kept = b.delete(&format!("/quotes/{quote_id}"), None).await;
    assert_eq!(kept.status, StatusCode::CONFLICT, "{}", kept.body);
}

#[tokio::test]
async fn mileage_rates_versions_and_vehicles_can_be_changed() {
    let b = Books::new("maint-mileage").await;
    let vehicle = b.ok(
        b.post(
            "/mileage/vehicles",
            json!({ "name": "Corolla", "vehicle_type": "car", "fuel_type": "hybrid",
                    "new_rate": { "name": "Kilometre rate", "effective_from": "2025-04-01", "rate_per_km": "1.04" } }),
        )
        .await,
    );
    let vehicle_id = id(&vehicle);
    let rate = vehicle["default_rate_id"]
        .as_str()
        .expect("rate")
        .to_owned();

    let renamed = b.ok(b
        .put(
            &format!("/mileage/rates/{rate}"),
            json!({ "name": "IRD rate", "notes": "Tier 1" }),
        )
        .await);
    assert_eq!(renamed["name"], "IRD rate");
    let blank = b
        .put(&format!("/mileage/rates/{rate}"), json!({ "name": "" }))
        .await;
    assert!(blank.field_error("name").is_some(), "{}", blank.body);

    let version = b.ok(b
        .post(
            &format!("/mileage/rates/{rate}/versions"),
            json!({ "effective_from": "2026-04-01", "rate_per_km": "1.17" }),
        )
        .await);
    let version_id = version["versions"]
        .as_array()
        .expect("versions")
        .iter()
        .find(|v| v["effective_from"] == "2026-04-01")
        .map(id)
        .expect("new version");
    let changed = b.ok(b
        .put(
            &format!("/mileage/rates/{rate}/versions/{version_id}"),
            json!({ "effective_from": "2026-04-01", "rate_per_km": "1.20" }),
        )
        .await);
    assert!(changed.to_string().contains("1.2"), "{changed}");
    let clash = b
        .put(
            &format!("/mileage/rates/{rate}/versions/{version_id}"),
            json!({ "effective_from": "2025-04-01", "rate_per_km": "1.20" }),
        )
        .await;
    assert!(clash.status.is_client_error(), "{}", clash.body);
    let removed = b
        .delete(
            &format!("/mileage/rates/{rate}/versions/{version_id}"),
            None,
        )
        .await;
    assert!(removed.status.is_success(), "{}", removed.body);

    let spare = b.ok(b
        .post(
            "/mileage/rates",
            json!({ "name": "Spare", "effective_from": "2026-01-01", "rate_per_km": "0.90" }),
        )
        .await);
    let spare_id = id(&spare);
    let archived = b.ok(b
        .post(
            &format!("/mileage/rates/{spare_id}/archive"),
            json!({ "archived": true }),
        )
        .await);
    assert_eq!(archived["archived"], true);
    b.ok(b
        .post(
            &format!("/mileage/rates/{spare_id}/archive"),
            json!({ "archived": false }),
        )
        .await);

    let updated = b.ok(
        b.put(
            &format!("/mileage/vehicles/{vehicle_id}"),
            json!({ "name": "Family car", "registration": "xyz789", "vehicle_type": "car", "fuel_type": "petrol",
                    "default_rate_id": spare_id, "is_default": true, "notes": "Shared" }),
        )
        .await,
    );
    assert_eq!(updated["name"], "Family car");
    assert_eq!(updated["registration"], "XYZ789");
    assert_eq!(updated["default_rate_id"], spare_id.as_str());
    let missing_rate = b
        .put(
            &format!("/mileage/vehicles/{vehicle_id}"),
            json!({ "name": "Family car", "vehicle_type": "car", "fuel_type": "petrol", "default_rate_id": Uuid::new_v4() }),
        )
        .await;
    assert!(
        missing_rate.status.is_client_error(),
        "{}",
        missing_rate.body
    );
}

#[tokio::test]
async fn allocation_rules_can_be_listed_changed_and_reordered() {
    let b = Books::new("maint-rules").await;
    let holiday = id(&b.ok(b
        .post(
            "/allocations/destinations",
            json!({ "name": "Holiday fund" }),
        )
        .await));
    let first = id(&b.ok(b
        .post(
            "/allocations/rules",
            json!({ "destination_id": holiday, "method": "percentage", "percentage": "10" }),
        )
        .await));
    let second = id(&b.ok(b
        .post(
            "/allocations/rules",
            json!({ "destination_id": holiday, "method": "fixed", "fixed_amount": 5_000 }),
        )
        .await));
    let rules = b.ok(b.get("/allocations/rules").await);
    assert_eq!(rules.as_array().map(Vec::len), Some(2), "{rules}");

    let updated = b.ok(
        b.put(
            &format!("/allocations/rules/{first}"),
            json!({ "destination_id": holiday, "method": "percentage", "percentage": "15", "basis": "received",
                    "source": "income", "income_type": "sales", "active_from": "2026-01-01", "active_to": "2026-12-31",
                    "max_amount": 100_000 }),
        )
        .await,
    );
    assert_eq!(updated["percentage"], "15.00", "{updated}");
    let invalid = b
        .put(
            &format!("/allocations/rules/{first}"),
            json!({ "destination_id": holiday, "method": "percentage", "percentage": "150" }),
        )
        .await;
    assert!(
        invalid.field_error("percentage").is_some(),
        "{}",
        invalid.body
    );

    let ordered = b
        .put(
            "/allocations/rules/order",
            json!({ "ids": [second, first] }),
        )
        .await;
    assert!(ordered.status.is_success(), "{}", ordered.body);
    let rules = b.ok(b.get("/allocations/rules").await);
    assert_eq!(rules[0]["id"], second.as_str(), "{rules}");
    let wrong = b
        .put("/allocations/rules/order", json!({ "ids": [first] }))
        .await;
    assert!(wrong.status.is_client_error(), "{}", wrong.body);

    // The allocations report describes each rule in words.
    let report = b.ok(b
        .get("/reports/allocations?from=2026-01-01&to=2026-12-31")
        .await);
    assert!(report.to_string().contains("Holiday fund"), "{report}");
}

#[tokio::test]
async fn health_checks_answer_without_an_owner() {
    let app = TestApp::new().await;
    assert_eq!(
        app.get("/health/live", &Auth::None).await.status,
        StatusCode::OK
    );
    assert_eq!(
        app.get("/health/ready", &Auth::None).await.status,
        StatusCode::OK
    );
}
