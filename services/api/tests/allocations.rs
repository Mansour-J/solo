//! Allocations: destinations, rules, review and confirmation of receipts, balances, and every
//! correction path (edits, deletions, user reversals) without double counting.

mod common;

use axum::http::StatusCode;
use common::{TestApp, TestUser};
use serde_json::{Value, json};
use uuid::Uuid;

struct Plan {
    app: TestApp,
    user: TestUser,
    business: Uuid,
}

impl Plan {
    async fn new(prefix: &str) -> Self {
        let app = TestApp::new().await;
        let user = app.register(prefix).await;
        let business = app.create_business(&user, &format!("{prefix} Ltd")).await;
        Self {
            app,
            user,
            business,
        }
    }

    fn path(&self, rest: &str) -> String {
        format!("/api/v1/businesses/{}{rest}", self.business)
    }

    async fn get(&self, rest: &str) -> common::TestResponse {
        self.app.get(&self.path(rest), &self.user.auth()).await
    }

    async fn post(&self, rest: &str, body: Value) -> common::TestResponse {
        self.app
            .post(&self.path(rest), &self.user.auth(), body)
            .await
    }

    async fn put(&self, rest: &str, body: Value) -> common::TestResponse {
        self.app
            .put(&self.path(rest), &self.user.auth(), body)
            .await
    }

    async fn delete(&self, rest: &str, body: Value) -> common::TestResponse {
        self.app
            .delete(&self.path(rest), &self.user.auth(), Some(body))
            .await
    }

    async fn destination(&self, name: &str) -> String {
        let list = self.get("/allocations/destinations").await;
        list.body
            .as_array()
            .expect("destinations")
            .iter()
            .find(|d| d["name"] == name)
            .and_then(|d| d["id"].as_str())
            .unwrap_or_else(|| panic!("destination {name}"))
            .to_owned()
    }

    async fn rule(&self, body: Value) -> String {
        let rule = self.post("/allocations/rules", body).await;
        assert_eq!(rule.status, StatusCode::CREATED, "{}", rule.body);
        rule.body["id"].as_str().expect("id").to_owned()
    }

    async fn income(&self, amount: i64, date: &str) -> String {
        let income = self
            .post(
                "/income",
                json!({ "income_date": date, "payer": "Market stall", "income_type": "sales",
                        "gross_amount": amount, "gst_treatment": "no_gst", "payment_method": "cash" }),
            )
            .await;
        assert_eq!(income.status, StatusCode::CREATED, "{}", income.body);
        income.body["id"].as_str().expect("id").to_owned()
    }

    /// Every allocation of an income record, newest first.
    async fn allocations_of_income(&self, income_id: &str) -> Vec<Value> {
        let list = self
            .get(&format!("/allocations?income_id={income_id}"))
            .await;
        assert_eq!(list.status, StatusCode::OK, "{}", list.body);
        list.body["items"].as_array().expect("items").clone()
    }

    /// The open allocation of an income record, with its plan.
    async fn open_for_income(&self, income_id: &str) -> Value {
        let all = self.allocations_of_income(income_id).await;
        let open = all
            .iter()
            .find(|a| {
                matches!(
                    a["status"].as_str(),
                    Some("pending" | "skipped" | "confirmed")
                )
            })
            .expect("open allocation");
        let detail = self
            .get(&format!(
                "/allocations/{}",
                open["id"].as_str().expect("id")
            ))
            .await;
        assert_eq!(detail.status, StatusCode::OK, "{}", detail.body);
        detail.body
    }

    /// Every allocation of an invoice payment (newest first) and the open one, if any.
    async fn payment_allocations(&self, payment_id: &str) -> (Vec<Value>, Option<Value>) {
        let list = self
            .get(&format!("/allocations?invoice_payment_id={payment_id}"))
            .await
            .body;
        let items = list["items"].as_array().expect("items").clone();
        let open = items
            .iter()
            .find(|a| matches!(a["status"].as_str(), Some("pending" | "confirmed")))
            .cloned();
        (items, open)
    }

