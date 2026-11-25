//! A made-up sole trader's years to 31 March 2025 and 2026, entered through the API from
//! `fixtures/fy2025.json` and `fixtures/fy2026.json` (built and checked by `fixtures/build.mjs`) and
//! compared with every figure an accountant's annual report would print, worked out independently by
//! the simulation in `build.mjs`: profit and loss, balance sheet, statement of changes in equity and
//! depreciation schedule, with the comparative year beside each. Annual reports print whole dollars;
//! the app keeps cents, so each figure is compared after rounding to the dollar.

mod common;

use axum::http::StatusCode;
use common::{TestApp, TestResponse, TestUser};
use serde_json::{Value, json};
use uuid::Uuid;

const FY2025: &str = include_str!("fixtures/fy2025.json");
const FY2026: &str = include_str!("fixtures/fy2026.json");

struct Books {
    app: TestApp,
    user: TestUser,
    business: Uuid,
    customer: Option<Value>,
}

fn fixture(text: &str) -> Value {
    serde_json::from_str(text).expect("fixture")
}

impl Books {
    /// The business, its opening balances at 31 March 2024 and the assets it brought forward.
    async fn new(prefix: &str) -> Self {
        let first = fixture(FY2025);
        let app = TestApp::new().await;
        let user = app.register(prefix).await;
        let mut business = first["business"].clone();
        business["complete_onboarding"] = json!(true);
        let created = app.post("/api/v1/businesses", &user.auth(), business).await;
        assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
        let business = created.body["id"]
            .as_str()
            .expect("id")
            .parse()
            .expect("uuid");
        let books = Self {
            app,
            user,
            business,
            customer: None,
        };
        books
            .put("/opening-balances", first["opening_balances"].clone())
            .await;
        for asset in first["assets"].as_array().expect("assets") {
            books.post("/assets", asset.clone()).await;
        }
        books
    }

    /// FY2025 entered.
    async fn fy2025(prefix: &str) -> Self {
        let mut books = Self::new(prefix).await;
        books.enter(&fixture(FY2025)).await;
        books
    }

    /// FY2025 then FY2026 entered, in the same business.
    async fn fy2026(prefix: &str) -> Self {
        let mut books = Self::fy2025(prefix).await;
        books.enter(&fixture(FY2026)).await;
        books
    }

    fn path(&self, rest: &str) -> String {
        format!("/api/v1/businesses/{}{rest}", self.business)
    }

    async fn get(&self, rest: &str) -> TestResponse {
        self.app.get(&self.path(rest), &self.user.auth()).await
    }

    fn ok(response: TestResponse, what: &str) -> Value {
        assert!(
            response.status.is_success(),
            "{what}: {} {}",
            response.status,
            response.body
        );
        response.body
    }

    async fn post(&self, rest: &str, body: Value) -> Value {
        Self::ok(
            self.app
                .post(&self.path(rest), &self.user.auth(), body)
                .await,
            rest,
        )
    }

    async fn put(&self, rest: &str, body: Value) -> Value {
        Self::ok(
            self.app
                .put(&self.path(rest), &self.user.auth(), body)
                .await,
            rest,
        )
    }

    async fn categories(&self) -> Vec<Value> {
        Self::ok(self.get("/expense-categories").await, "categories")
            .as_array()
            .expect("categories")
            .clone()
    }

    /// Enters a year the way a user would, and posts its depreciation.
    async fn enter(&mut self, d: &Value) {
        for category in d["categories"].as_array().expect("categories") {
            self.post("/expense-categories", category.clone()).await;
        }
        if self.customer.is_none() {
            self.customer = Some(
                self.post("/customers", json!({ "name": "Contract client" }))
                    .await["id"]
                    .clone(),
            );
        }
        let customer = self.customer.clone();
        for invoice in d["invoices"].as_array().expect("invoices") {
            let lines: Vec<Value> = invoice["lines"]
                .as_array()
                .expect("lines")
                .iter()
                .map(|l| {
                    let mut line = l.clone();
                    line["tax_treatment"] = json!("standard");
                    line
                })
                .collect();
            let draft = self
                .post(
                    "/invoices",
                    json!({ "customer_id": customer, "issue_date": invoice["issue_date"], "due_date": invoice["due_date"], "lines": lines }),
                )
                .await;
            let id = draft["id"].as_str().expect("id").to_owned();
            let issued = self.post(&format!("/invoices/{id}/issue"), json!({})).await;
            assert_eq!(
                issued["total"], invoice["payment"]["amount"],
                "{}",
                invoice["issue_date"]
            );
            let mut payment = invoice["payment"].clone();
            payment["method"] = json!("bank_transfer");
            self.post(&format!("/invoices/{id}/payments"), payment)
                .await;
        }
        let categories = self.categories().await;
        for expense in d["expenses"].as_array().expect("expenses") {
            let category = categories
                .iter()
                .find(|c| c["name"] == expense["category"])
                .unwrap_or_else(|| panic!("category {}", expense["category"]))["id"]
                .clone();
            let mut body = expense.clone();
            body["category_id"] = category;
            body.as_object_mut().expect("expense").remove("category");
            self.post("/expenses", body).await;
        }
        for movement in d["movements"].as_array().expect("movements") {
            self.post("/cash-movements", movement.clone()).await;
        }
        self.post(
            "/assets/depreciation",
            json!({ "year_end": d["year"]["to"] }),
        )
        .await;
    }

