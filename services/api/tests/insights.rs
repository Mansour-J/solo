//! Insights against a hand-worked data set placed relative to the business's today (the rules read
//! "last month", "the last three months", "overdue"…). Months are named M1 (last complete month) to
//! M6 (six months back). The business isn't GST-registered, on the accrual basis.
//!
//! - Tūī Media: $1,000 invoices issued in M6, M5 and M4 (paid after 10, 20 and 30 days) and $2,000
//!   in M2, due on the 20th and unpaid (overdue).
//! - Kākā Studio: $500 in M3 (paid after 20 days) and $1,500 in M1, due in 10 days (not yet due).
//! - Software (Xero): $50 in each of M4 – M1. Advertising (Meta): $100 in each of M6 – M2, $400 in M1.
//!
//! So: revenue M1 $1,500 against M2 $2,000 (25% lower); the last three months $4,000 against $3,000
//! (33% higher); Tūī Media brought in 71% ($5,000 of $7,000); the average invoice is $1,166.67
//! ($7,000 / 6); customers take 20 days on average to pay ((10 + 20 + 30 + 20) / 4); advertising is
//! the biggest expense over three months ($600, 80% of $750), grew 100%, and M1's $400 is unusual
//! against its usual $100; Xero is a regular $50 a month; $3,500 is owed, $2,000 of it overdue, and
//! $1,500 is expected within 30 days.

mod common;

use axum::http::StatusCode;
use chrono::{Datelike, Duration, Months, NaiveDate};
use common::{TestApp, TestUser};
use serde_json::{Value, json};
use uuid::Uuid;

struct Books {
    app: TestApp,
    user: TestUser,
    business: Uuid,
    today: NaiveDate,
}

impl Books {
    async fn new(prefix: &str) -> Self {
        let app = TestApp::new().await;
        let user = app.register(prefix).await;
        let business = app.create_business(&user, &format!("{prefix} Ltd")).await;
        let mut books = Self {
            app,
            user,
            business,
            today: NaiveDate::MIN,
        };
        let today = books.get("").await.body["today"]
            .as_str()
            .expect("today")
            .to_owned();
        books.today = today.parse().expect("date");
        books
    }

    fn path(&self, rest: &str) -> String {
        format!("/api/v1/businesses/{}{rest}", self.business)
    }

    async fn get(&self, rest: &str) -> common::TestResponse {
        self.app.get(&self.path(rest), &self.user.auth()).await
    }

    async fn post(&self, rest: &str, body: Value) -> common::TestResponse {
        let response = self
            .app
            .post(&self.path(rest), &self.user.auth(), body)
            .await;
        assert!(
            response.status.is_success(),
            "{rest}: {} {}",
            response.status,
            response.body
        );
        response
    }

    /// Day `day` of the month `n` months before this one.
    fn month(&self, n: u32, day: u32) -> NaiveDate {
        let first = self.today.with_day(1).expect("first");
        first
            .checked_sub_months(Months::new(n))
            .expect("month")
            .with_day(day)
            .expect("day")
    }

    async fn customer(&self, name: &str) -> String {
        self.post("/customers", json!({ "name": name })).await.body["id"]
            .as_str()
            .expect("id")
            .to_owned()
    }

    async fn invoice(
        &self,
        customer: &str,
        issued: NaiveDate,
        due: NaiveDate,
        amount: i64,
    ) -> String {
        let draft = self
            .post(
                "/invoices",
                json!({ "customer_id": customer, "issue_date": issued, "due_date": due,
                        "lines": [{ "description": "Work", "quantity": "1", "unit_price": amount, "tax_treatment": "no_gst" }] }),
            )
            .await;
        let id = draft.body["id"].as_str().expect("id").to_owned();
        self.post(&format!("/invoices/{id}/issue"), json!({})).await;
        id
    }

    async fn pay(&self, invoice: &str, date: NaiveDate, amount: i64) {
        self.post(
            &format!("/invoices/{invoice}/payments"),
            json!({ "payment_date": date, "amount": amount, "method": "bank_transfer" }),
        )
        .await;
    }

    async fn expense(&self, category: &str, supplier: &str, date: NaiveDate, amount: i64) {
        let categories = self.get("/expense-categories").await.body;
        let id = categories
            .as_array()
            .expect("categories")
            .iter()
            .find(|c| c["name"] == category)
            .expect("category")["id"]
            .clone();
        self.post(
            "/expenses",
            json!({ "expense_date": date, "supplier": supplier, "category_id": id, "gross_amount": amount,
                    "gst_treatment": "no_gst", "payment_method": "business_account", "classification": "business" }),
        )
        .await;
    }