    /// Confirms an allocation exactly as planned.
    async fn confirm_plan(&self, allocation: &Value) -> Value {
        let lines: Vec<Value> = allocation["plan"]["lines"]
            .as_array()
            .expect("plan lines")
            .iter()
            .map(|l| json!({ "destination_id": l["destination_id"], "amount": l["amount"], "rule_id": l["rule_id"] }))
            .collect();
        let confirmed = self
            .post(
                &format!(
                    "/allocations/{}/confirm",
                    allocation["id"].as_str().expect("id")
                ),
                json!({ "lines": lines }),
            )
            .await;
        assert_eq!(confirmed.status, StatusCode::OK, "{}", confirmed.body);
        confirmed.body
    }

    async fn balance(&self, name: &str) -> i64 {
        let balances = self.get("/allocations/balances").await;
        assert_eq!(balances.status, StatusCode::OK, "{}", balances.body);
        balances.body["destinations"]
            .as_array()
            .expect("destinations")
            .iter()
            .find(|d| d["name"] == name)
            .and_then(|d| d["balance"].as_i64())
            .unwrap_or_else(|| panic!("balance of {name}"))
    }
}

fn amounts(plan: &Value) -> Vec<i64> {
    plan["lines"]
        .as_array()
        .expect("lines")
        .iter()
        .map(|l| l["amount"].as_i64().expect("amount"))
        .collect()
}

#[tokio::test]
async fn destinations_are_seeded_and_can_be_managed() {
    let p = Plan::new("alloc-destinations").await;
    let list = p.get("/allocations/destinations").await;
    let names: Vec<&str> = list
        .body
        .as_array()
        .expect("list")
        .iter()
        .filter_map(|d| d["name"].as_str())
        .collect();
    assert_eq!(
        names,
        [
            "Income tax",
            "GST",
            "ACC levies",
            "Savings",
            "Emergency fund",
            "KiwiSaver",
            "Investments",
            "Mortgage",
            "Personal spending"
        ]
    );
    assert!(
        list.body
            .as_array()
            .expect("list")
            .iter()
            .all(|d| d["balance"] == 0 && d["target_amount"].is_null()),
        "defaults come without amounts"
    );

    let holiday = p
        .post(
            "/allocations/destinations",
            json!({ "name": "Holiday fund", "target_amount": 200_000 }),
        )
        .await;
    assert_eq!(holiday.status, StatusCode::CREATED, "{}", holiday.body);
    assert_eq!(holiday.body["kind"], "custom");
    let holiday_id = holiday.body["id"].as_str().expect("id").to_owned();
    let duplicate = p
        .post(
            "/allocations/destinations",
            json!({ "name": "holiday FUND" }),
        )
        .await;
    assert!(
        duplicate.field_error("name").is_some(),
        "{}",
        duplicate.body
    );
    let bad_target = p
        .post(
            "/allocations/destinations",
            json!({ "name": "Boat", "target_amount": 0 }),
        )
        .await;
    assert!(bad_target.field_error("target_amount").is_some());

    let renamed = p
        .put(
            &format!("/allocations/destinations/{holiday_id}"),
            json!({ "name": "Holidays", "target_amount": 250_000, "minimum_balance": 10_000 }),
        )
        .await;
    assert_eq!(renamed.status, StatusCode::OK, "{}", renamed.body);
    assert_eq!(renamed.body["name"], "Holidays");

    // Reordering needs every active destination exactly once.
    let mut ids: Vec<String> = p
        .get("/allocations/destinations")
        .await
        .body
        .as_array()
        .expect("list")
        .iter()
        .map(|d| d["id"].as_str().expect("id").to_owned())
        .collect();
    ids.reverse();
    let ordered = p
        .put("/allocations/destinations/order", json!({ "ids": ids }))
        .await;
    assert_eq!(ordered.status, StatusCode::OK, "{}", ordered.body);
    assert_eq!(ordered.body[0]["name"], "Holidays");
    let partial = p
        .put(
            "/allocations/destinations/order",
            json!({ "ids": [ids[0]] }),
        )
        .await;
    assert!(partial.field_error("ids").is_some());

    // A destination with active rules can't be archived until its rules are.
    let rule = p
        .rule(json!({ "destination_id": holiday_id, "method": "fixed", "fixed_amount": 5_000 }))
        .await;
    let blocked = p
        .post(
            &format!("/allocations/destinations/{holiday_id}/archive"),
            json!({ "archived": true }),
        )
        .await;
    assert_eq!(blocked.status, StatusCode::CONFLICT);
    p.post(
        &format!("/allocations/rules/{rule}/archive"),
        json!({ "archived": true }),
    )
    .await;
    let archived = p
        .post(
            &format!("/allocations/destinations/{holiday_id}/archive"),
            json!({ "archived": true }),
        )
        .await;
    assert_eq!(archived.status, StatusCode::OK, "{}", archived.body);
    assert_eq!(archived.body["archived"], true);
    let restore_rule = p
        .post(
            &format!("/allocations/rules/{rule}/archive"),
            json!({ "archived": false }),
        )
        .await;
    assert_eq!(
        restore_rule.status,
        StatusCode::CONFLICT,
        "its destination is archived"
    );
}

