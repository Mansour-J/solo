//! Reports against a hand-checked data set: a GST-registered business with three months of invoices,
//! payments, income and expenses (April – June 2026) and one sale in March (the comparison period).
//!
//! Worked by hand (all GST 15%):
//! - Invoice 1 (Tūī Media): $1,000 + $150 GST, issued 10 Apr, paid in full 20 Apr.
//! - Invoice 2 (Kākā Studio): $2,000 + $300 GST, issued 5 May, $1,000 paid 10 Jun.
//! - Invoice 3 (Tūī Media): $400 + $60 GST, issued 15 Jun, due in 30 days, unpaid.
//! - Income: $230 market sale (incl. $30 GST) 12 May; $50 interest (exempt) 1 Jun; $115 sale 15 Mar.
//! - Expenses: Xero $115 (software, $15 GST) 15 Apr; Spark $115 internet, 60% business 20 May
//!   ($60 expense, $9 GST, $46 private).
//!
//! Accrual, April – June: sales $3,600, other income $50, expenses $160, net profit $3,490;
//! GST collected $540, claimable $24. Cash: invoice 2's $1,000 payment carries $130.43 of GST
//! (300 × 1,000 / 2,300), so sales are $1,000 + $869.57 + $200 = $2,069.57; GST collected $310.43.

mod common;

use axum::http::{StatusCode, header};
use common::{TestApp, TestUser};
use serde_json::{Value, json};
use uuid::Uuid;

struct Books {
    app: TestApp,
    user: TestUser,
    business: Uuid,
    tui: String,
    kaka: String,
    today: String,
}

const Q1: &str = "from=2026-04-01&to=2026-06-30";

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
        let business: Uuid = created.body["id"]
            .as_str()
            .expect("id")
            .parse()
            .expect("uuid");
        let mut books = Self {
            app,
            user,
            business,
            tui: String::new(),
            kaka: String::new(),
            today: String::new(),
        };
        books.today = books.get("").await.body["today"]
            .as_str()
            .expect("today")
            .to_owned();
        books.tui = books.customer("Tūī Media").await;
        books.kaka = books.customer("Kākā Studio").await;
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

    async fn customer(&self, name: &str) -> String {
        self.post("/customers", json!({ "name": name })).await.body["id"]
            .as_str()
            .expect("id")
            .to_owned()
    }

    async fn invoice(&self, customer: &str, issue: &str, due: &str, net: i64) -> String {
        let draft = self
            .post(
                "/invoices",
                json!({ "customer_id": customer, "issue_date": issue, "due_date": due,
                        "lines": [{ "description": "Design work", "quantity": "1", "unit_price": net, "tax_treatment": "standard" }] }),
            )
            .await;
        let id = draft.body["id"].as_str().expect("id").to_owned();
        self.post(&format!("/invoices/{id}/issue"), json!({})).await;
        id
    }

    async fn pay(&self, invoice: &str, date: &str, amount: i64) {
        self.post(
            &format!("/invoices/{invoice}/payments"),
            json!({ "payment_date": date, "amount": amount, "method": "bank_transfer", "reference": "INV" }),
        )
        .await;
    }

    async fn category(&self, name: &str) -> Value {
        self.get("/expense-categories")
            .await
            .body
            .as_array()
            .expect("categories")
            .iter()
            .find(|c| c["name"] == name)
            .expect("category")["id"]
            .clone()
    }

    /// The data set described at the top of the file.
    async fn seed(&self) {
        let due = shift(&self.today, 30);
        let one = self
            .invoice(&self.tui, "2026-04-10", "2026-04-30", 100_000)
            .await;
        self.pay(&one, "2026-04-20", 115_000).await;
        let two = self
            .invoice(&self.kaka, "2026-05-05", "2026-05-20", 200_000)
            .await;
        self.pay(&two, "2026-06-10", 100_000).await;
        self.invoice(&self.tui, "2026-06-15", &due, 40_000).await;
        for (date, payer, kind, gross, treatment) in [
            ("2026-03-15", "Market stall", "sales", 11_500, "standard"),
            ("2026-05-12", "Market stall", "sales", 23_000, "standard"),
            ("2026-06-01", "Bank interest", "other", 5_000, "exempt"),
        ] {
            self.post(
                "/income",
                json!({ "income_date": date, "payer": payer, "income_type": kind, "gross_amount": gross,
                        "gst_treatment": treatment, "payment_method": "bank_transfer" }),
            )
            .await;
        }
        let software = self.category("Software").await;
        let internet = self.category("Internet").await;
        self.post(
            "/expenses",
            json!({ "expense_date": "2026-04-15", "supplier": "Xero", "category_id": software, "gross_amount": 11_500,
                    "gst_treatment": "standard", "payment_method": "business_account", "classification": "business" }),
        )
        .await;
        self.post(
            "/expenses",
            json!({ "expense_date": "2026-05-20", "supplier": "Spark", "category_id": internet, "gross_amount": 11_500,
                    "gst_treatment": "standard", "payment_method": "business_account", "classification": "partial",
                    "business_percentage": "60" }),
        )
        .await;
    }

    async fn report(&self, kind: &str, query: &str) -> Value {
        let response = self.get(&format!("/reports/{kind}?{query}")).await;
        assert_eq!(response.status, StatusCode::OK, "{kind}: {}", response.body);
        response.body
    }
}