    async fn seed(&self) {
        let tui = self.customer("Tūī Media").await;
        let kaka = self.customer("Kākā Studio").await;
        for (n, days) in [(6, 10), (5, 20), (4, 30)] {
            let issued = self.month(n, 5);
            let invoice = self
                .invoice(&tui, issued, issued + Duration::days(14), 100_000)
                .await;
            self.pay(&invoice, issued + Duration::days(days), 100_000)
                .await;
        }
        let issued = self.month(3, 5);
        let invoice = self
            .invoice(&kaka, issued, issued + Duration::days(14), 50_000)
            .await;
        self.pay(&invoice, issued + Duration::days(20), 50_000)
            .await;
        self.invoice(&tui, self.month(2, 5), self.month(2, 20), 200_000)
            .await;
        self.invoice(
            &kaka,
            self.month(1, 5),
            self.today + Duration::days(10),
            150_000,
        )
        .await;
        for n in 1..=4 {
            self.expense("Software", "Xero", self.month(n, 10), 5_000)
                .await;
        }
        for n in 2..=6 {
            self.expense("Advertising", "Meta", self.month(n, 10), 10_000)
                .await;
        }
        self.expense("Advertising", "Meta", self.month(1, 10), 40_000)
            .await;
    }

    async fn insights(&self) -> Value {
        let response = self.get("/insights").await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.body);
        response.body
    }
}

fn item<'a>(insights: &'a Value, key: &str) -> &'a Value {
    insights["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .flat_map(|s| s["items"].as_array().expect("items"))
        .find(|i| i["key"] == key)
        .unwrap_or_else(|| panic!("insight {key}"))
}