#[tokio::test]
async fn rule_errors_name_the_field_to_correct() {
    let p = Plan::new("alloc-rule-errors").await;
    let tax = p.destination("Income tax").await;
    let cases = [
        (json!({ "method": "percentage" }), "percentage"),
        (
            json!({ "method": "percentage", "percentage": "150" }),
            "percentage",
        ),
        (json!({ "method": "fixed" }), "fixed_amount"),
        (
            json!({ "method": "fixed", "fixed_amount": -5 }),
            "fixed_amount",
        ),
        (
            json!({ "method": "fixed", "fixed_amount": 100, "income_type": "sales" }),
            "income_type",
        ),
        (
            json!({ "method": "fixed", "fixed_amount": 100, "cap_amount": 1_000 }),
            "cap_period",
        ),
        (
            json!({ "method": "fixed", "fixed_amount": 100, "cap_period": "week" }),
            "cap_amount",
        ),
        (
            json!({ "method": "fixed", "fixed_amount": 100, "cap_amount": 1_000, "cap_period": "fortnight" }),
            "cap_anchor",
        ),
        (
            json!({ "method": "fixed", "fixed_amount": 100, "cap_amount": 1_000, "cap_period": "custom", "cap_anchor": "2026-06-01" }),
            "cap_period_days",
        ),
        (
            json!({ "method": "fixed", "fixed_amount": 100, "active_from": "2026-06-01", "active_to": "2026-05-01" }),
            "active_to",
        ),
        (
            json!({ "method": "fixed", "fixed_amount": 100, "customer_id": Uuid::now_v7() }),
            "customer_id",
        ),
    ];
    for (mut body, field) in cases {
        body["destination_id"] = json!(tax);
        let response = p.post("/allocations/rules", body.clone()).await;
        assert!(
            response.field_error(field).is_some(),
            "{field}: {body} → {}",
            response.body
        );
    }
    let unknown = p
        .post(
            "/allocations/rules",
            json!({ "destination_id": Uuid::now_v7(), "method": "fixed", "fixed_amount": 100 }),
        )
        .await;
    assert!(unknown.field_error("destination_id").is_some());
}

