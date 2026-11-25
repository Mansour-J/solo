//! IRD's rules for sole traders, checked end to end. Each test names the rule and its source (see
//! docs/product/ird-rules.md for the full list, what the app doesn't do yet, and links):
//!
//! - GST is 15%, or 3/23 of a GST-inclusive price (IR375).
//! - Zero-rated and exempt supplies, and anything before the registration date, carry no GST (IR375).
//! - Payments basis: GST when paid, in proportion; invoice basis: when invoiced; hybrid: sales by
//!   invoice, purchases by payment (IR375, page 7).
//! - A schedular payment: withholding tax is worked out on the GST-exclusive amount (IR335, IR330C);
//!   checked against a real buyer-created tax invoice (168 hours at $130).
//! - Mixed business and private use: claim the business share of the GST and the cost (IR375,
//!   "GST adjustments for business, private and exempt use"; home office internet).
//! - Kilometre rates: "When self-employed people use kilometre rates, they do not need to consider
//!   GST" (ird.govt.nz/vehicle-expenses).

mod common;

use axum::http::StatusCode;
use common::{TestApp, TestUser};
use serde_json::{Value, json};

struct Books {
    app: TestApp,
    user: TestUser,
    base: String,
    business: Value,
    customer: String,
}

impl Books {
    /// A GST-registered business (from 1 Jan 2026, two-monthly, periods ending in odd months).
    async fn new(prefix: &str, gst_basis: &str, registered_from: &str) -> Self {
        let app = TestApp::new().await;
        let user = app.register(prefix).await;
        let created = app
            .post(
                "/api/v1/businesses",
                &user.auth(),
                json!({ "name": format!("{prefix} Ltd"), "gst_registered": true, "gst_number": "49091850",
                        "gst_registered_from": registered_from, "gst_basis": gst_basis, "complete_onboarding": true }),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
        let base = format!(
            "/api/v1/businesses/{}",
            created.body["id"].as_str().expect("id")
        );
        let mut books = Self {
            app,
            user,
            base,
            business: created.body,
            customer: String::new(),
        };
        let customer = books
            .ok(
                "POST",
                "/customers",
                json!({ "name": "Example client via agency" }),
            )
            .await;
        books.customer = customer["id"].as_str().expect("id").to_owned();
        books
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

    /// Issues an invoice with these lines and returns it.
    async fn invoice(&self, date: &str, lines: Value, prices_include_gst: bool) -> Value {
        let draft = self
            .ok(
                "POST",
                "/invoices",
                json!({ "customer_id": self.customer, "issue_date": date, "due_date": date, "lines": lines,
                        "prices_include_gst": prices_include_gst }),
            )
            .await;
        let id = draft["id"].as_str().expect("id");
        self.ok("POST", &format!("/invoices/{id}/issue"), json!({}))
            .await
    }

    async fn pay(&self, invoice: &Value, date: &str, amount: i64, withholding_tax: i64) {
        let id = invoice["id"].as_str().expect("id");
        self.ok(
            "POST",
            &format!("/invoices/{id}/payments"),
            json!({ "payment_date": date, "amount": amount, "withholding_tax": withholding_tax, "method": "bank_transfer" }),
        )
        .await;
    }

    async fn category(&self) -> Value {
        let categories = self.ok("GET", "/expense-categories", json!(null)).await;
        categories[0]["id"].clone()
    }

    /// The GST return period ending on `end`.
    async fn gst_period(&self, end: &str) -> Value {
        let returns = self.ok("GET", "/gst/returns", json!(null)).await;
        returns["returns"]
            .as_array()
            .expect("returns")
            .iter()
            .find(|r| r["end"] == end)
            .cloned()
            .unwrap_or_else(|| panic!("no GST period ending {end}: {returns}"))
    }

    async fn set_gst_basis(&self, basis: &str) {
        let mut input = self.business.clone();
        input["gst_basis"] = json!(basis);
        self.ok("PUT", "", input).await;
    }
}

fn line(description: &str, quantity: &str, unit_price: i64, tax_treatment: &str) -> Value {
    json!({ "description": description, "quantity": quantity, "unit_price": unit_price, "tax_treatment": tax_treatment })
}

#[tokio::test]
async fn gst_is_15_percent_or_three_twenty_thirds_of_an_inclusive_price() {
    let b = Books::new("ird-gst-rate", "invoice", "2026-01-01").await;
    // IR375's worked example: a $230 GST-inclusive price contains $30 of GST.
    let inclusive = b
        .invoice(
            "2026-04-10",
            json!([line("Workshop", "1", 23_000, "standard")]),
            true,
        )
        .await;
    assert_eq!(
        (
            &inclusive["subtotal"],
            &inclusive["gst_total"],
            &inclusive["total"]
        ),
        (&json!(20_000), &json!(3_000), &json!(23_000))
    );
    // $1,000 excluding GST: $150 of GST, $1,150 in total.
    let exclusive = b
        .invoice(
            "2026-04-20",
            json!([line("Design", "1", 100_000, "standard")]),
            false,
        )
        .await;
    assert_eq!(
        (&exclusive["gst_total"], &exclusive["total"]),
        (&json!(15_000), &json!(115_000))
    );
    // An expense of $115 including GST: $15 to claim.
    b.ok(
        "POST",
        "/expenses",
        json!({ "expense_date": "2026-05-05", "supplier": "Stationer", "category_id": b.category().await, "gross_amount": 11_500,
                "gst_treatment": "standard", "payment_method": "business_account", "classification": "business" }),
    )
    .await;
    let period = b.gst_period("2026-05-31").await;
    assert_eq!(
        (&period["collected"], &period["claimable"], &period["net"]),
        (&json!(18_000), &json!(1_500), &json!(16_500))
    );
}

#[tokio::test]
async fn zero_rated_exempt_and_unregistered_supplies_carry_no_gst() {
    let b = Books::new("ird-no-gst", "invoice", "2026-05-01").await;
    let mixed = b
        .invoice(
            "2026-05-10",
            json!([
                line("Services to an overseas client", "1", 100_000, "zero_rated"),
                line("Residential rent", "1", 50_000, "exempt"),
                line("Local consulting", "1", 10_000, "standard"),
            ]),
            false,
        )
        .await;
    assert_eq!(mixed["gst_total"], 1_500, "only the standard-rated line");
    // Before the registration date (1 May) nothing carries GST, even at the standard rate.
    let before = b
        .invoice(
            "2026-04-15",
            json!([line("Early work", "1", 100_000, "standard")]),
            false,
        )
        .await;
    assert_eq!(
        (&before["gst_total"], &before["total"]),
        (&json!(0), &json!(100_000))
    );
    let period = b.gst_period("2026-05-31").await;
    assert_eq!(period["collected"], 1_500);
}

#[tokio::test]
async fn each_accounting_basis_counts_gst_at_the_right_time() {
    let b = Books::new("ird-bases", "payments", "2026-01-01").await;
    // Invoiced in May ($1,000 + $150), half paid in June, the rest never.
    let invoice = b
        .invoice(
            "2026-05-10",
            json!([line("Work", "1", 100_000, "standard")]),
            false,
        )
        .await;
    b.pay(&invoice, "2026-06-10", 57_500, 0).await;

    // Payments basis: nothing in Apr–May; in Jun–Jul the GST in what was received, 3/23 of $575.
    assert_eq!(b.gst_period("2026-05-31").await["collected"], 0);
    assert_eq!(b.gst_period("2026-07-31").await["collected"], 7_500);

    // Invoice basis: all of it when invoiced.
    b.set_gst_basis("invoice").await;
    assert_eq!(b.gst_period("2026-05-31").await["collected"], 15_000);
    assert_eq!(b.gst_period("2026-07-31").await["collected"], 0);

    // Hybrid: sales on the invoice basis.
    b.set_gst_basis("hybrid").await;
    assert_eq!(b.gst_period("2026-05-31").await["collected"], 15_000);
}

#[tokio::test]
async fn a_schedular_payment_matches_a_buyer_created_tax_invoice() {
    // A real buyer-created tax invoice: 168 hours at $130 = $21,840; GST $3,276; withholding tax at
    // 20% of the GST-exclusive amount, $4,368; amount paid $20,748.
    let b = Books::new("ird-schedular", "payments", "2026-01-01").await;
    let invoice = b
        .invoice(
            "2026-05-31",
            json!([line("Contract, May 2026", "168", 13_000, "standard")]),
            false,
        )
        .await;
    assert_eq!(
        (
            &invoice["subtotal"],
            &invoice["gst_total"],
            &invoice["total"]
        ),
        (&json!(2_184_000), &json!(327_600), &json!(2_511_600))
    );
    b.pay(&invoice, "2026-06-05", 2_511_600, 436_800).await;

    let period = b.gst_period("2026-07-31").await;
    assert_eq!(
        period["collected"], 327_600,
        "GST on the whole invoice, though the bank got less"
    );

    let changes = b
        .ok(
            "GET",
            "/reports/equity_changes?from=2026-04-01&to=2027-03-31&basis=cash",
            json!(null),
        )
        .await;
    let withheld = changes["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .flat_map(|s| s["rows"].as_array().cloned().unwrap_or_default())
        .find(|r| r[0] == "Withholding tax paid")
        .expect("withholding tax row");
    assert_eq!(withheld[1], 436_800, "the tax withheld counts as tax paid");

    let sheet = b
        .ok(
            "GET",
            "/reports/balance_sheet?from=2026-04-01&to=2027-03-31&basis=cash",
            json!(null),
        )
        .await;
    let bank = sheet["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .flat_map(|s| s["rows"].as_array().cloned().unwrap_or_default())
        .find(|r| r[0] == "Business bank account")
        .expect("bank row");
    assert_eq!(bank[1], 2_074_800, "the bank received $20,748");
}

#[tokio::test]
async fn only_the_business_share_of_a_mixed_use_cost_is_claimed() {
    // Home internet, $115 a month including GST, 35% business use, paid personally.
    let b = Books::new("ird-mixed-use", "payments", "2026-01-01").await;
    let expense = b
        .ok(
            "POST",
            "/expenses",
            json!({ "expense_date": "2026-05-10", "supplier": "Internet provider", "category_id": b.category().await,
                    "gross_amount": 11_500, "gst_treatment": "standard", "payment_method": "personal_funds",
                    "classification": "partial", "business_percentage": "35" }),
        )
        .await;
    assert_eq!(
        (
            &expense["split"]["expense_amount"],
            &expense["split"]["gst_claim"]
        ),
        (&json!(3_500), &json!(525)),
        "35% of $100 and of the $15 GST: {expense}"
    );
    assert_eq!(
        expense["split"]["private_amount"], 7_475,
        "the rest is private"
    );
    assert_eq!(b.gst_period("2026-05-31").await["claimable"], 525);
}

#[tokio::test]
async fn secondhand_goods_from_someone_not_registered_claim_3_23_of_the_price() {
    // IRD: a secondhand purchase from a seller not registered for GST claims 3/23 of the price paid,
    // the same as the GST share of a 15% GST-inclusive price, so it's recorded as a standard expense.
    let b = Books::new("ird-secondhand", "payments", "2026-01-01").await;
    let expense = b
        .ok(
            "POST",
            "/expenses",
            json!({ "expense_date": "2026-05-10", "supplier": "Private seller", "category_id": b.category().await,
                    "gross_amount": 46_000, "gst_treatment": "standard", "payment_method": "personal_funds", "classification": "business",
                    "notes": "Secondhand desk bought on a marketplace" }),
        )
        .await;
    assert_eq!(
        expense["split"]["gst_claim"], 6_000,
        "3/23 of $460 is $60: {expense}"
    );
    assert_eq!(b.gst_period("2026-05-31").await["claimable"], 6_000);
}

#[tokio::test]
async fn the_expense_preview_is_what_saving_books() {
    let b = Books::new("ird-expense-preview", "payments", "2026-01-01").await;
    let category = b.category().await;
    let body = json!({ "expense_date": "2026-05-10", "supplier": "Internet provider", "category_id": category,
                       "gross_amount": 11_500, "gst_treatment": "standard", "payment_method": "personal_funds",
                       "classification": "partial", "business_percentage": "35" });
    let preview = b.ok("POST", "/expenses/calculate", body.clone()).await;
    assert_eq!(preview["registered"], true);
    assert_eq!(preview["gst_amount"], 1_500, "3/23 of $115: {preview}");
    assert_eq!(preview["gst_rate"], "0.15");
    let saved = b.ok("POST", "/expenses", body).await;
    assert_eq!(
        preview["split"], saved["split"],
        "the preview matches the saved booking"
    );
    assert_eq!(
        preview["deductible_percentage"],
        saved["deductible_percentage"]
    );
    assert_eq!(saved["gst_entered"], false, "worked out from the amount");
    // GST typed in from a receipt that differs from 3/23 is marked as entered, so editing keeps it.
    let entered = b
        .ok(
            "POST",
            "/expenses",
            json!({ "expense_date": "2026-05-11", "supplier": "Hardware store", "category_id": category,
                    "gross_amount": 11_500, "gst_treatment": "standard", "gst_amount": 1_480,
                    "payment_method": "business_account", "classification": "business" }),
        )
        .await;
    assert_eq!(entered["gst_entered"], true, "{entered}");

    // Before GST registration there's no GST to claim: it's part of the cost.
    let before = b
        .ok(
            "POST",
            "/expenses/calculate",
            json!({ "expense_date": "2025-12-10", "gross_amount": 11_500, "gst_treatment": "standard",
                    "classification": "business" }),
        )
        .await;
    assert_eq!(
        (
            &before["registered"],
            &before["gst_amount"],
            &before["split"]["expense_amount"]
        ),
        (&json!(false), &json!(0), &json!(11_500)),
        "{before}"
    );

    // The same field errors as saving.
    let bad = b
        .app
        .post(
            &format!("{}/expenses/calculate", b.base),
            &b.user.auth(),
            json!({ "expense_date": "2026-05-10", "gross_amount": 11_500, "gst_treatment": "standard",
                    "classification": "partial", "business_percentage": "120" }),
        )
        .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", bad.body);
    assert!(bad.field_error("business_percentage").is_some());
}

#[tokio::test]
async fn cost_of_sales_comes_before_gross_profit() {
    let b = Books::new("cost-of-sales", "payments", "2026-01-01").await;
    let materials = b
        .ok(
            "POST",
            "/expense-categories",
            json!({ "name": "Materials", "non_deductible": false, "cost_of_sales": true }),
        )
        .await;
    // A cost of sales is deductible: both can't be ticked.
    let both = b
        .app
        .post(
            &format!("{}/expense-categories", b.base),
            &b.user.auth(),
            json!({ "name": "Odd", "non_deductible": true, "cost_of_sales": true }),
        )
        .await;
    assert_eq!(both.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(both.field_error("cost_of_sales").is_some(), "{}", both.body);

    let invoice = b
        .invoice(
            "2026-05-04",
            json!([line("Built-in shelves", "1", 100_000, "standard")]),
            false,
        )
        .await;
    b.pay(&invoice, "2026-05-20", 115_000, 0).await;
    for (supplier, category, gross) in [
        ("Timber yard", materials["id"].clone(), 23_000),
        ("Phone company", b.category().await, 11_500),
    ] {
        b.ok(
            "POST",
            "/expenses",
            json!({ "expense_date": "2026-05-06", "supplier": supplier, "category_id": category,
                    "gross_amount": gross, "gst_treatment": "standard", "payment_method": "business_account",
                    "classification": "business" }),
        )
        .await;
    }
    let pl = b
        .ok(
            "GET",
            "/reports/profit_and_loss?from=2026-04-01&to=2027-03-31&basis=accrual",
            json!(null),
        )
        .await;
    let section = |title: &str| {
        pl["sections"]
            .as_array()
            .expect("sections")
            .iter()
            .find(|s| s["title"] == title)
            .cloned()
            .unwrap_or_else(|| panic!("no {title} section: {pl}"))
    };
    assert_eq!(
        section("Cost of sales")["totals"][1],
        20_000,
        "$200 of materials"
    );
    assert_eq!(
        section("Expenses")["totals"][1],
        10_000,
        "the phone, not the materials"
    );
    let profit = section("Profit");
    let row = |label: &str| {
        profit["rows"]
            .as_array()
            .expect("rows")
            .iter()
            .find(|r| r[0] == label)
            .cloned()
            .unwrap_or_else(|| panic!("no {label}: {profit}"))
    };
    assert_eq!(
        row("Gross profit")[1],
        80_000,
        "$1,000 of sales less $200 of materials"
    );
    assert_eq!(row("Net profit")[1], 70_000);
}

#[tokio::test]
async fn the_gst_return_boxes_add_up_to_what_is_paid() {
    // IR375 / GST101A: box 5 sales incl. GST, 6 zero-rated, 7 = 5 − 6, 8 GST on sales, 9 debit
    // adjustments, 10 = 8 + 9, 11 purchases incl. GST, 12 GST on them, 13 credit adjustments,
    // 14 = 12 + 13, 15 = 10 − 14 (what the GST page says to pay).
    let b = Books::new("gst-boxes", "payments", "2026-01-01").await;
    let invoice = b
        .invoice(
            "2026-04-10",
            json!([
                line("Design", "1", 100_000, "standard"),
                line("Export work", "1", 20_000, "zero_rated")
            ]),
            false,
        )
        .await;
    b.pay(&invoice, "2026-04-20", 135_000, 0).await;
    for (payer, gross, treatment) in [
        ("Bank", 5_000, "exempt"),
        ("Market stall", 11_500, "standard"),
    ] {
        b.ok(
            "POST",
            "/income",
            json!({ "income_date": "2026-04-30", "payer": payer, "income_type": "other", "gross_amount": gross,
                    "gst_treatment": treatment, "payment_method": "bank_transfer" }),
        )
        .await;
    }
    let category = b.category().await;
    for (gross, classification, share) in
        [(23_000, "business", None), (11_500, "partial", Some("35"))]
    {
        b.ok(
            "POST",
            "/expenses",
            json!({ "expense_date": "2026-05-02", "supplier": "Supplier", "category_id": category,
                    "gross_amount": gross, "gst_treatment": "standard", "payment_method": "business_account",
                    "classification": classification, "business_percentage": share }),
        )
        .await;
    }
    let boxes = b
        .ok("GET", "/gst/returns/boxes?end=2026-05-31", json!(null))
        .await;
    let expect = [
        ("box5_sales", 146_500),
        ("box6_zero_rated", 20_000),
        ("box7", 126_500),
        ("box8_gst", 16_500),
        ("box9_debit_adjustments", 0),
        ("box11_purchases", 27_025),
        ("box12_gst", 3_525),
        ("box13_credit_adjustments", 0),
        ("box15", 12_975),
    ];
    for (name, value) in expect {
        assert_eq!(boxes[name], value, "{name}: {boxes}");
    }
    assert_eq!(
        boxes["box15"],
        b.gst_period("2026-05-31").await["net"],
        "box 15 is the return's GST to pay"
    );

    // On the invoice basis, a bad debt written off is a credit adjustment in the period written off.
    b.set_gst_basis("invoice").await;
    let unpaid = b
        .invoice(
            "2026-06-05",
            json!([line("Work", "1", 100_000, "standard")]),
            false,
        )
        .await;
    let id = unpaid["id"].as_str().expect("id");
    b.ok(
        "POST",
        &format!("/invoices/{id}/payments"),
        json!({ "payment_date": "2026-07-10", "amount": 115_000, "method": "write_off" }),
    )
    .await;
    let july = b
        .ok("GET", "/gst/returns/boxes?end=2026-07-31", json!(null))
        .await;
    assert_eq!(july["box13_credit_adjustments"], 15_000, "{july}");
    assert_eq!(july["box15"], b.gst_period("2026-07-31").await["net"]);
}

#[tokio::test]
async fn recurring_invoices_repeat_an_invoice_on_schedule() {
    let b = Books::new("recurring", "payments", "2026-01-01").await;
    let today =
        chrono::NaiveDate::parse_from_str(b.business["today"].as_str().expect("today"), "%Y-%m-%d")
            .expect("today");
    let first = today - chrono::Duration::days(40);
    // The invoice to repeat: due 20 days after issue.
    let draft = b
        .ok(
            "POST",
            "/invoices",
            json!({ "customer_id": b.customer, "issue_date": first.to_string(),
                    "due_date": (first + chrono::Duration::days(20)).to_string(), "reference": "PO-7",
                    "lines": [line("Weekly support", "1", 50_000, "standard")] }),
        )
        .await;
    let schedule = b
        .ok(
            "POST",
            "/recurring-invoices",
            json!({ "invoice_id": draft["id"], "name": "Weekly support", "frequency": "weekly",
                    "next_date": first.to_string() }),
        )
        .await;
    assert_eq!(schedule["payment_terms_days"], 20, "{schedule}");

    // Every week from 40 days ago to today: 6 invoices, then nothing more until the next one is due.
    let run = b.ok("POST", "/recurring-invoices/run", json!({})).await;
    assert_eq!(run["created"], 6, "{run}");
    assert_eq!(run["issued"], 0);
    let again = b.ok("POST", "/recurring-invoices/run", json!({})).await;
    assert_eq!(again["created"], 0);
    let list = b.ok("GET", "/recurring-invoices", json!(null)).await;
    assert_eq!(list[0]["invoices_created"], 6);
    assert_eq!(
        list[0]["next_date"],
        (first + chrono::Duration::days(42)).to_string()
    );

    let invoices = b
        .ok("GET", "/invoices?status=draft&per_page=50", json!(null))
        .await;
    let made: Vec<&Value> = invoices["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter(|i| i["reference"] == "PO-7" && i["id"] != draft["id"])
        .collect();
    assert_eq!(made.len(), 6, "{invoices}");
    assert!(made.iter().all(|i| i["total"] == 57_500));
    let last = made
        .iter()
        .max_by_key(|i| i["issue_date"].as_str().map(str::to_owned))
        .expect("last");
    assert_eq!(
        last["issue_date"],
        (first + chrono::Duration::days(35)).to_string()
    );
    assert_eq!(
        last["due_date"],
        (first + chrono::Duration::days(55)).to_string()
    );

    // Issuing automatically, and stopping.
    let id = schedule["id"].as_str().expect("id");
    let changed = b
        .ok(
            "PUT",
            &format!("/recurring-invoices/{id}"),
            json!({ "name": "Weekly support", "frequency": "weekly", "next_date": today.to_string(), "auto_issue": true }),
        )
        .await;
    assert_eq!(changed["auto_issue"], true);
    let issued = b.ok("POST", "/recurring-invoices/run", json!({})).await;
    assert_eq!(
        (&issued["created"], &issued["issued"]),
        (&json!(1), &json!(1)),
        "{issued}"
    );
    b.ok(
        "POST",
        &format!("/recurring-invoices/{id}/archive"),
        json!({ "archived": true }),
    )
    .await;
    assert_eq!(
        b.ok("GET", "/recurring-invoices", json!(null)).await[0]["archived"],
        true
    );

    // An end date before the next invoice is refused; another business can't see the schedule.
    let bad = b
        .app
        .put(
            &format!("{}/recurring-invoices/{id}", b.base),
            &b.user.auth(),
            json!({ "name": "x", "frequency": "weekly", "next_date": today.to_string(),
                    "end_date": first.to_string() }),
        )
        .await;
    assert!(bad.field_error("end_date").is_some(), "{}", bad.body);
    let other = b.app.register("recurring-other").await;
    let response = b
        .app
        .put(
            &format!("{}/recurring-invoices/{id}", b.base),
            &other.auth(),
            json!({ "name": "x", "frequency": "weekly", "next_date": today.to_string() }),
        )
        .await;
    assert!(
        response.status == StatusCode::NOT_FOUND || response.status == StatusCode::FORBIDDEN,
        "{}",
        response.status
    );
}

#[tokio::test]
async fn kilometre_rate_mileage_claims_carry_no_gst() {
    let b = Books::new("ird-mileage", "payments", "2026-01-01").await;
    let vehicle = b
        .ok(
            "POST",
            "/mileage/vehicles",
            json!({ "name": "Car", "vehicle_type": "car", "fuel_type": "petrol",
                    "new_rate": { "name": "IRD tier 1 (2025-26)", "effective_from": "2025-04-01", "rate_per_km": "1.20" } }),
        )
        .await;
    b.ok(
        "POST",
        "/mileage/trips",
        json!({ "vehicle_id": vehicle["id"], "trip_date": "2026-05-12", "purpose": "Client site", "distance_km": "100" }),
    )
    .await;
    let pl = b
        .ok(
            "GET",
            "/reports/profit_and_loss?from=2026-04-01&to=2027-03-31&basis=cash",
            json!(null),
        )
        .await;
    assert!(
        pl.to_string().contains("12000"),
        "100 km at $1.20 is a $120 expense: {pl}"
    );
    let gst = b
        .ok(
            "GET",
            "/reports/gst?from=2026-04-01&to=2027-03-31&basis=cash",
            json!(null),
        )
        .await;
    assert_eq!(
        gst["sections"][0]["rows"][1][1], 0,
        "no GST claimed on the mileage: {gst}"
    );
}

#[tokio::test]
async fn return_trips_count_both_ways_and_frequent_routes_are_offered() {
    let b = Books::new("ird-return-trips", "payments", "2026-01-01").await;
    let vehicle = b
        .ok(
            "POST",
            "/mileage/vehicles",
            json!({ "name": "Car", "vehicle_type": "car", "fuel_type": "petrol",
                    "new_rate": { "name": "Rate", "effective_from": "2025-04-01", "rate_per_km": "1.20" } }),
        )
        .await;
    let trip = |date: &str| {
        json!({ "vehicle_id": vehicle["id"], "trip_date": date, "purpose": "Client site", "start_location": "Home",
                "destination": "Client office", "distance_km": "20", "return_trip": true })
    };
    let first = b.ok("POST", "/mileage/trips", trip("2026-05-04")).await;
    assert_eq!(first["return_trip"], true);
    assert_eq!(first["distance_km"], "40.0", "20 km each way: {first}");
    assert_eq!(first["claim_amount"], 4_800, "40 km × $1.20");

    // Odometer readings already cover the whole trip.
    let odometer = b
        .app
        .post(
            &format!("{}/mileage/trips", b.base),
            &b.user.auth(),
            json!({ "vehicle_id": vehicle["id"], "trip_date": "2026-05-05", "purpose": "Client site",
                    "start_odometer": "1000", "end_odometer": "1040", "return_trip": true }),
        )
        .await;
    assert_eq!(odometer.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        odometer.field_error("return_trip").is_some(),
        "{}",
        odometer.body
    );

    // A route made twice is offered as a frequent trip, with the distance as entered.
    assert_eq!(
        b.ok("GET", "/mileage/frequent-trips", json!(null)).await,
        json!([])
    );
    b.ok("POST", "/mileage/trips", trip("2026-05-11")).await;
    let frequent = b.ok("GET", "/mileage/frequent-trips", json!(null)).await;
    assert_eq!(frequent.as_array().map(Vec::len), Some(1), "{frequent}");
    assert_eq!(frequent[0]["destination"], "Client office");
    assert_eq!(frequent[0]["distance_km"], "20.0");
    assert_eq!(frequent[0]["return_trip"], true);
    assert_eq!(frequent[0]["trips"], 2);
    assert_eq!(frequent[0]["last_date"], "2026-05-11");
}

#[tokio::test]
async fn kilometres_past_14000_in_the_year_take_irds_tier_2_rate() {
    // IRD 2025-26 petrol: $1.20 a km for the first 14,000 km a vehicle travels in the year, $0.37 after.
    let b = Books::new("ird-tiers", "payments", "2026-01-01").await;
    let auth = b.user.auth();

    // Tiers are both or neither.
    let half = b
        .app
        .post(
            &format!("{}/mileage/rates", b.base),
            &auth,
            json!({ "name": "Half", "effective_from": "2025-04-01", "rate_per_km": "1.20", "tier1_limit_km": "14000" }),
        )
        .await;
    assert_eq!(
        half.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        half.body
    );
    assert!(
        half.field_error("tier2_rate_per_km").is_some(),
        "{}",
        half.body
    );

    let vehicle = b
        .ok(
            "POST",
            "/mileage/vehicles",
            json!({ "name": "Car", "vehicle_type": "car", "fuel_type": "petrol",
                    "new_rate": { "name": "IRD petrol (2025-26)", "effective_from": "2025-04-01", "rate_per_km": "1.20",
                                  "tier1_limit_km": "14000", "tier2_rate_per_km": "0.37" } }),
        )
        .await;
    let trip = |date: &str, km: &str, share: &str| json!({ "vehicle_id": vehicle["id"], "trip_date": date, "purpose": "Travel", "distance_km": km, "business_percentage": share });
    // 13,900 km of private travel earlier in the year (April 2026 to March 2027).
    b.ok("POST", "/mileage/trips", trip("2026-04-10", "9000", "0"))
        .await;
    let second = b
        .ok("POST", "/mileage/trips", trip("2026-05-10", "4900", "0"))
        .await;
    // A trip in the previous year doesn't count toward this year's limit.
    b.ok("POST", "/mileage/trips", trip("2026-03-20", "5000", "0"))
        .await;

    // The preview shows the split before saving.
    let preview = b
        .ok(
            "POST",
            "/mileage/calculate",
            trip("2026-06-01", "200", "100"),
        )
        .await;
    assert_eq!(preview["travelled_before_km"], "13900.0", "{preview}");
    assert_eq!(preview["tier2_distance_km"], "100.0", "{preview}");
    assert_eq!(preview["claim_amount"], 15_700, "100 × $1.20 + 100 × $0.37");

    let crossing = b
        .ok("POST", "/mileage/trips", trip("2026-06-01", "200", "100"))
        .await;
    assert_eq!(crossing["claim_amount"], 15_700, "{crossing}");
    assert_eq!(crossing["tier2_distance_km"], "100.0");
    // Half business, all of it past 14,000 km: 100 business km × $0.37.
    let after = b
        .ok("POST", "/mileage/trips", trip("2026-07-01", "200", "50"))
        .await;
    assert_eq!(after["claim_amount"], 3_700, "{after}");

    // A trip recorded later but dated earlier pushes the others further into tier 2.
    let early = b
        .ok("POST", "/mileage/trips", trip("2026-05-20", "100", "100"))
        .await;
    assert_eq!(early["claim_amount"], 12_000, "13,900 → 14,000: all tier 1");
    let id = |t: &Value| t["id"].as_str().expect("id").to_owned();
    let crossing_now = b
        .ok(
            "GET",
            &format!("/mileage/trips/{}", id(&crossing)),
            json!(null),
        )
        .await;
    assert_eq!(crossing_now["claim_amount"], 200 * 37, "{crossing_now}");
    assert_eq!(crossing_now["tier2_distance_km"], "200.0");

    // Deleting the 4,900 km trip brings the later ones back under the limit.
    let deleted = b
        .app
        .delete(
            &format!("{}/mileage/trips/{}", b.base, id(&second)),
            &auth,
            Some(json!({ "reason": "Duplicate" })),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{}", deleted.body);
    let crossing_now = b
        .ok(
            "GET",
            &format!("/mileage/trips/{}", id(&crossing)),
            json!(null),
        )
        .await;
    assert_eq!(crossing_now["claim_amount"], 24_000, "{crossing_now}");
    assert_eq!(crossing_now["tier2_distance_km"], "0.0");
    let after_now = b
        .ok(
            "GET",
            &format!("/mileage/trips/{}", id(&after)),
            json!(null),
        )
        .await;
    assert_eq!(after_now["claim_amount"], 12_000, "{after_now}");

    // The books follow: 100 km + 200 km + 100 km at $1.20 = $480.
    let pl = b
        .ok(
            "GET",
            "/reports/profit_and_loss?from=2026-04-01&to=2027-03-31&basis=cash",
            json!(null),
        )
        .await;
    assert!(pl.to_string().contains("48000"), "{pl}");
}

#[tokio::test]
async fn a_business_over_60000_in_12_months_is_told_to_register_for_gst() {
    // IRD: register for GST when turnover was over $60,000 in the last 12 months.
    let app = TestApp::new().await;
    let user = app.register("ird-threshold").await;
    let created = app
        .post(
            "/api/v1/businesses",
            &user.auth(),
            json!({ "name": "Unregistered Ltd", "complete_onboarding": true }),
        )
        .await;
    let base = format!(
        "/api/v1/businesses/{}",
        created.body["id"].as_str().expect("id")
    );
    let today = chrono::NaiveDate::parse_from_str(
        created.body["today"].as_str().expect("today"),
        "%Y-%m-%d",
    )
    .expect("date");
    let customer = app
        .post(
            &format!("{base}/customers"),
            &user.auth(),
            json!({ "name": "Client" }),
        )
        .await;
    let sell = |dollars: i64, days_ago: i64| {
        let date = (today - chrono::Duration::days(days_ago)).to_string();
        json!({ "customer_id": customer.body["id"], "issue_date": date, "due_date": date,
                "lines": [{ "description": "Work", "quantity": "1", "unit_price": dollars * 100, "tax_treatment": "no_gst" }] })
    };
    for (dollars, days_ago) in [(30_000, 200), (29_000, 20)] {
        let draft = app
            .post(
                &format!("{base}/invoices"),
                &user.auth(),
                sell(dollars, days_ago),
            )
            .await;
        let id = draft.body["id"].as_str().expect("id");
        app.post(
            &format!("{base}/invoices/{id}/issue"),
            &user.auth(),
            json!({}),
        )
        .await;
    }
    assert!(
        !notices(&app, &base, &user).await.contains("GST threshold"),
        "$59,000: not yet"
    );
    let draft = app
        .post(&format!("{base}/invoices"), &user.auth(), sell(2_000, 5))
        .await;
    let id = draft.body["id"].as_str().expect("id");
    app.post(
        &format!("{base}/invoices/{id}/issue"),
        &user.auth(),
        json!({}),
    )
    .await;
    let text = notices(&app, &base, &user).await;
    assert!(
        text.contains("$61,000.00, over the $60,000 GST threshold"),
        "{text}"
    );
    assert!(text.contains("register for GST within 21 days"), "{text}");
}

async fn notices(app: &TestApp, base: &str, user: &TestUser) -> String {
    let response = app.get(&format!("{base}/insights"), &user.auth()).await;
    response.body["notices"].to_string()
}

#[tokio::test]
async fn an_asset_over_10000_gets_change_of_use_adjustments_at_balance_date() {
    // IR375 (Ben): a car bought in May for $28,750 including $3,750 GST, intended 80% business, so
    // $3,000 is claimed. At the end of the income year it was used 100% for business: claim the other
    // $750 in the return that includes the balance date. Five adjustment periods ($20,001-$500,000).
    let b = Books::new("ird-change-of-use", "payments", "2025-01-01").await;
    let car = b
        .ok(
            "POST",
            "/assets",
            json!({ "name": "Car", "asset_group": "Motor vehicles", "cost": 2_500_000, "purchase_gst": 300_000,
                    "acquired_on": "2025-05-14", "method": "dv", "rate": "30", "private_use": "20" }),
        )
        .await;
    let id = car["id"].as_str().expect("id").to_owned();
    // The GST on the purchase is claimed in the return it was bought in.
    assert_eq!(b.gst_period("2025-05-31").await["claimable"], 300_000);

    let path = format!("/assets/{id}/use-reviews");
    let before = b.ok("GET", &path, json!(null)).await;
    assert_eq!(before["applies"], true, "{before}");
    assert_eq!(before["full_gst"], 375_000);
    assert_eq!(before["intended_use"], "80.00");
    let periods = before["periods"].as_array().expect("periods");
    assert_eq!(periods.len(), 5);
    assert_eq!(periods[0]["period_end"], "2026-03-31");
    assert_eq!(periods[0]["months"], 11);
    assert_eq!(periods[0]["reviewable"], true);

    // A later period can't be reviewed first, nor one that hasn't ended.
    let auth = b.user.auth();
    let early = b
        .app
        .put(
            &format!("{}{path}", b.base),
            &auth,
            json!({ "period_end": "2027-03-31", "business_use": "100" }),
        )
        .await;
    assert_eq!(
        early.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        early.body
    );
    assert!(early.field_error("business_use").is_some());

    let reviewed = b
        .ok(
            "PUT",
            &path,
            json!({ "period_end": "2026-03-31", "business_use": "100", "notes": "Logbook" }),
        )
        .await;
    assert_eq!(reviewed["periods"][0]["actual_use"], "100.0", "{reviewed}");
    assert_eq!(reviewed["periods"][0]["adjustment"], 75_000);
    assert_eq!(reviewed["total_adjustment"], 75_000);
    // The $750 is claimed in the February-March return.
    assert_eq!(b.gst_period("2026-03-31").await["claimable"], 75_000);

    // Removing the review reverses the adjustment.
    b.ok(
        "PUT",
        &path,
        json!({ "period_end": "2026-03-31", "business_use": null }),
    )
    .await;
    assert_eq!(b.gst_period("2026-03-31").await["claimable"], 0);

    // Using it less than intended pays GST back: 50% → (50 − 80) × $3,750 = $1,125 back.
    b.ok(
        "PUT",
        &path,
        json!({ "period_end": "2026-03-31", "business_use": "50" }),
    )
    .await;
    assert_eq!(b.gst_period("2026-03-31").await["claimable"], -112_500);

    // An asset of $10,000 or less needs no adjustments.
    let laptop = b
        .ok(
            "POST",
            "/assets",
            json!({ "name": "Laptop", "asset_group": "Computer equipment", "cost": 300_000, "purchase_gst": 45_000,
                    "acquired_on": "2025-06-01", "method": "dv", "rate": "50" }),
        )
        .await;
    let small = b
        .ok(
            "GET",
            &format!("/assets/{}/use-reviews", laptop["id"].as_str().expect("id")),
            json!(null),
        )
        .await;
    assert_eq!(small["applies"], false, "{small}");

    // Another business can't see the reviews.
    let other = b.app.register("ird-change-of-use-other").await;
    let response = b.app.get(&format!("{}{path}", b.base), &other.auth()).await;
    assert!(
        response.status == StatusCode::NOT_FOUND || response.status == StatusCode::FORBIDDEN,
        "{}",
        response.status
    );
}

#[tokio::test]
async fn the_dashboard_lists_upcoming_key_dates_with_the_days_left() {
    let b = Books::new("ird-key-dates", "payments", "2026-01-01").await;
    // GST collected in February 2026 and never paid: the February-March return is overdue.
    let invoice = b
        .invoice(
            "2026-02-10",
            json!([line("Consulting", "1", 100_000, "standard")]),
            false,
        )
        .await;
    b.pay(&invoice, "2026-02-20", 115_000, 0).await;
    let dates = b.ok("GET", "/key-dates", json!(null)).await;
    let today =
        chrono::NaiveDate::parse_from_str(dates["today"].as_str().expect("today"), "%Y-%m-%d")
            .expect("today");
    let items = dates["items"].as_array().expect("items");
    assert!(!items.is_empty() && items.len() <= 6, "{dates}");

    let first = &items[0];
    assert_eq!(first["kind"], "gst_return", "overdue first: {dates}");
    assert_eq!(first["urgency"], "overdue");
    assert_eq!(
        first["date"], "2026-05-07",
        "IRD: 7 May for a period ending 31 March"
    );
    assert_eq!(first["amount"], 15_000);
    assert_eq!(first["link"], "/gst");

    for item in items {
        let date =
            chrono::NaiveDate::parse_from_str(item["date"].as_str().expect("date"), "%Y-%m-%d")
                .expect("date");
        assert_eq!(item["days_left"], (date - today).num_days(), "{item}");
    }
    // After the overdue ones, the rest are in date order and not past.
    let upcoming: Vec<&str> = items
        .iter()
        .filter(|i| i["urgency"] != "overdue")
        .map(|i| i["date"].as_str().expect("date"))
        .collect();
    let mut sorted = upcoming.clone();
    sorted.sort();
    assert_eq!(upcoming, sorted);
    assert!(
        upcoming
            .iter()
            .all(|d| *d >= dates["today"].as_str().expect("today"))
    );
    // The end of the financial year is always there (when six dates don't come first).
    let year_end = b.business["current_financial_year"]["end"].clone();
    assert!(
        items
            .iter()
            .any(|i| i["kind"] == "year_end" && i["date"] == year_end)
            || items.len() == 6,
        "{dates}"
    );
}

#[tokio::test]
async fn selling_an_asset_recovers_depreciation_or_records_a_loss() {
    // IR260: no depreciation in the year of sale; the sale against the value at the start of that
    // year gives depreciation recovered (taxable) or a loss (deductible).
    let b = Books::new("asset-disposal", "payments", "2025-01-01").await;
    let laptop = b
        .ok(
            "POST",
            "/assets",
            json!({ "name": "Laptop", "asset_group": "Computer equipment", "cost": 300_000, "purchase_gst": 45_000,
                    "acquired_on": "2025-06-01", "method": "dv", "rate": "50" }),
        )
        .await;
    let id = laptop["id"].as_str().expect("id").to_owned();
    let path = format!("/assets/{id}/disposal");
    let auth = b.user.auth();

    // The value at the start of the year of sale must be known first.
    let early = b
        .app
        .put(
            &format!("{}{path}", b.base),
            &auth,
            json!({ "disposed_on": "2026-08-10", "sale_price": 230_000 }),
        )
        .await;
    assert_eq!(early.status, StatusCode::CONFLICT, "{}", early.body);

    // FY2026: 10 months at 50% → $1,250 depreciation, $1,750 left.
    b.ok(
        "POST",
        "/assets/depreciation",
        json!({ "year_end": "2026-03-31" }),
    )
    .await;
    let sold = b
        .ok(
            "PUT",
            &path,
            json!({ "disposed_on": "2026-08-10", "sale_price": 230_000, "notes": "Sold to a friend" }),
        )
        .await;
    assert_eq!(sold["book_value"], 175_000, "{sold}");
    assert_eq!(sold["sale_gst"], 30_000, "3/23 of $2,300");
    assert_eq!(sold["recovered"], 25_000, "$2,000 against $1,750");
    assert_eq!(sold["loss"], 0);

    // The GST on the sale goes in the August return; the recovery is income for FY2027.
    assert_eq!(b.gst_period("2026-09-30").await["collected"], 30_000);
    async fn fy27_profit_and_loss(b: &Books) -> String {
        b.ok(
            "GET",
            "/reports/profit_and_loss?from=2026-04-01&to=2027-03-31&basis=accrual",
            json!(null),
        )
        .await
        .to_string()
    }
    assert!(
        fy27_profit_and_loss(&b)
            .await
            .contains("Depreciation recovered")
    );

    // No depreciation in the year of sale, and the earlier year can't change under it.
    let fy27 = b
        .ok(
            "POST",
            "/assets/depreciation",
            json!({ "year_end": "2027-03-31" }),
        )
        .await;
    assert_eq!(fy27["depreciation"], 0, "{fy27}");
    let remove = b
        .app
        .delete(
            &format!("{}/assets/depreciation?year_end=2026-03-31", b.base),
            &auth,
            None,
        )
        .await;
    assert_eq!(remove.status, StatusCode::CONFLICT, "{}", remove.body);

    // The schedule shows it going out.
    let schedule = b
        .ok(
            "GET",
            "/reports/depreciation_schedule?from=2026-04-01&to=2027-03-31",
            json!(null),
        )
        .await;
    let total = schedule["sections"]
        .as_array()
        .and_then(|s| s.iter().find(|s| s["title"] == "Total"))
        .expect("total")["totals"]
        .clone();
    assert_eq!(total[4], 175_000, "disposals: {schedule}");

    // Scrapped instead: the whole value is a loss.
    let scrapped = b
        .ok(
            "PUT",
            &path,
            json!({ "disposed_on": "2026-08-10", "sale_price": 0 }),
        )
        .await;
    assert_eq!(
        (&scrapped["loss"], &scrapped["recovered"]),
        (&json!(175_000), &json!(0))
    );
    assert!(
        fy27_profit_and_loss(&b)
            .await
            .contains("Loss on disposal of assets")
    );

    // Undoing the sale reverses it.
    let undone = b
        .app
        .delete(&format!("{}{path}", b.base), &auth, None)
        .await;
    assert_eq!(undone.status, StatusCode::NO_CONTENT, "{}", undone.body);
    assert!(
        !fy27_profit_and_loss(&b)
            .await
            .contains("Loss on disposal of assets")
    );
    assert_eq!(b.gst_period("2026-09-30").await["collected"], 0);
}

#[tokio::test]
async fn a_low_value_asset_is_deducted_in_full_in_the_year_bought() {
    // IR260: an asset costing $1,000 or less (excluding GST) can be deducted in full in the year it
    // was bought. A $900 monitor bought in March, the last month of the year: $900, not 1/12 of a rate.
    let b = Books::new("ird-low-value", "payments", "2020-01-01").await;
    let asset = |name: &str, cost: i64| {
        json!({ "name": name, "asset_group": "Computer equipment", "cost": cost, "purchase_gst": cost * 15 / 100,
                "acquired_on": "2026-03-15", "method": "full", "rate": "100", "private_use": "0" })
    };
    b.ok("POST", "/assets", asset("Monitor", 90_000)).await;
    let year = b
        .ok(
            "POST",
            "/assets/depreciation",
            json!({ "year_end": "2026-03-31" }),
        )
        .await;
    assert_eq!(year["depreciation"], 90_000, "{year}");
    assert_eq!(year["lines"][0]["closing_value"], 0);

    let too_dear = b
        .app
        .post(
            &format!("{}/assets", b.base),
            &b.user.auth(),
            asset("Laptop", 120_000),
        )
        .await;
    assert_eq!(
        too_dear.field_error("method"),
        Some("Only an asset costing $1,000 or less (excluding GST) can be deducted in full.")
    );
}

#[tokio::test]
async fn entertainment_is_half_deductible_with_irds_yearly_gst_adjustment() {
    // IR375 / IR268: a $230 (GST-inclusive) client lunch, 50% deductible: claim the $30 GST during the
    // year, then a $15 adjustment (15% of the $100 non-deductible GST-exclusive amount) in the return
    // for the period the year ends in. The profit and loss shows $100 as an expense and $100 as not
    // deductible; the income tax estimate deducts $100.
    let b = Books::new("ird-entertainment", "payments", "2020-01-01").await;
    b.ok(
        "POST",
        "/expenses",
        json!({ "expense_date": "2025-05-10", "supplier": "Restaurant", "category_id": b.category().await,
                "gross_amount": 23_000, "gst_treatment": "standard", "payment_method": "business_account",
                "classification": "business", "deductible_percentage": "50" }),
    )
    .await;

    let may = b.gst_period("2025-05-31").await;
    assert_eq!(
        (&may["claimable"], &may["adjustments"]),
        (&json!(3_000), &json!(0))
    );
    let year_end = b.gst_period("2026-03-31").await;
    assert_eq!(
        (&year_end["adjustments"], &year_end["net"]),
        (&json!(1_500), &json!(1_500)),
        "the yearly adjustment lands in the period the financial year ends in"
    );

    let pl = b
        .ok(
            "GET",
            "/reports/profit_and_loss?from=2025-04-01&to=2026-03-31&basis=cash",
            json!(null),
        )
        .await;
    let rows: Vec<Value> = pl["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .flat_map(|s| {
            s["rows"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .chain(s["totals"].as_array().map(|t| Value::Array(t.clone())))
        })
        .collect();
    let row = |label: &str| {
        rows.iter()
            .find(|r| r[0].as_str().is_some_and(|l| l.contains(label)))
            .cloned()
            .unwrap_or_else(|| panic!("no {label}: {pl}"))
    };
    assert_eq!(row("Total expenses")[1], 10_000);
    assert_eq!(row("(non-deductible share)")[1], 10_000);
    assert_eq!(row("Net profit before adjustments")[1], -10_000);
    let net = rows
        .iter()
        .find(|r| r[0] == "Net profit")
        .expect("net profit");
    assert_eq!(
        net[1], -20_000,
        "the whole $200 is a cost; half isn't deductible: {pl}"
    );

    let tax = b.ok("GET", "/income-tax", json!(null)).await;
    let fy26 = tax["years"]
        .as_array()
        .expect("years")
        .iter()
        .find(|y| y["label"] == "FY2026")
        .cloned()
        .expect("FY2026");
    assert_eq!(fy26["deductible_expenses"], 10_000);
}

#[tokio::test]
async fn a_bad_debt_written_off_gets_its_gst_back_in_that_period_on_the_invoice_basis() {
    // IR375, "Bad debts written off": on the invoice or hybrid basis, a credit adjustment of 3/23 of the
    // amount written off, in the period it's written off; on the payments basis nothing (the sale was
    // never counted).
    let b = Books::new("ird-bad-debt", "invoice", "2020-01-01").await;
    let invoice = b
        .invoice(
            "2026-04-10",
            json!([line("Work", "1", 100_000, "standard")]),
            false,
        )
        .await;
    b.pay(&invoice, "2026-05-10", 57_500, 0).await;
    let id = invoice["id"].as_str().expect("id");
    let refused = b
        .app
        .post(
            &format!("{}/invoices/{id}/payments", b.base),
            &b.user.auth(),
            json!({ "payment_date": "2026-06-15", "amount": 57_500, "withholding_tax": 100, "method": "write_off" }),
        )
        .await;
    assert_eq!(
        refused.field_error("withholding_tax"),
        Some("Nothing is withheld from an amount written off.")
    );
    let written_off = b
        .ok(
            "POST",
            &format!("/invoices/{id}/payments"),
            json!({ "payment_date": "2026-06-15", "amount": 57_500, "method": "write_off", "notes": "Customer went into liquidation" }),
        )
        .await;
    assert_eq!(written_off["display_status"], "written_off");
    assert_eq!(written_off["balance"], 0);

    assert_eq!(b.gst_period("2026-05-31").await["collected"], 15_000);
    assert_eq!(
        b.gst_period("2026-07-31").await["collected"],
        -7_500,
        "3/23 of the $575 written off, in the period it was written off"
    );
    let pl = b
        .ok(
            "GET",
            "/reports/profit_and_loss?from=2026-04-01&to=2027-03-31&basis=accrual",
            json!(null),
        )
        .await;
    let bad_debts = pl["sections"][1]["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .find(|r| r[0] == "Bad debts")
        .cloned()
        .expect("a bad debts expense");
    assert_eq!(bad_debts[1], 50_000);
    let receivables = b.ok("GET", "/reports/receivables", json!(null)).await;
    assert!(
        !receivables.to_string().contains("INV-0001"),
        "nothing left owing: {receivables}"
    );

    // Payments basis: the $575 received counts; the write-off doesn't.
    b.set_gst_basis("payments").await;
    assert_eq!(b.gst_period("2026-05-31").await["collected"], 7_500);
    assert_eq!(b.gst_period("2026-07-31").await["collected"], 0);
}

#[tokio::test]
async fn home_office_by_irds_square_metre_rate() {
    // ird.govt.nz/home-office-expenses, square metre rate option: (a × b) + (c × d). 20 m² of a 200 m²
    // home, $10,000 of mortgage interest, rates and rent, 2025-26's $57.30 a m²: $1,000 + $1,146.
    let b = Books::new("ird-home-office", "invoice", "2020-01-01").await;
    let year = b
        .ok("GET", "/home-office?year_end=2026-03-31", json!(null))
        .await;
    assert_eq!(year["rate"], 5_730);
    assert!(year["claim"].is_null());
    let saved = b
        .ok(
            "PUT",
            "/home-office",
            json!({ "year_end": "2026-03-31", "business_area": "20", "total_area": "200", "premises_costs": 1_000_000 }),
        )
        .await;
    assert_eq!(saved["claim"]["claim"], 214_600);
    let pl = b
        .ok(
            "GET",
            "/reports/profit_and_loss?from=2025-04-01&to=2026-03-31&basis=accrual",
            json!(null),
        )
        .await;
    assert!(pl.to_string().contains("[\"Home office\",214600"), "{pl}");
    let gst = b.ok("GET", "/gst/returns", json!(null)).await;
    assert_eq!(
        gst["returns"],
        json!([]),
        "no GST on the home office claim: {gst}"
    );

    // Saving again replaces the year's claim (2024-25's rate was $55.60).
    let previous = b
        .ok(
            "PUT",
            "/home-office",
            json!({ "year_end": "2025-03-31", "business_area": "12.5", "total_area": "110" }),
        )
        .await;
    assert_eq!(previous["claim"]["claim"], 69_500);

    let auth = b.user.auth();
    let wrong = b
        .app
        .put(
            &format!("{}/home-office", b.base),
            &auth,
            json!({ "year_end": "2026-02-28", "business_area": "20", "total_area": "200" }),
        )
        .await;
    assert_eq!(
        wrong.field_error("year_end"),
        Some("Choose the last day of a financial year.")
    );
    let too_big = b
        .app
        .put(
            &format!("{}/home-office", b.base),
            &auth,
            json!({ "year_end": "2026-03-31", "business_area": "300", "total_area": "200" }),
        )
        .await;
    assert!(
        too_big.field_error("business_area").is_some(),
        "{}",
        too_big.body
    );
    let unpublished = b
        .app
        .put(
            &format!("{}/home-office", b.base),
            &auth,
            json!({ "year_end": "2099-03-31", "business_area": "20", "total_area": "200" }),
        )
        .await;
    assert!(
        unpublished.field_error("year_end").is_some(),
        "{}",
        unpublished.body
    );

    // Another owner can't see it.
    let intruder = b.app.register("ird-home-intruder").await;
    assert_eq!(
        b.app
            .get(
                &format!("{}/home-office?year_end=2026-03-31", b.base),
                &intruder.auth()
            )
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    // Removing it removes the expense.
    let removed = b
        .app
        .delete(
            &format!("{}/home-office?year_end=2026-03-31", b.base),
            &auth,
            None,
        )
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    let pl = b
        .ok(
            "GET",
            "/reports/profit_and_loss?from=2025-04-01&to=2026-03-31&basis=accrual",
            json!(null),
        )
        .await;
    assert!(
        pl.to_string().contains("[\"Home office\",0,69500"),
        "only last year's claim is left: {pl}"
    );
}