fn shift(date: &str, days: i64) -> String {
    let d = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").expect("date");
    (d + chrono::Duration::days(days)).to_string()
}

fn section<'a>(report: &'a Value, title: &str) -> &'a Value {
    report["sections"]
        .as_array()
        .expect("sections")
        .iter()
        .find(|s| s["title"] == title)
        .unwrap_or_else(|| panic!("section {title} in {}", report["title"]))
}

/// The row whose first cell is `label`.
fn row<'a>(section: &'a Value, label: &str) -> &'a Vec<Value> {
    section["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter_map(Value::as_array)
        .find(|r| r[0] == label)
        .unwrap_or_else(|| panic!("row {label} in {}", section["title"]))
}

fn totals(section: &Value) -> &Vec<Value> {
    section["totals"].as_array().expect("totals")
}

#[tokio::test]
async fn profit_and_loss_matches_the_hand_worked_figures_on_both_bases() {
    let b = Books::new("reports-pnl").await;
    b.seed().await;

    let accrual = b
        .report("profit_and_loss", &format!("{Q1}&basis=accrual"))
        .await;
    assert_eq!(accrual["basis"], "accrual");
    assert_eq!(
        accrual["period"],
        json!({ "from": "2026-04-01", "to": "2026-06-30" })
    );
    let income = section(&accrual, "Income");
    // Account, this period, previous period (January – March), change.
    assert_eq!(
        row(income, "Sales")[1..],
        [json!(360_000), json!(10_000), json!(350_000)]
    );
    assert_eq!(row(income, "Other income")[1], 5_000);
    assert_eq!(totals(income)[1], 365_000);
    let expenses = section(&accrual, "Expenses");
    assert_eq!(row(expenses, "Software")[1], 10_000);
    assert_eq!(row(expenses, "Internet")[1], 6_000);
    assert_eq!(totals(expenses)[1], 16_000);
    let profit = section(&accrual, "Profit");
    assert_eq!(row(profit, "Gross profit")[1], 360_000);
    assert_eq!(
        row(profit, "Net profit")[1..],
        [json!(349_000), json!(10_000), json!(339_000)]
    );

    let cash = b
        .report("profit_and_loss", &format!("{Q1}&basis=cash"))
        .await;
    assert_eq!(row(section(&cash, "Income"), "Sales")[1], 206_957);
    assert_eq!(row(section(&cash, "Profit"), "Net profit")[1], 195_957);
}