#[tokio::test]
async fn receipts_are_planned_reviewed_and_confirmed() {
    let p = Plan::new("alloc-review").await;
    // Without rules, nothing is proposed.
    let before_rules = p.income(10_000, "2026-06-01").await;
    assert!(p.allocations_of_income(&before_rules).await.is_empty());
    let balances = p.get("/allocations/balances").await;
    assert_eq!(balances.body["has_rules"], false);

    let tax = p.destination("Income tax").await;
    let emergency = p.destination("Emergency fund").await;
    let savings = p.destination("Savings").await;
    let tax_rule = p
        .rule(json!({ "destination_id": tax, "method": "percentage", "percentage": "20" }))
        .await;
    p.rule(json!({ "destination_id": emergency, "method": "fixed", "fixed_amount": 5_000 }))
        .await;
    let savings_rule = p
        .rule(json!({ "destination_id": savings, "method": "percentage", "percentage": "10", "basis": "remaining" }))
        .await;

    // $1,000 received: 20% ($200), $50, then 10% of the $750 left ($75).
    let income = p.income(100_000, "2026-06-10").await;
    let pending = p.open_for_income(&income).await;
    assert_eq!(pending["status"], "pending");
    assert_eq!(pending["source_label"], "Market stall");
    assert_eq!(amounts(&pending["plan"]), [20_000, 5_000, 7_500]);
    assert_eq!(pending["plan"]["allocated"], 32_500);
    assert_eq!(pending["plan"]["remaining"], 67_500);
    let preview = p
        .post(
            "/allocations/preview",
            json!({ "amount": 100_000, "date": "2026-06-10", "source": "income" }),
        )
        .await;
    assert_eq!(preview.status, StatusCode::OK, "{}", preview.body);
    assert_eq!(amounts(&preview.body), [20_000, 5_000, 7_500]);
    assert_eq!(
        p.get("/allocations/balances").await.body["pending_count"],
        1
    );

    // The review form's totals come from the API; more than received is refused.
    let id = pending["id"].as_str().expect("id");
    let too_much = p
        .post(
            &format!("/allocations/{id}/calculate"),
            json!({ "lines": [{ "destination_id": tax, "amount": 100_000 }, { "destination_id": savings, "amount": 1 }] }),
        )
        .await;
    assert!(too_much.field_error("lines").is_some(), "{}", too_much.body);
    let duplicate = p
        .post(
            &format!("/allocations/{id}/calculate"),
            json!({ "lines": [{ "destination_id": tax, "amount": 1 }, { "destination_id": tax, "amount": 1 }] }),
        )
        .await;
    assert!(duplicate.field_error("lines.1.destination_id").is_some());
    let adjusted = json!({ "lines": [
        { "destination_id": tax, "amount": 25_000, "rule_id": tax_rule },
        { "destination_id": emergency, "amount": 0 },
        { "destination_id": savings, "amount": 7_500, "rule_id": savings_rule },
    ] });
    let totals = p
        .post(&format!("/allocations/{id}/calculate"), adjusted.clone())
        .await;
    assert_eq!(totals.status, StatusCode::OK, "{}", totals.body);
    assert_eq!(totals.body["allocated"], 32_500);
    assert_eq!(totals.body["remaining"], 67_500);
    assert_eq!(totals.body["lines"].as_array().map(Vec::len), Some(2));

    // Confirming records the adjusted amounts once.
    let confirmed = p
        .post(&format!("/allocations/{id}/confirm"), adjusted.clone())
        .await;
    assert_eq!(confirmed.status, StatusCode::OK, "{}", confirmed.body);
    assert_eq!(confirmed.body["status"], "confirmed");
    assert_eq!(confirmed.body["allocated"], 32_500);
    assert_eq!(confirmed.body["entries"].as_array().map(Vec::len), Some(2));
    let again = p
        .post(&format!("/allocations/{id}/confirm"), adjusted)
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
    assert_eq!(p.balance("Income tax").await, 25_000);
    assert_eq!(p.balance("Savings").await, 7_500);
    assert_eq!(p.balance("Emergency fund").await, 0);
    let balances = p.get("/allocations/balances").await.body;
    assert_eq!(balances["total"], 32_500);
    assert_eq!(balances["pending_count"], 0);

    // History by destination.
    let history = p
        .get(&format!(
            "/allocations?destination_id={tax}&status=confirmed"
        ))
        .await;
    assert_eq!(history.body["total"], 1);

    // Skipping and reopening.
    let other = p.income(20_000, "2026-06-11").await;
    let other = p.open_for_income(&other).await;
    let other_id = other["id"].as_str().expect("id");
    let skipped = p
        .post(&format!("/allocations/{other_id}/skip"), json!({}))
        .await;
    assert_eq!(skipped.body["status"], "skipped");
    let reopened = p
        .post(&format!("/allocations/{other_id}/reopen"), json!({}))
        .await;
    assert_eq!(reopened.body["status"], "pending");
    assert!(reopened.body["plan"].is_object());
}