    async fn report(&self, kind: &str, year: &Value) -> Value {
        let query = format!(
            "/reports/{kind}?from={}&to={}&basis=accrual",
            year["from"].as_str().expect("from"),
            year["to"].as_str().expect("to")
        );
        Self::ok(self.get(&query).await, kind)
    }
}

/// Cents to whole dollars, halves away from zero (as annual reports round).
fn dollars(cents: &Value) -> i64 {
    let c = cents
        .as_i64()
        .unwrap_or_else(|| panic!("not money: {cents}"));
    let rounded = (c.abs() + 50) / 100;
    if c < 0 { -rounded } else { rounded }
}

/// Every row and total in the report: (section title, first cell, cells).
fn lines(report: &Value) -> Vec<(String, String, Vec<Value>)> {
    let mut out = Vec::new();
    for section in report["sections"].as_array().expect("sections") {
        let title = section["title"].as_str().unwrap_or_default().to_owned();
        let rows = section["rows"].as_array().cloned().unwrap_or_default();
        for row in rows.into_iter().chain(
            section["totals"]
                .as_array()
                .map(|t| Value::Array(t.clone())),
        ) {
            let cells = row.as_array().expect("cells").clone();
            out.push((
                title.clone(),
                cells[0].as_str().unwrap_or_default().to_owned(),
                cells,
            ));
        }
    }
    out
}

fn line<'a>(all: &'a [(String, String, Vec<Value>)], label: &str) -> &'a [Value] {
    &all.iter()
        .find(|(_, l, _)| l == label)
        .unwrap_or_else(|| {
            panic!(
                "no line “{label}” in {:?}",
                all.iter().map(|l| &l.1).collect::<Vec<_>>()
            )
        })
        .2
}

/// The printed figure in `column` (1 = the year, 2 = the comparative) or zero when a line isn't shown.
fn figure(all: &[(String, String, Vec<Value>)], label: &str, column: usize) -> i64 {
    all.iter()
        .find(|(_, l, _)| l == label)
        .map_or(0, |(_, _, cells)| dollars(&cells[column]))
}

/// Compares every expected figure: a number for the year alone, or `[year, comparative]`.
fn assert_matches(report: &Value, expected: &Value, what: &str) {
    let all = lines(report);
    for (label, value) in expected.as_object().expect("expected figures") {
        let columns: Vec<i64> = match value {
            Value::Array(pair) => pair.iter().map(|v| v.as_i64().expect("figure")).collect(),
            other => vec![
                other
                    .as_i64()
                    .unwrap_or_else(|| panic!("{what} {label}: {other}")),
            ],
        };
        for (i, amount) in columns.iter().enumerate() {
            let column = i + 1;
            if *amount == 0 {
                // A dash in the report: the line may be absent or zero.
                assert_eq!(
                    figure(&all, label, column),
                    0,
                    "{what}: {label} (column {column})"
                );
            } else {
                assert_eq!(
                    dollars(&line(&all, label)[column]),
                    *amount,
                    "{what}: {label} (column {column})"
                );
            }
        }
    }
}

fn assert_schedule(report: &Value, expected: &Value) {
    let all = lines(report);
    for (label, figures) in expected.as_object().expect("depreciation") {
        let cells = line(&all, label);
        assert_eq!(dollars(&cells[1]), figures["cost"], "{label} cost");
        assert_eq!(dollars(&cells[2]), figures["opening"], "{label} opening");
        assert_eq!(dollars(&cells[3]), 0, "{label} purchases");
        assert_eq!(
            dollars(&cells[5]),
            figures["depreciation"],
            "{label} depreciation"
        );
        assert_eq!(dollars(&cells[6]), figures["closing"], "{label} closing");
        assert_eq!(dollars(&cells[7]), 0, "{label} private use");
    }
}

