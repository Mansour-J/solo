//! Money in and out, fixed assets, depreciation and opening balances: validation, edits, deletion,
//! the conflicts that protect posted years, and isolation between businesses. The full-year figures
//! are checked in `annual_reports.rs`.

mod common;

use axum::http::StatusCode;
use common::{TestApp, TestResponse, TestUser};
use serde_json::{Value, json};
use uuid::Uuid;

struct Books {
    app: TestApp,
    user: TestUser,
    business: Uuid,
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
        Self {
            app,
            user,
            business,
        }
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

    async fn created(&self, rest: &str, body: Value) -> Value {
        let response = self.post(rest, body).await;
        assert!(
            response.status.is_success(),
            "{rest}: {} {}",
            response.status,
            response.body
        );
        response.body
    }

    async fn report(&self, kind: &str, query: &str) -> Value {
        let response = self.get(&format!("/reports/{kind}?{query}")).await;
        assert_eq!(response.status, StatusCode::OK, "{kind}: {}", response.body);
        response.body
    }

    async fn opening(&self) {
        let saved = self
            .put(
                "/opening-balances",
                json!({ "as_at": "2024-03-31", "bank": 1_000_000, "gst_payable": 50_000, "drawings": -200_000,
                        "funds_introduced": 100_000 }),
            )
            .await;
        assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    }
}

fn line(report: &Value, label: &str) -> Vec<Value> {
    report["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .flat_map(|s| {
            let mut rows = s["rows"].as_array().cloned().unwrap_or_default();
            rows.extend(s["totals"].as_array().map(|t| Value::Array(t.clone())));
            rows
        })
        .filter_map(|r| r.as_array().cloned())
        .find(|r| r[0] == label)
        .unwrap_or_else(|| panic!("no line {label}"))
}

fn sheet_at(to: &str) -> String {
    format!("from=2024-04-01&to={to}&basis=accrual")
}

// ─── Money in and out ───────────────────────────────────────────────────────