#[tokio::test]
async fn income_corrections_reverse_without_double_counting() {
    let p = Plan::new("alloc-income-corrections").await;
    let tax = p.destination("Income tax").await;
    p.rule(json!({ "destination_id": tax, "method": "percentage", "percentage": "20" }))
        .await;
    let income = p.income(100_000, "2026-06-10").await;
    let first = p.open_for_income(&income).await;
    p.confirm_plan(&first).await;
    assert_eq!(p.balance("Income tax").await, 20_000);
    let body = |amount: i64, date: &str| {
        json!({ "income_date": date, "payer": "Market stall", "income_type": "sales",
                "gross_amount": amount, "gst_treatment": "no_gst", "payment_method": "cash" })
    };

    // A new date keeps the confirmed amounts.
    let moved = p
        .put(&format!("/income/{income}"), body(100_000, "2026-06-12"))
        .await;
    assert_eq!(moved.status, StatusCode::OK, "{}", moved.body);
    let still = p.open_for_income(&income).await;
    assert_eq!(still["id"], first["id"]);
    assert_eq!(still["status"], "confirmed");
    assert_eq!(still["received_date"], "2026-06-12");
    assert_eq!(p.balance("Income tax").await, 20_000);

    // A new amount reverses the confirmed allocation and opens a new one for review.
    p.put(&format!("/income/{income}"), body(50_000, "2026-06-12"))
        .await;
    assert_eq!(p.balance("Income tax").await, 0);
    let replacement = p.open_for_income(&income).await;
    assert_eq!(replacement["status"], "pending");
    assert_eq!(replacement["replaces_id"], first["id"]);
    assert_eq!(amounts(&replacement["plan"]), [10_000]);
    let reversed = p
        .get(&format!(
            "/allocations/{}",
            first["id"].as_str().expect("id")
        ))
        .await
        .body;
    assert_eq!(reversed["status"], "reversed");
    assert!(
        reversed["close_reason"]
            .as_str()
            .is_some_and(|r| r.contains("$1,000.00") && r.contains("$500.00")),
        "{}",
        reversed["close_reason"]
    );
    let entries = reversed["entries"].as_array().expect("entries");
    assert_eq!(
        entries.len(),
        2,
        "the entry and its reversal stay in the history"
    );
    assert_eq!(entries[1]["kind"], "reversal");
    assert_eq!(entries[1]["amount"], -20_000);
    assert_eq!(entries[1]["reverses_id"], entries[0]["id"]);
    p.confirm_plan(&replacement).await;
    assert_eq!(p.balance("Income tax").await, 10_000);

    // The user can reverse a confirmed allocation to correct it.
    let corrected = p
        .post(
            &format!(
                "/allocations/{}/reverse",
                replacement["id"].as_str().expect("id")
            ),
            json!({ "reason": "Wrong amount set aside" }),
        )
        .await;
    assert_eq!(corrected.status, StatusCode::OK, "{}", corrected.body);
    assert_eq!(corrected.body["status"], "pending");
    assert_eq!(corrected.body["replaces_id"], replacement["id"]);
    assert_eq!(p.balance("Income tax").await, 0);
    let reverse_again = p
        .post(
            &format!(
                "/allocations/{}/reverse",
                replacement["id"].as_str().expect("id")
            ),
            json!({}),
        )
        .await;
    assert_eq!(reverse_again.status, StatusCode::CONFLICT);
    p.confirm_plan(&corrected.body).await;
    assert_eq!(p.balance("Income tax").await, 10_000);

    // Deleting the income reverses what is confirmed.
    let deleted = p
        .delete(
            &format!("/income/{income}"),
            json!({ "reason": "Duplicate" }),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{}", deleted.body);
    assert_eq!(p.balance("Income tax").await, 0);
    let all = p.allocations_of_income(&income).await;
    let statuses: Vec<&str> = all.iter().filter_map(|a| a["status"].as_str()).collect();
    // The original, the one opened after the amount changed, and the one after the user's reversal.
    assert_eq!(statuses.len(), 3);
    assert!(statuses.iter().all(|s| *s == "reversed"), "{statuses:?}");

    // Deleting income whose allocation was never confirmed cancels it.
    let unconfirmed = p.income(30_000, "2026-06-15").await;
    p.delete(
        &format!("/income/{unconfirmed}"),
        json!({ "reason": "Entered by mistake" }),
    )
    .await;
    let cancelled = p.allocations_of_income(&unconfirmed).await;
    assert_eq!(cancelled[0]["status"], "cancelled");
    assert_eq!(
        p.get("/allocations/balances").await.body["pending_count"],
        0
    );
    assert_eq!(p.get("/allocations/balances").await.body["total"], 0);
}

#[tokio::test]
async fn invoice_payments_and_customer_rules_follow_the_same_paths() {
    let p = Plan::new("alloc-payments").await;
    let customer = p.post("/customers", json!({ "name": "Kākā Studio" })).await;
    let customer = customer.body["id"].as_str().expect("id").to_owned();
    let tax = p.destination("Income tax").await;
    let savings = p.destination("Savings").await;
    p.rule(json!({ "destination_id": tax, "method": "percentage", "percentage": "25", "source": "invoice_payment" }))
        .await;
    p.rule(json!({ "destination_id": savings, "method": "fixed", "fixed_amount": 1_000, "customer_id": customer }))
        .await;

    // Income from no customer meets neither rule: its plan is empty.
    let income = p.income(10_000, "2026-06-01").await;
    let plan = p.open_for_income(&income).await;
    assert!(amounts(&plan["plan"]).is_empty());

    let draft = p
        .post(
            "/invoices",
            json!({ "customer_id": customer, "issue_date": "2026-06-01", "due_date": "2026-06-30",
                    "lines": [{ "description": "Brand work", "quantity": "1", "unit_price": 100_000, "tax_treatment": "no_gst" }] }),
        )
        .await;
    assert_eq!(draft.status, StatusCode::CREATED, "{}", draft.body);
    let invoice = draft.body["id"].as_str().expect("id").to_owned();
    let issued = p
        .post(&format!("/invoices/{invoice}/issue"), json!({}))
        .await;
    assert_eq!(issued.status, StatusCode::OK, "{}", issued.body);
    let paid = p
        .post(
            &format!("/invoices/{invoice}/payments"),
            json!({ "payment_date": "2026-06-05", "amount": 40_000, "method": "bank_transfer" }),
        )
        .await;
    assert_eq!(paid.status, StatusCode::CREATED, "{}", paid.body);
    let payment = paid.body["payments"][0]["id"]
        .as_str()
        .expect("payment")
        .to_owned();

    let (_, pending) = p.payment_allocations(&payment).await;
    let pending = p
        .get(&format!(
            "/allocations/{}",
            pending.expect("pending")["id"].as_str().expect("id")
        ))
        .await
        .body;
    assert_eq!(pending["customer_name"], "Kākā Studio");
    assert!(
        pending["source_label"]
            .as_str()
            .is_some_and(|l| l.starts_with("Invoice ")),
        "{}",
        pending["source_label"]
    );
    assert_eq!(pending["invoice_id"], invoice.as_str());
    assert_eq!(amounts(&pending["plan"]), [10_000, 1_000]);
    p.confirm_plan(&pending).await;
    assert_eq!(p.balance("Income tax").await, 10_000);
    assert_eq!(p.balance("Savings").await, 1_000);

    // Changing the payment's amount reverses and re-plans it.
    let edited = p
        .put(
            &format!("/invoices/{invoice}/payments/{payment}"),
            json!({ "payment_date": "2026-06-05", "amount": 60_000, "method": "bank_transfer" }),
        )
        .await;
    assert_eq!(edited.status, StatusCode::OK, "{}", edited.body);
    assert_eq!(p.balance("Income tax").await, 0);
    let (_, replacement) = p.payment_allocations(&payment).await;
    let replacement = replacement.expect("replacement");
    assert_eq!(replacement["status"], "pending");
    assert_eq!(replacement["received_amount"], 60_000);

    // Removing the payment cancels the unconfirmed allocation; nothing is left in the balances.
    let removed = p
        .delete(
            &format!("/invoices/{invoice}/payments/{payment}"),
            json!({ "reason": "Bounced" }),
        )
        .await;
    assert_eq!(removed.status, StatusCode::OK, "{}", removed.body);
    let (all, open_now) = p.payment_allocations(&payment).await;
    assert!(open_now.is_none());
    let statuses: Vec<&str> = all.iter().filter_map(|a| a["status"].as_str()).collect();
    // Newest first: the replacement (cancelled with the payment), then the original (reversed).
    assert_eq!(statuses, ["cancelled", "reversed"]);
    assert_eq!(p.balance("Income tax").await, 0);
    assert_eq!(p.balance("Savings").await, 0);
}

#[tokio::test]
async fn caps_and_targets_count_earlier_receipts() {
    let p = Plan::new("alloc-caps").await;
    let savings = p.destination("Savings").await;
    let emergency = p.destination("Emergency fund").await;
    p.rule(
        json!({ "destination_id": savings, "method": "percentage", "percentage": "50",
                   "cap_amount": 30_000, "cap_period": "month" }),
    )
    .await;
    p.put(
        &format!("/allocations/destinations/{emergency}"),
        json!({ "name": "Emergency fund", "kind": "emergency_fund", "target_amount": 25_000, "minimum_balance": 50_000 }),
    )
    .await;
    p.rule(json!({ "destination_id": emergency, "method": "fixed", "fixed_amount": 20_000, "stop_at_target": true }))
        .await;

    // June: $200 then $100 (the rest of the $300 cap); July starts again.
    let first = p.income(40_000, "2026-06-02").await;
    let first = p.open_for_income(&first).await;
    assert_eq!(amounts(&first["plan"]), [20_000, 20_000]);
    p.confirm_plan(&first).await;
    let second = p.income(40_000, "2026-06-20").await;
    let second = p.open_for_income(&second).await;
    assert_eq!(amounts(&second["plan"]), [10_000, 5_000]);
    let limits: Vec<&str> = second["plan"]["lines"]
        .as_array()
        .expect("lines")
        .iter()
        .filter_map(|l| l["limited_by"].as_str())
        .collect();
    assert_eq!(limits, ["cap", "target"]);
    p.confirm_plan(&second).await;
    let july = p.income(40_000, "2026-07-01").await;
    let july = p.open_for_income(&july).await;
    assert_eq!(amounts(&july["plan"]), [20_000, 0]);

    let balances = p.get("/allocations/balances").await.body;
    let emergency = balances["destinations"]
        .as_array()
        .expect("destinations")
        .iter()
        .find(|d| d["name"] == "Emergency fund")
        .expect("emergency")
        .clone();
    assert_eq!(emergency["balance"], 25_000);
    assert_eq!(emergency["target_progress"], 100);
    assert_eq!(emergency["below_minimum"], true);
}

#[tokio::test]
async fn export_and_deletion_include_allocations() {
    let p = Plan::new("alloc-export").await;
    let tax = p.destination("Income tax").await;
    p.rule(json!({ "destination_id": tax, "method": "percentage", "percentage": "20" }))
        .await;
    let income = p.income(10_000, "2026-06-01").await;
    let open = p.open_for_income(&income).await;
    p.confirm_plan(&open).await;
    let export = p.app.get("/api/v1/users/me/export", &p.user.auth()).await;
    let records = &export.body["businesses"][0]["records"];
    assert_eq!(
        records["allocation_destinations"].as_array().map(Vec::len),
        Some(9)
    );
    assert_eq!(
        records["allocation_rules"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(records["allocations"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        records["allocation_entries"].as_array().map(Vec::len),
        Some(1)
    );
    let business = p.get("").await.body;
    let deleted = p
        .app
        .delete(
            &p.path(""),
            &p.user.auth(),
            Some(json!({ "confirmation_name": business["name"] })),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{}", deleted.body);
}

#[tokio::test]
async fn allocations_are_private_to_their_business() {
    let a = Plan::new("alloc-tenant-a").await;
    let tax = a.destination("Income tax").await;
    let rule = a
        .rule(json!({ "destination_id": tax, "method": "percentage", "percentage": "20" }))
        .await;
    let income = a.income(10_000, "2026-06-01").await;
    let allocation = a.open_for_income(&income).await;
    let allocation_id = allocation["id"].as_str().expect("id");
    let customer = a.post("/customers", json!({ "name": "Private" })).await;
    let customer = customer.body["id"].as_str().expect("id").to_owned();

    // Another user, with their own business.
    let b = Plan::new("alloc-tenant-b").await;
    let not_found = [
        b.get(&format!("/allocations/{allocation_id}")).await,
        b.post(
            &format!("/allocations/{allocation_id}/confirm"),
            json!({ "lines": [] }),
        )
        .await,
        b.post(&format!("/allocations/{allocation_id}/skip"), json!({}))
            .await,
        b.put(
            &format!("/allocations/destinations/{tax}"),
            json!({ "name": "Mine now" }),
        )
        .await,
        b.post(
            &format!("/allocations/rules/{rule}/archive"),
            json!({ "archived": true }),
        )
        .await,
    ];
    for response in not_found {
        assert_eq!(response.status, StatusCode::NOT_FOUND, "{}", response.body);
    }
    let foreign = b
        .post(
            "/allocations/rules",
            json!({ "destination_id": tax, "method": "fixed", "fixed_amount": 100, "customer_id": customer }),
        )
        .await;
    assert!(foreign.field_error("destination_id").is_some());
    assert!(foreign.field_error("customer_id").is_some());
    let b_balances = b.get("/allocations/balances").await.body;
    assert_eq!(b_balances["pending_count"], 0);
    assert_eq!(b.get("/allocations").await.body["total"], 0);
    // B's own destinations are B's: nine, none of A's.
    let b_destinations = b.get("/allocations/destinations").await.body;
    assert!(
        b_destinations
            .as_array()
            .expect("list")
            .iter()
            .all(|d| d["id"] != tax.as_str())
    );

    // And B can't reach A's business at all.
    let direct = b
        .app
        .get(
            &format!("/api/v1/businesses/{}/allocations/balances", a.business),
            &b.user.auth(),
        )
        .await;
    assert!(
        matches!(direct.status, StatusCode::NOT_FOUND | StatusCode::FORBIDDEN),
        "{}",
        direct.status
    );
}

#[tokio::test]
async fn money_received_before_the_rules_can_be_allocated_and_set_aside_money_spent() {
    let plan = Plan::new("alloc-backfill-spend").await;
    // Income recorded while there were no rules gets no allocation.
    let early = plan.income(100_000, "2026-03-10").await;
    assert!(plan.allocations_of_income(&early).await.is_empty());

    // Without a rule there's nothing to work the allocation out from.
    let none = plan
        .post(
            "/allocations/backfill",
            json!({ "from": "2026-01-01", "to": "2026-12-31" }),
        )
        .await;
    assert_eq!(none.status, StatusCode::CONFLICT, "{}", none.body);

    let tax = plan.destination("Income tax").await;
    plan.rule(json!({ "destination_id": tax, "method": "percentage", "percentage": "20" }))
        .await;
    let opened = plan
        .post(
            "/allocations/backfill",
            json!({ "from": "2026-01-01", "to": "2026-12-31" }),
        )
        .await;
    assert_eq!(opened.status, StatusCode::OK, "{}", opened.body);
    assert_eq!(opened.body["opened"], 1);
    // Once each: a second run finds nothing new.
    let again = plan
        .post(
            "/allocations/backfill",
            json!({ "from": "2026-01-01", "to": "2026-12-31" }),
        )
        .await;
    assert_eq!(again.body["opened"], 0);
    let open = plan.open_for_income(&early).await;
    plan.confirm_plan(&open).await;
    assert_eq!(plan.balance("Income tax").await, 20_000);

    // Spending lowers the balance, never below zero.
    let spent = plan
        .post(
            "/allocations/spending",
            json!({ "destination_id": tax, "spent_on": "2026-05-07", "amount": 15_000,
                    "description": "Provisional tax instalment" }),
        )
        .await;
    assert_eq!(spent.status, StatusCode::CREATED, "{}", spent.body);
    assert_eq!(plan.balance("Income tax").await, 5_000);
    let too_much = plan
        .post(
            "/allocations/spending",
            json!({ "destination_id": tax, "spent_on": "2026-05-08", "amount": 5_001, "description": "More" }),
        )
        .await;
    assert_eq!(too_much.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        too_much.field_error("amount").is_some(),
        "{}",
        too_much.body
    );

    let list = plan.get("/allocations/spending").await;
    assert_eq!(list.body.as_array().map(Vec::len), Some(1), "{}", list.body);
    let report = plan
        .get("/reports/allocations?from=2026-04-01&to=2027-03-31")
        .await;
    assert!(
        report
            .body
            .to_string()
            .contains("Provisional tax instalment"),
        "{}",
        report.body
    );

    // Deleting it puts the money back.
    let id = spent.body["id"].as_str().expect("id");
    let deleted = plan
        .delete(
            &format!("/allocations/spending/{id}"),
            json!({ "reason": "Entered twice" }),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{}", deleted.body);
    assert_eq!(plan.balance("Income tax").await, 20_000);

    // Another business can't see or delete it.
    let other = Plan::new("alloc-backfill-spend-other").await;
    let response = other
        .app
        .delete(
            &format!(
                "/api/v1/businesses/{}/allocations/spending/{id}",
                other.business
            ),
            &other.user.auth(),
            Some(json!({ "reason": "x" })),
        )
        .await;
    assert_eq!(response.status, StatusCode::NOT_FOUND);
}
