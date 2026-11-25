//! Income tax and provisional tax worked by hand, for a sole trader not registered for GST whose
//! customer withholds 10% (a schedular payment), with a 31 March balance date:
//!
//! FY2025 (1 Apr 2024 – 31 Mar 2025, the 2024-25 composite rates): $100,000 invoiced and paid with
//! $10,000 withheld; $10,000 of deductible expenses and a $500 penalty (not deductible). Taxable
//! income $90,000; tax 1,470 + 205.12 + 5,670 + 1,190.20 + 4,950 + 2,510.19 + 3,927 = $19,922.51;
//! RIT $9,922.51, due 9 February 2026 (the 7th is a Saturday).
//!
//! FY2026 (rates from 1 April 2025): $60,000 with $6,000 withheld. Tax 1,638 + 6,632.50 + 1,950 =
//! $10,220.50; RIT $4,220.50. Provisional tax (FY2025's RIT over $5,000): $9,922.51 × 1.05 =
//! $10,418.63, in whole dollars $10,418, in three instalments: $3,472.66 on 28 August 2025, $3,472.66
//! on 15 January 2026 and $3,472.68 on 7 May 2026. Terminal tax: 4,220.50 − 10,418 = a $6,197.50 refund.

mod common;

use axum::http::StatusCode;
use common::{TestApp, TestUser};
use serde_json::{Value, json};

struct Books {
    app: TestApp,
    user: TestUser,
    base: String,
}

impl Books {
    async fn new(prefix: &str) -> Self {
        let app = TestApp::new().await;
        let user = app.register(prefix).await;
        let created = app
            .post(
                "/api/v1/businesses",
                &user.auth(),
                json!({ "name": format!("{prefix} Ltd"), "reporting_basis": "accrual", "complete_onboarding": true }),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
        let base = format!(
            "/api/v1/businesses/{}",
            created.body["id"].as_str().expect("id")
        );
        Self { app, user, base }
    }

    async fn ok(&self, method: &str, path: &str, body: Value) -> Value {
        let path = format!("{}{path}", self.base);
        let response = match method {
            "GET" => self.app.get(&path, &self.user.auth()).await,
            _ => self.app.post(&path, &self.user.auth(), body).await,
        };
        assert!(
            response.status.is_success(),
            "{method} {path}: {} {}",
            response.status,
            response.body
        );
        response.body
    }

    async fn paid_invoice(&self, customer: &Value, date: &str, dollars: i64, withheld: i64) {
        let draft = self
            .ok(
                "POST",
                "/invoices",
                json!({ "customer_id": customer["id"], "issue_date": date, "due_date": date,
                        "lines": [{ "description": "Contract work", "quantity": "1", "unit_price": dollars * 100, "tax_treatment": "no_gst" }] }),
            )
            .await;
        let id = draft["id"].as_str().expect("id");
        self.ok("POST", &format!("/invoices/{id}/issue"), json!({}))
            .await;
        self.ok(
            "POST",
            &format!("/invoices/{id}/payments"),
            json!({ "payment_date": date, "amount": dollars * 100, "withholding_tax": withheld * 100, "method": "bank_transfer" }),
        )
        .await;
    }

    async fn expense(&self, category: &Value, date: &str, dollars: i64) {
        self.ok(
            "POST",
            "/expenses",
            json!({ "expense_date": date, "supplier": "Supplier", "category_id": category["id"], "gross_amount": dollars * 100,
                    "gst_treatment": "no_gst", "payment_method": "business_account", "classification": "business" }),
        )
        .await;
    }
}

fn year<'a>(tax: &'a Value, label: &str) -> &'a Value {
    tax["years"]
        .as_array()
        .expect("years")
        .iter()
        .find(|y| y["label"] == label)
        .unwrap_or_else(|| panic!("no {label}: {tax}"))
}