#[tokio::test]
async fn the_fy2025_annual_report_matches_the_expected_figures() {
    let b = Books::fy2025("fy2025").await;
    let d = fixture(FY2025);
    let e = &d["expected"];

    let pnl = b.report("profit_and_loss", &d["year"]).await;
    assert_matches(&pnl, &e["profit_and_loss"], "profit and loss");
    let adjustments = pnl["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .find(|s| s["title"] == "Adjustments")
        .expect("adjustments");
    assert_eq!(
        adjustments["rows"].as_array().expect("rows").len(),
        1,
        "only the penalties are non-deductible"
    );

    // Balance sheet at 31 March 2025, with 31 March 2024 alongside.
    let sheet = b.report("balance_sheet", &d["year"]).await;
    assert_matches(&sheet, &e["balance_sheet"], "balance sheet 2025");
    let all = lines(&sheet);
    for (label, amount) in e["balance_sheet_2024"].as_object().expect("2024") {
        if label == "retained_and_current" {
            // The app's opening balance puts all earlier profit in retained earnings, where an
            // accountant's report may split it into that year's earnings and retained earnings.
            assert_eq!(
                dollars(&line(&all, "Retained earnings")[2]),
                *amount,
                "2024 earnings and retained"
            );
            continue;
        }
        assert_eq!(
            dollars(&line(&all, label)[2]),
            *amount,
            "balance sheet 2024: {label}"
        );
    }
    // The books balance to the cent.
    assert_eq!(line(&all, "Net assets")[1], line(&all, "Total equity")[1]);
    assert_eq!(line(&all, "Net assets")[2], line(&all, "Total equity")[2]);

    assert_matches(
        &b.report("equity_changes", &d["year"]).await,
        &e["equity_changes"],
        "changes in equity",
    );
    assert_schedule(
        &b.report("depreciation_schedule", &d["year"]).await,
        &e["depreciation"],
    );

    let assets = Books::ok(b.get("/assets").await, "assets");
    let book: i64 = assets
        .as_array()
        .expect("assets")
        .iter()
        .map(|a| a["book_value"].as_i64().expect("book value"))
        .sum();
    let checks = &e["checks"];
    assert_eq!(dollars(&json!(book)), checks["book_value"]);
    let opening = Books::ok(b.get("/opening-balances").await, "opening");
    assert_eq!(
        dollars(&opening["retained_earnings"]),
        checks["opening_retained_earnings"]
    );
    assert_eq!(
        dollars(&opening["fixed_assets"]),
        checks["opening_fixed_assets"]
    );
}

#[tokio::test]
async fn the_fy2026_annual_report_matches_the_expected_figures_beside_fy2025() {
    let b = Books::fy2026("fy2026").await;
    let d = fixture(FY2026);
    let e = &d["expected"];

    // Each report carries the year before alongside, as an accountant's does.
    let pnl = b.report("profit_and_loss", &d["year"]).await;
    assert_matches(&pnl, &e["profit_and_loss"], "profit and loss");
    let sheet = b.report("balance_sheet", &d["year"]).await;
    assert_matches(&sheet, &e["balance_sheet"], "balance sheet");
    let all = lines(&sheet);
    assert_eq!(
        line(&all, "Net assets")[1],
        line(&all, "Total equity")[1],
        "the books balance to the cent"
    );
    assert_matches(
        &b.report("equity_changes", &d["year"]).await,
        &e["equity_changes"],
        "changes in equity",
    );

    // Depreciation carries on from FY2025's closing values.
    assert_schedule(
        &b.report("depreciation_schedule", &d["year"]).await,
        &e["depreciation"],
    );
    let assets = Books::ok(b.get("/assets").await, "assets");
    let book: i64 = assets
        .as_array()
        .expect("assets")
        .iter()
        .map(|a| a["book_value"].as_i64().expect("book value"))
        .sum();
    let checks = &e["checks"];
    assert_eq!(dollars(&json!(book)), checks["book_value"]);

    // Last year's profit has moved into retained earnings; this year's earnings are this year's alone.
    assert_eq!(
        dollars(&line(&all, "Retained earnings")[1]),
        checks["retained_earnings"]
    );
    assert_eq!(
        dollars(&line(&all, "Current year earnings")[1]),
        checks["current_year_earnings"]
    );
}

#[tokio::test]
async fn home_office_costs_paid_personally_are_funds_introduced_for_the_business_share_only() {
    let b = Books::fy2026("fy2026-home").await;
    let d = fixture(FY2026);
    let checks = &d["expected"]["checks"];
    // Household internet each month, partly for the business, paid from the owner's own account.
    let internet = &checks["home_office_internet"];
    let gross = internet["monthly_gross"].as_i64().expect("gross");
    let share = internet["business_percentage"].as_i64().expect("share");
    let net = gross - (gross * 3 + 11) / 23; // GST is 3/23 of the inclusive amount, rounded
    let business_share = (net * share + 50) / 100;
    let pnl = lines(&b.report("profit_and_loss", &d["year"]).await);
    assert_eq!(line(&pnl, "Home Office- Internet")[1], 12 * business_share);
    assert_eq!(
        line(&pnl, "Home Office- Internet")[1],
        checks["home_office_internet_cents"]
    );
    let equity = lines(&b.report("equity_changes", &d["year"]).await);
    // The contributions, plus the business share of the internet with its GST.
    assert_eq!(
        line(&equity, "Owner funds introduced")[1],
        checks["owner_funds_introduced_cents"]
    );
    // The private share never went through the business, so it isn't a drawing: drawings are the transfers.
    let transfers: i64 = d["movements"]
        .as_array()
        .expect("movements")
        .iter()
        .filter(|m| m["kind"] == "drawing")
        .map(|m| m["amount"].as_i64().expect("amount"))
        .sum();
    assert_eq!(line(&equity, "Drawings")[1], transfers);
    assert_eq!(line(&equity, "Drawings")[1], checks["drawings_cents"]);
}

#[tokio::test]
async fn the_annual_accounts_export_for_the_accountant() {
    let b = Books::fy2025("fy2025-export").await;
    for kind in [
        "balance_sheet",
        "equity_changes",
        "depreciation_schedule",
        "profit_and_loss",
    ] {
        for (format, magic) in [("pdf", &b"%PDF-"[..]), ("xlsx", &b"PK"[..])] {
            let response = b.get(&format!("/reports/{kind}/export?format={format}&from=2024-04-01&to=2025-03-31&basis=accrual")).await;
            assert_eq!(response.status, StatusCode::OK, "{kind} {format}");
            assert!(response.bytes.starts_with(magic), "{kind} {format}");
        }
    }
    let csv = b
        .get("/reports/balance_sheet/export?format=csv&from=2024-04-01&to=2025-03-31&basis=accrual")
        .await;
    let text = String::from_utf8(csv.bytes).expect("utf-8");
    assert!(text.contains("Withholding tax paid"), "{text}");
    let net_assets = fixture(FY2025)["expected"]["checks"]["net_assets_csv"].clone();
    assert!(
        text.contains(net_assets.as_str().expect("net assets")),
        "{text}"
    );
}

#[tokio::test]
async fn reposting_and_removing_depreciation_restores_the_books() {
    let b = Books::fy2025("fy2025-redo").await;
    let d = fixture(FY2025);
    let year = d["year"].clone();
    let checks = &d["expected"]["checks"];
    let before = lines(&b.report("balance_sheet", &year).await);

    // Posting the same year again changes nothing.
    b.post("/assets/depreciation", json!({ "year_end": "2025-03-31" }))
        .await;
    let again = lines(&b.report("balance_sheet", &year).await);
    assert_eq!(line(&again, "Total equity"), line(&before, "Total equity"));

    // Removing it puts the year's depreciation back on the assets and the profit.
    let removed = b
        .app
        .delete(
            &b.path("/assets/depreciation?year_end=2025-03-31"),
            &b.user.auth(),
            None,
        )
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{}", removed.body);
    let after = lines(&b.report("balance_sheet", &year).await);
    assert_eq!(
        line(&after, "Property, plant and equipment")[1],
        checks["opening_fixed_assets_cents"]
    );
    assert_eq!(
        line(&after, "Total equity")[1].as_i64().expect("equity")
            - line(&before, "Total equity")[1].as_i64().expect("equity"),
        checks["depreciation_cents"]
    );
    let schedule = b.report("depreciation_schedule", &year).await;
    assert!(
        schedule["notes"]
            .to_string()
            .contains("No depreciation has been posted")
    );
}

#[tokio::test]
async fn removing_an_earlier_year_of_depreciation_removes_the_years_after_it() {
    let b = Books::fy2026("fy2026-undo").await;
    let removed = b
        .app
        .delete(
            &b.path("/assets/depreciation?year_end=2025-03-31"),
            &b.user.auth(),
            None,
        )
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{}", removed.body);
    let fy2026 = Books::ok(
        b.get("/assets/depreciation?year_end=2026-03-31").await,
        "2026",
    );
    assert_eq!(
        fy2026["lines"],
        json!([]),
        "FY2026 depended on FY2025's closing values"
    );
    // Posting FY2026 alone isn't possible: FY2025 must come first.
    let skipped = b
        .app
        .post(
            &b.path("/assets/depreciation"),
            &b.user.auth(),
            json!({ "year_end": "2026-03-31" }),
        )
        .await;
    assert_eq!(skipped.status, StatusCode::CONFLICT, "{}", skipped.body);
    // Re-posting both restores the schedule exactly.
    b.post("/assets/depreciation", json!({ "year_end": "2025-03-31" }))
        .await;
    b.post("/assets/depreciation", json!({ "year_end": "2026-03-31" }))
        .await;
    let d = fixture(FY2026);
    assert_schedule(
        &b.report("depreciation_schedule", &d["year"]).await,
        &d["expected"]["depreciation"],
    );
}