#[tokio::test]
async fn income_and_expense_summaries_group_the_same_totals() {
    let b = Books::new("reports-summaries").await;
    b.seed().await;

    let income = b
        .report("income_summary", &format!("{Q1}&basis=accrual"))
        .await;
    let by_month = section(&income, "By month");
    assert_eq!(row(by_month, "Apr 2026")[1], 100_000);
    assert_eq!(row(by_month, "May 2026")[1], 220_000);
    assert_eq!(row(by_month, "Jun 2026")[1], 45_000);
    assert_eq!(totals(by_month)[1], 365_000);
    let by_customer = section(&income, "By customer");
    assert_eq!(row(by_customer, "Kākā Studio")[1], 200_000);
    assert_eq!(row(by_customer, "Tūī Media")[1], 140_000);
    assert_eq!(row(by_customer, "No customer")[1], 25_000);
    let by_source = section(&income, "By source");
    assert_eq!(row(by_source, "Invoices")[1], 340_000);
    assert_eq!(row(by_source, "Sales without an invoice")[1], 20_000);
    assert_eq!(row(by_source, "Other income")[1], 5_000);
    assert_eq!(
        row(section(&income, "By financial year"), "FY2027")[1],
        365_000
    );
    let invoices = section(&income, "Invoices issued in the period");
    // Label, count, invoiced, paid, outstanding.
    assert_eq!(
        row(invoices, "Paid in full")[1..],
        [json!(1), json!(115_000), json!(115_000), json!(0)]
    );
    assert_eq!(
        row(invoices, "Outstanding")[1..],
        [json!(2), json!(276_000), json!(100_000), json!(176_000)]
    );

    // One customer only.
    let tui = b
        .report(
            "income_summary",
            &format!("{Q1}&basis=accrual&customer_id={}", b.tui),
        )
        .await;
    assert_eq!(tui["subtitle"][0], "Customer: Tūī Media");
    assert_eq!(totals(section(&tui, "By month"))[1], 140_000);

    let expenses = b
        .report("expense_summary", &format!("{Q1}&basis=accrual"))
        .await;
    let by_category = section(&expenses, "By category");
    assert_eq!(row(by_category, "Software")[1], 10_000);
    assert_eq!(totals(by_category)[1], 16_000);
    assert_eq!(row(section(&expenses, "By supplier"), "Spark")[1], 6_000);
    // Business expenses, GST claimed, private share.
    assert_eq!(
        row(section(&expenses, "By month"), "May 2026")[1..],
        [json!(6_000), json!(900), json!(4_600)]
    );
    let split = section(&expenses, "Business and personal");
    assert_eq!(row(split, "GST claimed")[1], 2_400);
    assert_eq!(row(split, "Private share (owner drawings)")[1], 4_600);
    assert_eq!(totals(split)[1], 23_000, "everything paid: $115 + $115");

    let internet = b.category("Internet").await;
    let one = b
        .report(
            "expense_summary",
            &format!("{Q1}&category_id={}", internet.as_str().expect("id")),
        )
        .await;
    assert_eq!(one["subtitle"][0], "Category: Internet");
    assert_eq!(totals(section(&one, "By category"))[1], 6_000);
}