#[tokio::test]
async fn income_tax_provisional_tax_and_terminal_tax_by_year() {
    let b = Books::new("income-tax").await;
    let customer = b
        .ok("POST", "/customers", json!({ "name": "Agency" }))
        .await;
    let categories = b.ok("GET", "/expense-categories", json!(null)).await;
    let deductible = categories[0].clone();
    let penalties = b
        .ok(
            "POST",
            "/expense-categories",
            json!({ "name": "IRD penalties", "non_deductible": true }),
        )
        .await;
    b.paid_invoice(&customer, "2024-06-20", 100_000, 10_000)
        .await;
    b.expense(&deductible, "2024-07-01", 10_000).await;
    b.expense(&penalties, "2024-08-01", 500).await;
    b.paid_invoice(&customer, "2025-05-20", 60_000, 6_000).await;

    let tax = b.ok("GET", "/income-tax", json!(null)).await;
    assert_eq!(tax["basis"], "accrual");
    assert_eq!(
        tax["years"][0]["label"], "FY2027",
        "newest first, up to the current year"
    );

    let fy25 = year(&tax, "FY2025");
    assert_eq!(
        (
            &fy25["income"],
            &fy25["deductible_expenses"],
            &fy25["taxable_income"]
        ),
        (&json!(10_000_000), &json!(1_000_000), &json!(9_000_000)),
        "the penalty doesn't reduce taxable income"
    );
    assert_eq!(fy25["income_tax"], 1_992_251);
    assert_eq!(fy25["withholding_tax"], 1_000_000);
    assert_eq!(fy25["residual_income_tax"], 992_251);
    assert_eq!(fy25["provisional_tax"], Value::Null, "no year before it");
    assert_eq!(
        fy25["payments"],
        json!([{ "kind": "terminal", "label": "Terminal tax", "due_date": "2026-02-09", "amount": 992_251,
                 "settled": 0, "outstanding": 992_251, "status": "overdue" }])
    );

    let fy26 = year(&tax, "FY2026");
    assert_eq!(fy26["income_tax"], 1_022_050);
    assert_eq!(fy26["residual_income_tax"], 422_050);
    assert_eq!(fy26["provisional_tax"], 1_041_800);
    let payments = fy26["payments"].as_array().expect("payments");
    let summary: Vec<(String, String, i64)> = payments
        .iter()
        .map(|p| {
            (
                p["label"].as_str().unwrap_or_default().to_owned(),
                p["due_date"].as_str().unwrap_or_default().to_owned(),
                p["amount"].as_i64().unwrap_or_default(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("Instalment 1 of 3".into(), "2025-08-28".into(), 347_266),
            ("Instalment 2 of 3".into(), "2026-01-15".into(), 347_266),
            ("Instalment 3 of 3".into(), "2026-05-07".into(), 347_268),
            ("Refund".into(), "2027-02-09".into(), -619_750),
        ]
    );
    assert_eq!(payments[3]["status"], "refund_due");
    assert_eq!(
        year(&tax, "FY2027")["provisional_tax"],
        Value::Null,
        "FY2026's RIT was under $5,000"
    );

    // Paying everything due: payments settle the oldest due date first.
    b.ok(
        "POST",
        "/cash-movements",
        json!({ "kind": "income_tax_payment", "movement_date": "2026-02-05", "amount": 2_034_051, "description": "Income tax" }),
    )
    .await;
    let tax = b.ok("GET", "/income-tax", json!(null)).await;
    assert_eq!(tax["overdue"], 0);
    assert!(
        year(&tax, "FY2026")["payments"]
            .as_array()
            .expect("payments")[..3]
            .iter()
            .all(|p| p["status"] == "paid")
    );
    assert_eq!(year(&tax, "FY2025")["payments"][0]["status"], "paid");
    assert_eq!(tax["refunds_due"], 619_750);
    assert_eq!(tax["credit"], 0);
}

#[tokio::test]
async fn another_business_cannot_see_the_income_tax() {
    let b = Books::new("income-tax-private").await;
    let stranger = b.app.register("income-tax-stranger").await;
    let response = b
        .app
        .get(&format!("{}/income-tax", b.base), &stranger.auth())
        .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
}