fn has_item(insights: &Value, key: &str) -> bool {
    insights["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .flat_map(|s| s["items"].as_array().expect("items"))
        .any(|i| i["key"] == key)
}

fn figure(item: &Value) -> i64 {
    item["figure"]["amount"].as_i64().expect("figure")
}

fn detail(item: &Value, label: &str) -> i64 {
    item["details"]
        .as_array()
        .expect("details")
        .iter()
        .find(|d| d["label"] == label)
        .unwrap_or_else(|| panic!("detail {label}"))["figure"]["amount"]
        .as_i64()
        .expect("amount")
}

#[tokio::test]
async fn insights_match_the_hand_worked_figures() {
    let b = Books::new("insights-figures").await;
    b.seed().await;
    let insights = b.insights().await;
    assert_eq!(insights["basis"], "accrual");

    // Performance.
    let month = item(&insights, "revenue-month");
    assert_eq!(figure(month), -25);
    assert!(
        month["statement"]
            .as_str()
            .expect("text")
            .contains("25% lower")
    );
    assert_eq!(month["tone"], "negative");
    assert_eq!(figure(item(&insights, "revenue-growth")), 33);
    assert_eq!(figure(item(&insights, "expenses-month")), 200);

    // Business health.
    let concentration = item(&insights, "concentration");
    assert_eq!(figure(concentration), 71);
    assert_eq!(concentration["tone"], "warning");
    assert_eq!(detail(concentration, "Tūī Media"), 500_000);
    assert_eq!(detail(concentration, "Kākā Studio"), 200_000);
    let overdue = item(&insights, "overdue-customers");
    assert_eq!(figure(overdue), 200_000);
    assert_eq!(detail(overdue, "Tūī Media"), 200_000);
    assert_eq!(figure(item(&insights, "invoice-value")), 116_667);
    let paid = item(&insights, "payment-time");
    assert_eq!(paid["figure"], json!({ "kind": "days", "amount": 20 }));

    // Spending.
    let biggest = item(&insights, "biggest-categories");
    assert_eq!(detail(biggest, "Advertising"), 60_000);
    assert!(biggest["statement"].as_str().expect("text").contains("80%"));
    assert_eq!(
        detail(item(&insights, "growing-categories"), "Advertising"),
        100
    );
    let unusual = item(&insights, "unusual-spending");
    assert_eq!(figure(unusual), 40_000);
    assert!(
        unusual["statement"]
            .as_str()
            .expect("text")
            .contains("$100.00")
    );
    assert_eq!(detail(item(&insights, "recurring"), "Xero"), 5_000);

    // Cash flow: recorded figures and an estimate, told apart.
    let owed = item(&insights, "receivables");
    assert_eq!(figure(owed), 350_000);
    assert_eq!(owed["estimate"], false);
    let expected = item(&insights, "expected-payments");
    assert_eq!(figure(expected), 150_000);
    assert_eq!(expected["estimate"], true);

    // Notices.
    let notices: Vec<&str> = insights["notices"]
        .as_array()
        .expect("notices")
        .iter()
        .filter_map(|n| n["message"].as_str())
        .collect();
    assert!(
        notices.contains(&"You have 1 overdue invoice ($2,000.00)."),
        "{notices:?}"
    );
    assert!(
        notices.iter().any(|n| n.contains("25% lower")),
        "{notices:?}"
    );
    assert!(
        notices
            .iter()
            .any(|n| n.starts_with("Spending on Advertising in") && n.contains("300% higher")),
        "{notices:?}"
    );
}

#[tokio::test]
async fn nothing_is_shown_without_enough_data() {
    let b = Books::new("insights-empty").await;
    let insights = b.insights().await;
    for section in insights["sections"].as_array().expect("sections") {
        assert_eq!(
            section["items"].as_array().map(Vec::len),
            Some(0),
            "{}",
            section["key"]
        );
        assert!(section["empty_reason"].is_string());
    }
    assert_eq!(insights["notices"].as_array().map(Vec::len), Some(0));

    // One small invoice isn't enough for averages, concentration or trends.
    let customer = b.customer("Pūkeko Builders").await;
    let invoice = b
        .invoice(&customer, b.month(1, 5), b.month(1, 19), 20_000)
        .await;
    b.pay(&invoice, b.month(1, 12), 20_000).await;
    let insights = b.insights().await;
    for key in [
        "concentration",
        "invoice-value",
        "payment-time",
        "revenue-month",
        "profit-trend",
        "monthly-averages",
    ] {
        assert!(!has_item(&insights, key), "{key} shown without enough data");
    }
}

#[tokio::test]
async fn notices_can_be_dismissed_and_come_back_when_they_change() {
    let b = Books::new("insights-dismiss").await;
    let customer = b.customer("Tūī Media").await;
    b.invoice(&customer, b.month(2, 5), b.month(2, 20), 100_000)
        .await;
    let first = b.insights().await;
    let notice = first["notices"]
        .as_array()
        .expect("notices")
        .iter()
        .find(|n| n["key"].as_str().is_some_and(|k| k.starts_with("overdue:")))
        .expect("overdue notice")
        .clone();
    b.app
        .post(
            &b.path("/insights/dismiss"),
            &b.user.auth(),
            json!({ "key": notice["key"] }),
        )
        .await;
    let after = b.insights().await;
    assert!(
        after["notices"]
            .as_array()
            .expect("notices")
            .iter()
            .all(|n| n["key"] != notice["key"])
    );
    assert_eq!(after["dismissed_count"], 1);

    // Another overdue invoice changes the notice, so it's shown again.
    b.invoice(&customer, b.month(2, 6), b.month(2, 21), 50_000)
        .await;
    let changed = b.insights().await;
    assert!(
        changed["notices"]
            .as_array()
            .expect("notices")
            .iter()
            .any(|n| n["message"] == "You have 2 overdue invoices ($1,500.00).")
    );

    let empty = b
        .app
        .post(
            &b.path("/insights/dismiss"),
            &b.user.auth(),
            json!({ "key": " " }),
        )
        .await;
    assert!(empty.field_error("key").is_some());
    let restored = b
        .app
        .post(&b.path("/insights/restore"), &b.user.auth(), json!({}))
        .await;
    assert_eq!(restored.status, StatusCode::NO_CONTENT);
    assert_eq!(b.insights().await["dismissed_count"], 0);

    let export = b.app.get("/api/v1/users/me/export", &b.user.auth()).await;
    assert!(export.body["businesses"][0]["records"]["insight_dismissals"].is_array());
}

#[tokio::test]
async fn insights_are_private_to_their_business() {
    let a = Books::new("insights-tenant-a").await;
    a.seed().await;
    let b = Books::new("insights-tenant-b").await;
    let theirs = b.insights().await;
    assert!(!has_item(&theirs, "concentration"));
    assert!(!has_item(&theirs, "receivables"));
    let direct = b
        .app
        .get(
            &format!("/api/v1/businesses/{}/insights", a.business),
            &b.user.auth(),
        )
        .await;
    assert!(
        matches!(direct.status, StatusCode::NOT_FOUND | StatusCode::FORBIDDEN),
        "{}",
        direct.status
    );
}
