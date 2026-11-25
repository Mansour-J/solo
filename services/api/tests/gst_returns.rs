//! GST returns worked by hand. A business registered from 1 Jan 2026, filing two-monthly with periods
//! ending in odd months (Jan, Mar, May …):
//! - Invoice 1: $1,000 + $150 GST, issued 10 Apr, paid 5 Jun.
//! - Expense: $115 incl. $15 GST, 5 May, paid from the business account.
//! - Invoice 2: $400 + $60 GST, issued 15 Jun, unpaid.
//! - A GST payment of $135 on 20 Jun.
//!
//! Invoice basis: Apr–May $150 − $15 = $135 (due 29 Jun, the 28th being a Sunday), paid by the $135;
//! Jun–Jul $60, overdue. Payments basis: Apr–May −$15 (a refund due); Jun–Jul $150 (invoice 1's
//! payment), of which the $135 payment settles $135, leaving $15 overdue.

mod common;

use axum::http::StatusCode;
use common::{TestApp, TestUser};
use serde_json::{Value, json};

struct Gst {
    app: TestApp,
    user: TestUser,
    base: String,
    business: Value,
}

impl Gst {
    async fn new(prefix: &str, registered: bool) -> Self {
        let app = TestApp::new().await;
        let user = app.register(prefix).await;
        let created = app
            .post(
                "/api/v1/businesses",
                &user.auth(),
                json!({ "name": format!("{prefix} Ltd"), "gst_registered": registered, "gst_number": if registered { json!("49091850") } else { json!(null) },
                        "gst_registered_from": "2026-01-01", "gst_basis": "invoice", "gst_period": "two_monthly",
                        "gst_period_end_month": 3, "complete_onboarding": true }),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
        let base = format!(
            "/api/v1/businesses/{}",
            created.body["id"].as_str().expect("id")
        );
        Self {
            app,
            user,
            base,
            business: created.body,
        }
    }

    async fn ok(&self, method: &str, path: &str, body: Value) -> Value {
        let path = format!("{}{path}", self.base);
        let response = match method {
            "GET" => self.app.get(&path, &self.user.auth()).await,
            "PUT" => self.app.put(&path, &self.user.auth(), body).await,
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

    async fn seed(&self) {
        let customer = self
            .ok("POST", "/customers", json!({ "name": "Tūī Media" }))
            .await;
        let line = |dollars: i64| json!([{ "description": "Work", "quantity": "1", "unit_price": dollars * 100, "tax_treatment": "standard" }]);
        let mut invoices = Vec::new();
        for (date, dollars) in [("2026-04-10", 1_000), ("2026-06-15", 400)] {
            let draft = self
                .ok(
                    "POST",
                    "/invoices",
                    json!({ "customer_id": customer["id"], "issue_date": date, "due_date": date, "lines": line(dollars) }),
                )
                .await;
            let id = draft["id"].as_str().expect("id").to_owned();
            self.ok("POST", &format!("/invoices/{id}/issue"), json!({}))
                .await;
            invoices.push(id);
        }
        self.ok(
            "POST",
            &format!("/invoices/{}/payments", invoices[0]),
            json!({ "payment_date": "2026-06-05", "amount": 115_000, "method": "bank_transfer" }),
        )
        .await;
        let categories = self.ok("GET", "/expense-categories", json!(null)).await;
        let category = categories
            .as_array()
            .expect("categories")
            .iter()
            .find(|c| c["name"] == "Office expenses")
            .unwrap_or(&categories[0])["id"]
            .clone();
        self.ok(
            "POST",
            "/expenses",
            json!({ "expense_date": "2026-05-05", "supplier": "Stationer", "category_id": category, "gross_amount": 11_500,
                    "gst_treatment": "standard", "payment_method": "business_account", "classification": "business" }),
        )
        .await;
        self.ok(
            "POST",
            "/cash-movements",
            json!({ "kind": "gst_payment", "movement_date": "2026-06-20", "amount": 13_500, "description": "GST return" }),
        )
        .await;
    }

    async fn returns(&self) -> Value {
        self.ok("GET", "/gst/returns", json!(null)).await
    }
}

fn period<'a>(returns: &'a Value, end: &str) -> &'a Value {
    returns["returns"]
        .as_array()
        .expect("returns")
        .iter()
        .find(|r| r["end"] == end)
        .unwrap_or_else(|| panic!("no period ending {end}: {returns}"))
}

#[tokio::test]
async fn each_period_shows_its_due_date_what_to_pay_and_what_was_paid() {
    let g = Gst::new("gst-returns", true).await;
    g.seed().await;
    let r = g.returns().await;
    assert_eq!(r["gst_period"], "two_monthly");
    let first = &r["returns"][0];
    assert_eq!(
        (first["start"].as_str(), first["end"].as_str()),
        (Some("2026-04-01"), Some("2026-05-31")),
        "periods start at the first GST record"
    );
    assert_eq!(first["due_date"], "2026-06-29");
    assert_eq!(
        (&first["collected"], &first["claimable"], &first["net"]),
        (&json!(15_000), &json!(1_500), &json!(13_500))
    );
    assert_eq!(
        (&first["settled"], &first["status"]),
        (&json!(13_500), &json!("paid"))
    );

    let second = period(&r, "2026-07-31");
    assert_eq!(second["due_date"], "2026-08-28");
    assert_eq!(
        (&second["net"], &second["outstanding"]),
        (&json!(6_000), &json!(6_000))
    );
    assert_eq!(second["status"], "overdue");
    assert_eq!(period(&r, "2026-09-30")["status"], "nil");
    let last = r["returns"]
        .as_array()
        .expect("returns")
        .last()
        .expect("current");
    assert_eq!(
        last["status"], "in_progress",
        "ends with the period containing today"
    );
    assert_eq!(
        (&r["owing_now"], &r["overdue"]),
        (&json!(6_000), &json!(6_000))
    );
    assert_eq!(r["next_due"]["end"], "2026-07-31");
    assert_eq!(r["next_due"]["amount"], 6_000);

    // On the payments basis, invoice 1's GST counts when it was paid (June).
    let mut input = g.business.clone();
    input["gst_basis"] = json!("payments");
    g.ok("PUT", "", input).await;
    let r = g.returns().await;
    let first = period(&r, "2026-05-31");
    assert_eq!(
        (&first["net"], &first["status"]),
        (&json!(-1_500), &json!("refund_due"))
    );
    let second = period(&r, "2026-07-31");
    assert_eq!(second["net"], 15_000);
    assert_eq!(
        (&second["settled"], &second["outstanding"]),
        (&json!(13_500), &json!(1_500)),
        "the payment settles the oldest GST owed; a refund period isn't paid by it"
    );
    assert_eq!(
        (&r["owing_now"], &r["refunds_due"]),
        (&json!(1_500), &json!(1_500))
    );
}

#[tokio::test]
async fn the_return_period_is_a_business_setting() {
    let g = Gst::new("gst-period", true).await;
    g.seed().await;
    let mut input = g.business.clone();
    input["gst_period"] = json!("six_monthly");
    let saved = g.ok("PUT", "", input.clone()).await;
    assert_eq!(saved["gst_period"], "six_monthly");
    let r = g.returns().await;
    let first = &r["returns"][0];
    assert_eq!(
        (first["start"].as_str(), first["end"].as_str()),
        (Some("2026-04-01"), Some("2026-09-30"))
    );
    assert_eq!(first["due_date"], "2026-10-28");
    input["gst_period_end_month"] = json!(13);
    let invalid = g.app.put(&g.base, &g.user.auth(), input).await;
    assert_eq!(
        invalid.field_error("gst_period_end_month"),
        Some("Choose a month.")
    );
}

#[tokio::test]
async fn a_business_without_gst_has_no_returns_and_others_cannot_see_them() {
    let g = Gst::new("gst-none", false).await;
    let r = g.returns().await;
    assert_eq!(r["gst_registered"], false);
    assert_eq!(r["returns"], json!([]));

    let registered = Gst::new("gst-private", true).await;
    let stranger = registered.app.register("gst-stranger").await;
    let response = registered
        .app
        .get(
            &format!("{}/gst/returns", registered.base),
            &stranger.auth(),
        )
        .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn gst_owed_at_the_opening_balances_is_paid_first() {
    let g = Gst::new("gst-opening", true).await;
    g.ok(
        "PUT",
        "/opening-balances",
        json!({ "as_at": "2026-03-31", "bank": 100_000, "gst_payable": 5_000 }),
    )
    .await;
    g.seed().await;
    let r = g.returns().await;
    assert_eq!(
        r["brought_forward"],
        json!({ "as_at": "2026-03-31", "amount": 5_000, "settled": 5_000, "outstanding": 0 })
    );
    let first = &r["returns"][0];
    assert_eq!(
        first["start"], "2026-04-01",
        "returns start after the opening balances"
    );
    assert_eq!(
        (&first["settled"], &first["outstanding"], &first["status"]),
        (&json!(8_500), &json!(5_000), &json!("overdue")),
        "the $135 payment settled the $50 brought forward first"
    );
}