#[tokio::test]
async fn movements_post_to_the_bank_and_equity_and_can_be_changed_or_deleted() {
    let b = Books::new("movements").await;
    let drawing = b
        .created(
            "/cash-movements",
            json!({ "movement_date": "2024-05-01", "kind": "drawing", "amount": 50_000, "description": "  Rent  " }),
        )
        .await;
    assert_eq!(drawing["description"], "Rent");
    assert_eq!(drawing["currency"], "NZD");
    for (date, kind, amount) in [
        ("2024-05-02", "funds_introduced", 20_000),
        ("2024-05-03", "gst_refund", 3_000),
        ("2024-05-04", "income_tax_payment", 7_000),
        ("2024-05-05", "income_tax_refund", 1_000),
        ("2024-05-06", "gst_payment", 4_000),
    ] {
        b.created(
            "/cash-movements",
            json!({ "movement_date": date, "kind": kind, "amount": amount }),
        )
        .await;
    }
    let sheet = b.report("balance_sheet", &sheet_at("2024-06-30")).await;
    // 0 − 500 + 200 + 30 − 70 + 10 − 40 = −370.
    assert_eq!(line(&sheet, "Business bank account")[1], -37_000);
    assert_eq!(line(&sheet, "Drawings")[1], -50_000);
    assert_eq!(line(&sheet, "Owner funds introduced")[1], 20_000);
    assert_eq!(line(&sheet, "Income tax paid")[1], -6_000);
    // A GST refund received (30) less a payment (40) leaves 10 more paid than owed: a refund due.
    assert_eq!(line(&sheet, "GST refund due")[1], 1_000);
    assert_eq!(
        line(&sheet, "Net assets")[1],
        line(&sheet, "Total equity")[1]
    );

    let equity = b.report("equity_changes", &sheet_at("2025-03-31")).await;
    // Decreases are shown as positive amounts: $70 income tax paid less the $10 refund, and the drawing.
    assert_eq!(line(&equity, "Income tax paid")[1], 6_000);
    assert_eq!(line(&equity, "Drawings")[1], 50_000);
    assert_eq!(line(&equity, "Total decreases")[1], 56_000);
    assert_eq!(line(&equity, "Owner funds introduced")[1], 20_000);

    // Filters and paging.
    let list = b.get("/cash-movements?kind=drawing").await;
    assert_eq!(list.body["total"], 1, "{}", list.body);
    let list = b
        .get("/cash-movements?from=2024-05-03&to=2024-05-04&per_page=1&page=2")
        .await;
    assert_eq!(list.body["total"], 2);
    assert_eq!(list.body["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(list.body["page"], 2);

    let id = drawing["id"].as_str().expect("id");
    let updated = b
        .put(
            &format!("/cash-movements/{id}"),
            json!({ "movement_date": "2024-05-01", "kind": "drawing", "amount": 60_000 }),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    assert_eq!(updated.body["amount"], 60_000);
    let sheet = b.report("balance_sheet", &sheet_at("2024-06-30")).await;
    assert_eq!(line(&sheet, "Drawings")[1], -60_000);

    let deleted = b
        .delete(
            &format!("/cash-movements/{id}"),
            Some(json!({ "reason": "Entered twice" })),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{}", deleted.body);
    let sheet = b.report("balance_sheet", &sheet_at("2024-06-30")).await;
    assert_eq!(line(&sheet, "Business bank account")[1], 13_000);
    let again = b
        .delete(
            &format!("/cash-movements/{id}"),
            Some(json!({ "reason": "x" })),
        )
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    let change = b
        .put(
            &format!("/cash-movements/{id}"),
            json!({ "movement_date": "2024-05-01", "kind": "drawing", "amount": 1 }),
        )
        .await;
    assert_eq!(change.status, StatusCode::CONFLICT);
    let missing = b
        .put(
            &format!("/cash-movements/{}", Uuid::new_v4()),
            json!({ "movement_date": "2024-05-01", "kind": "drawing", "amount": 1 }),
        )
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    let audit = b.get("/activity").await;
    assert!(
        audit.body.to_string().contains("cash_movement.deleted"),
        "{}",
        audit.body
    );
}

#[tokio::test]
async fn movements_are_validated() {
    let b = Books::new("movements-invalid").await;
    let bad = b
        .post(
            "/cash-movements",
            json!({ "movement_date": "2024-05-01", "kind": "drawing", "amount": 0, "description": "x".repeat(301) }),
        )
        .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(bad.field_error("amount").is_some());
    assert!(bad.field_error("description").is_some());
    let unknown = b
        .post(
            "/cash-movements",
            json!({ "movement_date": "2024-05-01", "kind": "gift", "amount": 1 }),
        )
        .await;
    assert_eq!(unknown.status, StatusCode::UNPROCESSABLE_ENTITY);
    let created = b
        .created(
            "/cash-movements",
            json!({ "movement_date": "2024-05-01", "kind": "drawing", "amount": 1 }),
        )
        .await;
    let no_reason = b
        .delete(
            &format!("/cash-movements/{}", created["id"].as_str().expect("id")),
            Some(json!({ "reason": "x".repeat(501) })),
        )
        .await;
    assert!(
        no_reason.field_error("reason").is_some(),
        "{}",
        no_reason.body
    );
}

// ─── Withholding tax ────────────────────────────────────────────────────────

#[tokio::test]
async fn tax_withheld_from_a_payment_reaches_equity_not_the_bank() {
    let b = Books::new("withholding").await;
    let customer = b.created("/customers", json!({ "name": "Agency" })).await["id"].clone();
    let draft = b
        .created(
            "/invoices",
            json!({ "customer_id": customer, "issue_date": "2024-06-01", "due_date": "2024-06-20",
                    "lines": [{ "description": "Work", "quantity": "1", "unit_price": 100_000, "tax_treatment": "standard" }] }),
        )
        .await;
    let id = draft["id"].as_str().expect("id");
    b.created(&format!("/invoices/{id}/issue"), json!({})).await;
    let too_much = b
        .post(
            &format!("/invoices/{id}/payments"),
            json!({ "payment_date": "2024-06-20", "amount": 115_000, "withholding_tax": 115_000, "method": "bank_transfer" }),
        )
        .await;
    assert_eq!(
        too_much.field_error("withholding_tax"),
        Some("Tax withheld must be less than the amount paid.")
    );
    let negative = b
        .post(
            &format!("/invoices/{id}/payments"),
            json!({ "payment_date": "2024-06-20", "amount": 115_000, "withholding_tax": -1, "method": "bank_transfer" }),
        )
        .await;
    assert!(negative.field_error("withholding_tax").is_some());
    let paid = b
        .created(
            &format!("/invoices/{id}/payments"),
            json!({ "payment_date": "2024-06-20", "amount": 115_000, "withholding_tax": 20_000, "method": "bank_transfer" }),
        )
        .await;
    assert_eq!(paid["payments"][0]["withholding_tax"], 20_000, "{paid}");
    for basis in ["accrual", "cash"] {
        let sheet = b
            .report(
                "balance_sheet",
                &format!("from=2024-04-01&to=2025-03-31&basis={basis}"),
            )
            .await;
        assert_eq!(line(&sheet, "Business bank account")[1], 95_000, "{basis}");
        assert_eq!(line(&sheet, "Withholding tax paid")[1], -20_000, "{basis}");
        assert_eq!(
            line(&sheet, "Net assets")[1],
            line(&sheet, "Total equity")[1],
            "{basis}"
        );
    }
    let equity = b.report("equity_changes", &sheet_at("2025-03-31")).await;
    assert_eq!(line(&equity, "Withholding tax paid")[1], 20_000);
}

// ─── Opening balances ───────────────────────────────────────────────────────

#[tokio::test]
async fn opening_balances_post_once_and_balance_with_retained_earnings() {
    let b = Books::new("opening").await;
    assert_eq!(
        b.get("/opening-balances").await.body["balances"],
        Value::Null
    );
    let missing = b.delete("/opening-balances", None).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);

    let invalid = b
        .put(
            "/opening-balances",
            json!({ "as_at": "2024-03-31", "drawings": 5, "withholding_tax": 1, "income_tax": 1, "funds_introduced": -1,
                    "bank": 200_000_000_000_i64 }),
        )
        .await;
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
    for field in [
        "drawings",
        "withholding_tax",
        "income_tax",
        "funds_introduced",
        "bank",
    ] {
        assert!(
            invalid.field_error(field).is_some(),
            "{field}: {}",
            invalid.body
        );
    }

    b.opening().await;
    let view = b.get("/opening-balances").await.body;
    // 10,000 bank − 500 GST = 9,500 net; equity = −2,000 drawings + 1,000 funds + retained.
    assert_eq!(view["retained_earnings"], 1_050_000);
    let sheet = b.report("balance_sheet", &sheet_at("2025-03-31")).await;
    assert_eq!(
        line(&sheet, "Business bank account")[1..],
        [json!(1_000_000), json!(1_000_000)]
    );
    assert_eq!(line(&sheet, "GST payable")[1], 50_000);
    assert_eq!(line(&sheet, "Retained earnings")[1], 1_050_000);
    assert_eq!(line(&sheet, "Total equity")[1], 950_000);
    let equity = b.report("equity_changes", &sheet_at("2025-03-31")).await;
    assert_eq!(line(&equity, "Opening balance")[1], 950_000);
    assert_eq!(line(&equity, "Total equity")[1], 950_000);

    // Saving again replaces the entry rather than adding another.
    let saved = b
        .put(
            "/opening-balances",
            json!({ "as_at": "2024-03-31", "bank": 400_000 }),
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    let sheet = b.report("balance_sheet", &sheet_at("2025-03-31")).await;
    assert_eq!(line(&sheet, "Business bank account")[1], 400_000);
    assert_eq!(line(&sheet, "Total equity")[1], 400_000);

    let removed = b.delete("/opening-balances", None).await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    let sheet = b.report("balance_sheet", &sheet_at("2025-03-31")).await;
    assert_eq!(line(&sheet, "Business bank account")[1], 0);
}

// ─── Assets and depreciation ────────────────────────────────────────────────

fn laptop(acquired_on: &str, opening_value: Option<i64>) -> Value {
    json!({ "name": "Laptop", "asset_group": "Computer equipment", "cost": 240_000, "purchase_gst": 36_000,
            "acquired_on": acquired_on, "method": "dv", "rate": "50", "opening_value": opening_value })
}

#[tokio::test]
async fn assets_are_validated_against_the_opening_balances() {
    let b = Books::new("assets-invalid").await;
    let bad = b
        .post(
            "/assets",
            json!({ "name": " ", "asset_group": "", "cost": 0, "purchase_gst": -1, "acquired_on": "2024-05-01",
                    "method": "sl", "rate": "0", "private_use": "100", "opening_value": 5, "notes": "x".repeat(1001) }),
        )
        .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
    for field in [
        "name",
        "asset_group",
        "cost",
        "purchase_gst",
        "rate",
        "private_use",
        "opening_value",
        "notes",
    ] {
        assert!(bad.field_error(field).is_some(), "{field}: {}", bad.body);
    }
    b.opening().await;
    let late = b.post("/assets", laptop("2024-05-01", Some(100_000))).await;
    assert!(
        late.field_error("opening_value")
            .is_some_and(|m| m.contains("Only assets")),
        "{}",
        late.body
    );
    let early = b.post("/assets", laptop("2023-05-01", None)).await;
    assert!(
        early
            .field_error("opening_value")
            .is_some_and(|m| m.contains("before your opening")),
        "{}",
        early.body
    );
}

#[tokio::test]
async fn a_purchase_in_the_year_posts_its_cost_and_gst_and_depreciates_for_the_months_owned() {
    let b = Books::new("assets-purchase").await;
    b.opening().await;
    let asset = b.created("/assets", laptop("2024-10-10", None)).await;
    let id = asset["id"].as_str().expect("id").to_owned();
    assert_eq!(asset["book_value"], 240_000);
    let sheet = b.report("balance_sheet", &sheet_at("2024-12-31")).await;
    assert_eq!(line(&sheet, "Property, plant and equipment")[1], 240_000);
    // 10,000 − 2,400 − 360 GST.
    assert_eq!(line(&sheet, "Business bank account")[1], 724_000);
    assert_eq!(line(&sheet, "GST payable")[1], 14_000);

    // Changing the cost before depreciation re-posts the purchase.
    let mut cheaper = laptop("2024-10-10", None);
    cheaper["cost"] = json!(200_000);
    cheaper["purchase_gst"] = json!(30_000);
    cheaper["private_use"] = json!("10");
    let updated = b.put(&format!("/assets/{id}"), cheaper.clone()).await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    let sheet = b.report("balance_sheet", &sheet_at("2024-12-31")).await;
    assert_eq!(line(&sheet, "Property, plant and equipment")[1], 200_000);

    let year = b
        .created("/assets/depreciation", json!({ "year_end": "2025-03-31" }))
        .await;
    // October – March is six months: 2,000 × 50% × 6/12 = 500, of which 10% is private.
    assert_eq!(year["depreciation"], 50_000, "{year}");
    assert_eq!(year["private_amount"], 5_000);
    assert_eq!(year["lines"][0]["purchases"], 200_000);
    assert_eq!(year["lines"][0]["closing_value"], 150_000);
    let fetched = b.get("/assets/depreciation?year_end=2025-03-31").await;
    assert_eq!(fetched.body["depreciation"], 50_000);
    let pnl = b.report("profit_and_loss", &sheet_at("2025-03-31")).await;
    assert_eq!(
        line(&pnl, "Depreciation")[1],
        45_000,
        "only the business share is an expense"
    );
    let sheet = b.report("balance_sheet", &sheet_at("2025-03-31")).await;
    assert_eq!(line(&sheet, "Property, plant and equipment")[1], 150_000);
    assert_eq!(
        line(&sheet, "Drawings")[1],
        -205_000,
        "the private share is drawn"
    );
    let schedule = b
        .report("depreciation_schedule", &sheet_at("2025-03-31"))
        .await;
    assert_eq!(line(&schedule, "Laptop")[3], 200_000);
    assert_eq!(line(&schedule, "Laptop")[7], 5_000);

    // Posted years protect the asset.
    let locked = b.put(&format!("/assets/{id}"), cheaper.clone()).await;
    assert_eq!(locked.status, StatusCode::CONFLICT);

    // The next year depreciates the closing value, needing this one first.
    let skip = b
        .post("/assets/depreciation", json!({ "year_end": "2027-03-31" }))
        .await;
    assert_eq!(skip.status, StatusCode::CONFLICT, "{}", skip.body);
    let next = b
        .created("/assets/depreciation", json!({ "year_end": "2026-03-31" }))
        .await;
    assert_eq!(next["lines"][0]["opening_value"], 150_000);
    assert_eq!(next["depreciation"], 75_000);
    // Re-posting the first year drops the year after it.
    b.created("/assets/depreciation", json!({ "year_end": "2025-03-31" }))
        .await;
    assert_eq!(
        b.get("/assets/depreciation?year_end=2026-03-31").await.body["lines"],
        json!([])
    );

    // Archived assets stop depreciating and can't be changed until restored.
    let archived = b
        .created(
            &format!("/assets/{id}/archive"),
            json!({ "archived": true }),
        )
        .await;
    assert_eq!(archived["archived"], true);
    let blocked = b.put(&format!("/assets/{id}"), cheaper.clone()).await;
    assert_eq!(blocked.status, StatusCode::CONFLICT);
    let later = b
        .created("/assets/depreciation", json!({ "year_end": "2026-03-31" }))
        .await;
    assert_eq!(later["lines"], json!([]));
    b.created(
        &format!("/assets/{id}/archive"),
        json!({ "archived": false }),
    )
    .await;

    let gone = b
        .delete("/assets/depreciation?year_end=2025-03-31", None)
        .await;
    assert_eq!(gone.status, StatusCode::NO_CONTENT);
    let again = b
        .delete("/assets/depreciation?year_end=2025-03-31", None)
        .await;
    assert_eq!(again.status, StatusCode::NOT_FOUND);
    let editable = b
        .put(&format!("/assets/{id}"), laptop("2024-10-10", None))
        .await;
    assert_eq!(editable.status, StatusCode::OK, "{}", editable.body);
    assert_eq!(
        b.get("/assets").await.body.as_array().map(Vec::len),
        Some(1)
    );
}

#[tokio::test]
async fn depreciation_years_are_checked() {
    let b = Books::new("depreciation-years").await;
    let not_year_end = b
        .post("/assets/depreciation", json!({ "year_end": "2025-03-30" }))
        .await;
    assert!(
        not_year_end
            .field_error("year_end")
            .is_some_and(|m| m.contains("31/03/2025")),
        "{}",
        not_year_end.body
    );
    let get_bad = b.get("/assets/depreciation?year_end=2025-01-01").await;
    assert_eq!(get_bad.status, StatusCode::UNPROCESSABLE_ENTITY);
    let delete_bad = b
        .delete("/assets/depreciation?year_end=2025-01-01", None)
        .await;
    assert_eq!(delete_bad.status, StatusCode::UNPROCESSABLE_ENTITY);

    // An asset bought in an earlier year needs that year posted first.
    b.created("/assets", laptop("2023-06-01", None)).await;
    let skipped = b
        .post("/assets/depreciation", json!({ "year_end": "2025-03-31" }))
        .await;
    assert_eq!(skipped.status, StatusCode::CONFLICT, "{}", skipped.body);
    assert!(skipped.body.to_string().contains("Laptop"));

    // An asset brought forward must start in the year after the opening balances.
    let c = Books::new("depreciation-opening").await;
    c.opening().await;
    c.created("/assets", laptop("2023-06-01", Some(120_000)))
        .await;
    assert_eq!(
        c.get("/opening-balances").await.body["fixed_assets"],
        120_000
    );
    let before = c
        .post("/assets/depreciation", json!({ "year_end": "2024-03-31" }))
        .await;
    assert!(before.field_error("year_end").is_some(), "{}", before.body);
    let gap = c
        .post("/assets/depreciation", json!({ "year_end": "2026-03-31" }))
        .await;
    assert_eq!(gap.status, StatusCode::CONFLICT, "{}", gap.body);
    let first = c
        .created("/assets/depreciation", json!({ "year_end": "2025-03-31" }))
        .await;
    assert_eq!(first["lines"][0]["opening_value"], 120_000);
    assert_eq!(first["depreciation"], 60_000);
    // Removing the first year and posting the second can't skip it.
    c.created("/assets/depreciation", json!({ "year_end": "2026-03-31" }))
        .await;
    let sheet = c
        .report(
            "balance_sheet",
            "from=2025-04-01&to=2026-03-31&basis=accrual",
        )
        .await;
    assert_eq!(
        line(&sheet, "Property, plant and equipment")[1..],
        [json!(30_000), json!(60_000)]
    );
    assert_eq!(
        line(&sheet, "Net assets")[1],
        line(&sheet, "Total equity")[1]
    );
    assert_eq!(
        line(&sheet, "Net assets")[2],
        line(&sheet, "Total equity")[2]
    );
}

// ─── Non-deductible expenses ────────────────────────────────────────────────

#[tokio::test]
async fn non_deductible_categories_are_adjustments_in_profit_and_loss() {
    let b = Books::new("non-deductible").await;
    let category = b
        .created(
            "/expense-categories",
            json!({ "name": "Penalties", "non_deductible": true }),
        )
        .await;
    assert_eq!(category["non_deductible"], true, "{category}");
    b.created(
        "/expenses",
        json!({ "expense_date": "2024-07-01", "supplier": "IRD", "category_id": category["id"], "gross_amount": 10_000,
                "gst_treatment": "exempt", "payment_method": "business_account", "classification": "business" }),
    )
    .await;
    let pnl = b.report("profit_and_loss", &sheet_at("2025-03-31")).await;
    assert_eq!(line(&pnl, "Penalties")[1], 10_000);
    assert_eq!(line(&pnl, "Total non-deductible expenses")[1], 10_000);
    assert_eq!(line(&pnl, "Total expenses")[1], 0);
    assert_eq!(line(&pnl, "Net profit before adjustments")[1], 0);
    assert_eq!(line(&pnl, "Net profit")[1], -10_000);

    // Clearing the flag moves it back into expenses.
    let id = category["id"].as_str().expect("id");
    let updated = b
        .put(
            &format!("/expense-categories/{id}"),
            json!({ "name": "Penalties", "non_deductible": false }),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
    let pnl = b.report("profit_and_loss", &sheet_at("2025-03-31")).await;
    assert_eq!(line(&pnl, "Total expenses")[1], 10_000);
    assert!(!pnl.to_string().contains("Net profit before adjustments"));
}

// ─── Export, isolation ──────────────────────────────────────────────────────

#[tokio::test]
async fn annual_records_are_exported_and_isolated_between_businesses() {
    let b = Books::new("annual-isolated").await;
    b.opening().await;
    let asset = b
        .created("/assets", laptop("2023-06-01", Some(120_000)))
        .await;
    let movement = b
        .created(
            "/cash-movements",
            json!({ "movement_date": "2024-05-01", "kind": "drawing", "amount": 1_000 }),
        )
        .await;
    b.created("/assets/depreciation", json!({ "year_end": "2025-03-31" }))
        .await;

    let export = b.app.get("/api/v1/users/me/export", &b.user.auth()).await;
    let records = &export.body["businesses"][0]["records"];
    for table in [
        "cash_movements",
        "fixed_assets",
        "depreciation_entries",
        "opening_balances",
    ] {
        assert_eq!(records[table].as_array().map(Vec::len), Some(1), "{table}");
    }

    let intruder = b.app.register("annual-intruder").await;
    for path in [
        "/cash-movements",
        "/assets",
        "/opening-balances",
        "/assets/depreciation?year_end=2025-03-31",
    ] {
        assert_eq!(
            b.app.get(&b.path(path), &intruder.auth()).await.status,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    // In their own business, the victim's records don't exist.
    let own = b.app.create_business(&intruder, "Intruder Ltd").await;
    let theirs = |rest: String| format!("/api/v1/businesses/{own}{rest}");
    let movement_id = movement["id"].as_str().expect("id");
    let asset_id = asset["id"].as_str().expect("id");
    let edit = b
        .app
        .put(
            &theirs(format!("/cash-movements/{movement_id}")),
            &intruder.auth(),
            json!({ "movement_date": "2024-05-01", "kind": "drawing", "amount": 1 }),
        )
        .await;
    assert_eq!(edit.status, StatusCode::NOT_FOUND);
    let archive = b
        .app
        .post(
            &theirs(format!("/assets/{asset_id}/archive")),
            &intruder.auth(),
            json!({ "archived": true }),
        )
        .await;
    assert_eq!(archive.status, StatusCode::NOT_FOUND);
    assert_eq!(
        b.app
            .get(&theirs("/assets".into()), &intruder.auth())
            .await
            .body,
        json!([])
    );
    assert_eq!(
        b.app
            .get(&theirs("/opening-balances".into()), &intruder.auth())
            .await
            .body["balances"],
        Value::Null
    );
    let mut tx = b
        .app
        .state
        .db
        .begin_business(intruder.id, own)
        .await
        .expect("tx");
    for table in [
        "cash_movements",
        "fixed_assets",
        "depreciation_entries",
        "opening_balances",
    ] {
        let sql = match table {
            "cash_movements" => "SELECT count(*) FROM cash_movements",
            "fixed_assets" => "SELECT count(*) FROM fixed_assets",
            "depreciation_entries" => "SELECT count(*) FROM depreciation_entries",
            _ => "SELECT count(*) FROM opening_balances",
        };
        let visible: i64 = sqlx::query_scalar(sql)
            .fetch_one(&mut *tx)
            .await
            .expect("count");
        assert_eq!(visible, 0, "{table}");
    }
    drop(tx);

    // Deleting the business removes everything it owns.
    let business = b.get("").await.body;
    let deleted = b
        .delete("", Some(json!({ "confirmation_name": business["name"] })))
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{}", deleted.body);
}