#[tokio::test]
async fn statements_ageing_and_the_financial_statement() {
    let b = Books::new("reports-receivables").await;
    b.seed().await;

    let statement = b
        .report(
            "statement_of_account",
            &format!("{Q1}&customer_id={}", b.kaka),
        )
        .await;
    assert_eq!(statement["subtitle"][0], "For Kākā Studio");
    let transactions = section(&statement, "Transactions");
    let balances: Vec<&Value> = transactions["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|r| &r[4])
        .collect();
    assert_eq!(balances, [&json!(0), &json!(230_000), &json!(130_000)]);
    assert_eq!(
        totals(transactions)[2..],
        [json!(230_000), json!(100_000), json!(130_000)]
    );
    let summary = section(&statement, "Summary");
    assert_eq!(totals(summary)[1], 130_000);
    assert_eq!(row(summary, "Credit balance")[1], 0);
    // A later period brings the earlier activity forward.
    let tui = b
        .report(
            "statement_of_account",
            &format!("from=2026-05-01&to=2026-06-30&customer_id={}", b.tui),
        )
        .await;
    assert_eq!(
        row(section(&tui, "Summary"), "Balance brought forward")[1],
        0
    );
    assert_eq!(totals(section(&tui, "Summary"))[1], 46_000);
    let missing = b.get(&format!("/reports/statement_of_account?{Q1}")).await;
    assert!(
        missing.field_error("customer_id").is_some(),
        "{}",
        missing.body
    );

    let ageing = b.report("receivables", "").await;
    let by_customer = section(&ageing, "Ageing by customer");
    // Customer, current, 1–30, 31–60, 61–90, over 90, total.
    assert_eq!(
        row(by_customer, "Tūī Media")[1..],
        [
            json!(46_000),
            json!(0),
            json!(0),
            json!(0),
            json!(0),
            json!(46_000)
        ]
    );
    assert_eq!(
        row(by_customer, "Kākā Studio")[5..],
        [json!(130_000), json!(130_000)]
    );
    assert_eq!(totals(by_customer)[6], 176_000);

    let statement = b
        .report("financial_statement", &format!("{Q1}&basis=accrual"))
        .await;
    let months = section(&statement, "By month");
    // Month, income, expenses, net, net to date.
    assert_eq!(
        row(months, "Apr 2026")[1..],
        [json!(100_000), json!(10_000), json!(90_000), json!(90_000)]
    );
    assert_eq!(row(months, "Jun 2026")[4], 349_000);
    assert_eq!(
        row(section(&statement, "Summary"), "Net profit")[1],
        349_000
    );
}

#[tokio::test]
async fn gst_summary_lists_the_records_behind_each_figure() {
    let b = Books::new("reports-gst").await;
    b.seed().await;
    let accrual = b.report("gst", &format!("{Q1}&basis=accrual")).await;
    let summary = section(&accrual, "Summary");
    assert_eq!(row(summary, "GST collected on sales and income")[1], 54_000);
    assert_eq!(row(summary, "GST claimable on expenses")[1], 2_400);
    assert_eq!(totals(summary)[..], [json!("GST to pay"), json!(51_600)]);
    let records = section(&accrual, "Records");
    assert_eq!(records["rows"].as_array().map(Vec::len), Some(6));
    assert_eq!(totals(records)[2..], [json!(54_000), json!(2_400)]);
    assert!(accrual["notes"].as_array().expect("notes").iter().any(|n| {
        n.as_str()
            .is_some_and(|n| n.contains("not a filed GST return"))
    }));
    let cash = b.report("gst", &format!("{Q1}&basis=cash")).await;
    assert_eq!(
        row(
            section(&cash, "Summary"),
            "GST collected on sales and income"
        )[1],
        31_043
    );
}

#[tokio::test]
async fn every_report_exports_as_csv_excel_and_pdf() {
    let b = Books::new("reports-exports").await;
    b.seed().await;
    let csv = b
        .get(&format!(
            "/reports/profit_and_loss/export?format=csv&{Q1}&basis=accrual"
        ))
        .await;
    assert_eq!(csv.status, StatusCode::OK, "{}", csv.body);
    assert_eq!(csv.headers[header::CONTENT_TYPE], "text/csv; charset=utf-8");
    assert!(
        csv.headers[header::CONTENT_DISPOSITION]
            .to_str()
            .expect("header")
            .contains("Profit and loss 2026-04-01 to 2026-06-30.csv")
    );
    let text = String::from_utf8(csv.bytes.clone()).expect("utf8");
    assert!(text.contains("Net profit,3490.00,100.00,3390.00"), "{text}");

    for kind in [
        "profit_and_loss",
        "income_summary",
        "expense_summary",
        "receivables",
        "financial_statement",
        "mileage",
        "allocations",
        "gst",
    ] {
        let xlsx = b
            .get(&format!("/reports/{kind}/export?format=xlsx&{Q1}"))
            .await;
        assert_eq!(xlsx.status, StatusCode::OK, "{kind}: {}", xlsx.body);
        assert_eq!(&xlsx.bytes[..2], b"PK", "{kind}");
        let pdf = b
            .get(&format!("/reports/{kind}/export?format=pdf&{Q1}"))
            .await;
        assert_eq!(pdf.status, StatusCode::OK, "{kind}: {}", pdf.body);
        assert_eq!(&pdf.bytes[..4], b"%PDF", "{kind}");
    }
    let statement = b
        .get(&format!(
            "/reports/statement_of_account/export?format=pdf&{Q1}&customer_id={}",
            b.kaka
        ))
        .await;
    assert_eq!(&statement.bytes[..4], b"%PDF");
}

#[tokio::test]
async fn report_filters_are_validated_and_private_to_the_business() {
    let a = Books::new("reports-tenant-a").await;
    a.seed().await;
    let backwards = a
        .get("/reports/profit_and_loss?from=2026-06-30&to=2026-04-01")
        .await;
    assert!(backwards.field_error("to").is_some());

    // Another business can't name A's customer, and its own reports are empty.
    let b = Books::new("reports-tenant-b").await;
    let foreign = b
        .get(&format!(
            "/reports/statement_of_account?{Q1}&customer_id={}",
            a.kaka
        ))
        .await;
    assert!(
        foreign.field_error("customer_id").is_some(),
        "{}",
        foreign.body
    );
    let pnl = b.report("profit_and_loss", Q1).await;
    assert_eq!(totals(section(&pnl, "Income"))[1], 0);
    let direct = b
        .app
        .get(
            &format!(
                "/api/v1/businesses/{}/reports/profit_and_loss?{Q1}",
                a.business
            ),
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
async fn every_report_exports_together_in_one_file() {
    let b = Books::new("reports-export-all").await;
    b.seed().await;
    let titles = [
        "Profit and loss",
        "Balance sheet",
        "Statement of changes in equity",
        "Depreciation schedule",
        "Income summary",
        "Expense summary",
        "Accounts receivable",
        "GST",
        "Mileage",
        "Allocations",
    ];

    let csv = b
        .get(&format!(
            "/reports/all/export?format=csv&{Q1}&basis=accrual"
        ))
        .await;
    assert_eq!(csv.status, StatusCode::OK, "{}", csv.body);
    let disposition = csv.headers[header::CONTENT_DISPOSITION]
        .to_str()
        .expect("header");
    assert!(
        disposition.contains("All reports 2026-04-01 to 2026-06-30.csv"),
        "{disposition}"
    );
    let text = String::from_utf8(csv.bytes.clone()).expect("utf8");
    assert_eq!(
        text.matches('\u{feff}').count(),
        1,
        "one byte-order mark at the start"
    );
    let mut at = 0;
    for title in titles {
        let found = text[at..]
            .find(title)
            .unwrap_or_else(|| panic!("{title} missing or out of order"));
        at += found;
    }
    assert!(!text.contains("Statement of account"), "per customer only");
    assert!(
        text.contains("3600.00"),
        "the accrual sales figure from the profit and loss"
    );

    let xlsx = b
        .get(&format!("/reports/all/export?format=xlsx&{Q1}"))
        .await;
    assert_eq!(xlsx.status, StatusCode::OK);
    assert_eq!(&xlsx.bytes[..2], b"PK");
    let names = String::from_utf8_lossy(&xlsx.bytes);
    assert!(
        names.contains("xl/worksheets/sheet11.xml") && !names.contains("xl/worksheets/sheet12.xml"),
        "one worksheet per report"
    );

    let pdf = b.get(&format!("/reports/all/export?format=pdf&{Q1}")).await;
    assert_eq!(pdf.status, StatusCode::OK, "{}", pdf.body);
    assert!(pdf.bytes.starts_with(b"%PDF-"));
    let single = b
        .get(&format!("/reports/profit_and_loss/export?format=pdf&{Q1}"))
        .await;
    assert!(
        pdf.bytes.len() > single.bytes.len() * 3,
        "every report is in it"
    );
}
