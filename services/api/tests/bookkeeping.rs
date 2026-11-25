//! Customers, invoices, payments, quotes, expenses, income, PDFs, email and the ledger behind them.

mod common;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use common::{TestApp, TestUser};
use serde_json::{Value, json};
use uuid::Uuid;

/// A GST-registered business with one customer.
struct Books {
    app: TestApp,
    user: TestUser,
    business: Uuid,
    customer: Uuid,
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
        let business: Uuid = created.body["id"]
            .as_str()
            .expect("id")
            .parse()
            .expect("uuid");
        let customer = app
            .post(
                &format!("/api/v1/businesses/{business}/customers"),
                &user.auth(),
                json!({ "name": "Tūī Media", "email": "accounts@tui.example" }),
            )
            .await;
        assert_eq!(customer.status, StatusCode::CREATED, "{}", customer.body);
        let customer: Uuid = customer.body["id"]
            .as_str()
            .expect("id")
            .parse()
            .expect("uuid");
        Self {
            app,
            user,
            business,
            customer,
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

    /// Draft with $1,000.00 + GST: 8 hours at $125.
    async fn draft(&self, issue_date: &str, due_date: &str) -> Value {
        let response = self
            .post(
                "/invoices",
                json!({ "customer_id": self.customer, "issue_date": issue_date, "due_date": due_date,
                        "lines": [{ "description": "Design work", "quantity": "8", "unit_price": 12_500, "tax_treatment": "standard" }] }),
            )
            .await;
        assert_eq!(response.status, StatusCode::CREATED, "{}", response.body);
        response.body
    }

    async fn issued(&self, issue_date: &str, due_date: &str) -> Value {
        let draft = self.draft(issue_date, due_date).await;
        let id = draft["id"].as_str().expect("id");
        let issued = self.post(&format!("/invoices/{id}/issue"), json!({})).await;
        assert_eq!(issued.status, StatusCode::OK, "{}", issued.body);
        issued.body
    }

    async fn dashboard(&self, basis: &str) -> Value {
        let response = self
            .get(&format!(
                "/dashboard?from=2026-01-01&to=2026-12-31&basis={basis}"
            ))
            .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.body);
        response.body
    }

    /// Debits equal credits across the whole ledger.
    async fn assert_ledger_balances(&self) {
        let mut tx = self
            .app
            .state
            .db
            .begin_business(self.user.id, self.business)
            .await
            .expect("tx");
        let (debits, credits): (i64, i64) =
            sqlx::query_as("SELECT coalesce(sum(debit), 0)::bigint, coalesce(sum(credit), 0)::bigint FROM journal_entry_lines")
                .fetch_one(&mut *tx)
                .await
                .expect("sum");
        assert_eq!(debits, credits, "ledger out of balance");
    }
}

#[tokio::test]
async fn invoice_lifecycle_posts_balanced_ledger_entries_on_both_bases() {
    let b = Books::new("lifecycle").await;
    let draft = b.draft("2026-03-02", "2026-03-31").await;
    assert_eq!(draft["status"], "draft");
    assert!(draft["number"].is_null());
    assert_eq!(
        (
            draft["subtotal"].as_i64(),
            draft["gst_total"].as_i64(),
            draft["total"].as_i64()
        ),
        (Some(100_000), Some(15_000), Some(115_000))
    );
    assert!(
        draft["journal"].as_array().expect("journal").is_empty(),
        "drafts aren't posted"
    );

    let id = draft["id"].as_str().expect("id");
    let issued = b
        .post(&format!("/invoices/{id}/issue"), json!({}))
        .await
        .body;
    assert_eq!(issued["number"], "INV-0001");
    assert_eq!(
        issued["display_status"], "overdue",
        "due date is in the past"
    );

    // Accrual: income when invoiced. Cash: nothing until paid.
    assert_eq!(b.dashboard("accrual").await["totals"]["income"], 100_000);
    assert_eq!(b.dashboard("cash").await["totals"]["income"], 0);
    assert_eq!(b.dashboard("accrual").await["gst"]["collected"], 15_000);

    // Partial payment.
    let paid = b
        .post(
            &format!("/invoices/{id}/payments"),
            json!({ "payment_date": "2026-03-10", "amount": 46_000, "method": "bank_transfer" }),
        )
        .await;
    assert_eq!(paid.status, StatusCode::CREATED, "{}", paid.body);
    assert_eq!(paid.body["display_status"], "overdue");
    assert_eq!(paid.body["balance"], 69_000);
    let cash = b.dashboard("cash").await;
    assert_eq!(
        cash["totals"]["income"], 40_000,
        "46,000 of 115,000 = 40% of the 100,000 net"
    );
    assert_eq!(cash["gst"]["collected"], 6_000);

    // Overpaying is refused.
    let over = b
        .post(
            &format!("/invoices/{id}/payments"),
            json!({ "payment_date": "2026-03-11", "amount": 69_001, "method": "cash" }),
        )
        .await;
    assert_eq!(over.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(over.field_error("amount").is_some());

    // Final payment: cash basis recognises exactly the invoice.
    let done = b
        .post(
            &format!("/invoices/{id}/payments"),
            json!({ "payment_date": "2026-03-20", "amount": 69_000, "method": "bank_transfer" }),
        )
        .await;
    assert_eq!(done.body["display_status"], "paid");
    let cash = b.dashboard("cash").await;
    assert_eq!(cash["totals"]["income"], 100_000);
    assert_eq!(cash["gst"]["collected"], 15_000);
    assert_eq!(cash["receivables"]["outstanding_amount"], 0);
    b.assert_ledger_balances().await;
}

#[tokio::test]
async fn corrections_reverse_and_repost_instead_of_rewriting_history() {
    let b = Books::new("corrections").await;
    let invoice = b.issued("2026-04-01", "2026-04-30").await;
    let id = invoice["id"].as_str().expect("id");
    let with_payment = b
        .post(
            &format!("/invoices/{id}/payments"),
            json!({ "payment_date": "2026-04-05", "amount": 15_000, "method": "card" }),
        )
        .await
        .body;
    let payment_id = with_payment["payments"][0]["id"]
        .as_str()
        .expect("payment")
        .to_owned();
    let entries_before = with_payment["journal"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);

    // Editing a payment reverses its old entries and posts new ones.
    let edited = b
        .app
        .put(
            &b.path(&format!("/invoices/{id}/payments/{payment_id}")),
            &b.user.auth(),
            json!({ "payment_date": "2026-04-05", "amount": 20_000, "method": "card" }),
        )
        .await;
    assert_eq!(edited.status, StatusCode::OK, "{}", edited.body);
    let journal = edited.body["journal"].as_array().expect("journal");
    assert!(journal.len() > entries_before);
    assert!(
        journal.iter().any(|e| !e["reverses_entry_id"].is_null()),
        "a reversal was posted"
    );
    assert_eq!(edited.body["paid"], 20_000);

    // Lowering the invoice below what's been paid is refused.
    let too_low = b
        .app
        .put(
            &b.path(&format!("/invoices/{id}")),
            &b.user.auth(),
            json!({ "customer_id": b.customer, "issue_date": "2026-04-01", "due_date": "2026-04-30",
                    "lines": [{ "description": "Design work", "quantity": "1", "unit_price": 10_000, "tax_treatment": "standard" }] }),
        )
        .await;
    assert_eq!(too_low.status, StatusCode::UNPROCESSABLE_ENTITY);

    // Cancelling needs the payment removed first; removing it reverses its entries.
    let blocked = b
        .post(
            &format!("/invoices/{id}/cancel"),
            json!({ "reason": "Duplicate" }),
        )
        .await;
    assert_eq!(blocked.status, StatusCode::CONFLICT);
    let removed = b
        .app
        .request(
            Method::DELETE,
            &b.path(&format!("/invoices/{id}/payments/{payment_id}")),
            &b.user.auth(),
            Some(json!({ "reason": "Entered twice" })),
        )
        .await;
    assert_eq!(removed.status, StatusCode::OK, "{}", removed.body);
    assert_eq!(removed.body["paid"], 0);
    assert!(
        removed.body["payments"][0]["voided_at"].is_string(),
        "the payment is kept as voided"
    );
    let cancelled = b
        .post(
            &format!("/invoices/{id}/cancel"),
            json!({ "reason": "Duplicate" }),
        )
        .await;
    assert_eq!(cancelled.body["display_status"], "cancelled");

    // Every entry is matched by its reversal: the invoice contributes nothing any more.
    assert_eq!(b.dashboard("accrual").await["totals"]["income"], 0);
    assert_eq!(b.dashboard("cash").await["totals"]["income"], 0);
    b.assert_ledger_balances().await;

    // Issued invoices can't be deleted; drafts can.
    let delete_issued = b
        .app
        .delete(&b.path(&format!("/invoices/{id}")), &b.user.auth(), None)
        .await;
    assert_eq!(delete_issued.status, StatusCode::CONFLICT);
    let draft = b.draft("2026-05-01", "2026-05-15").await;
    let deleted = b
        .app
        .delete(
            &b.path(&format!("/invoices/{}", draft["id"].as_str().expect("id"))),
            &b.user.auth(),
            None,
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn the_ledger_is_append_only_for_the_application() {
    let b = Books::new("append-only").await;
    b.issued("2026-02-01", "2026-02-15").await;
    let mut tx = b
        .app
        .state
        .db
        .begin_business(b.user.id, b.business)
        .await
        .expect("tx");
    let update = sqlx::query("UPDATE journal_entry_lines SET debit = debit + 1")
        .execute(&mut *tx)
        .await;
    assert!(update.is_err(), "the API role can't change posted entries");
    drop(tx);
    let mut tx = b
        .app
        .state
        .db
        .begin_business(b.user.id, b.business)
        .await
        .expect("tx");
    let delete = sqlx::query("DELETE FROM journal_entries")
        .execute(&mut *tx)
        .await;
    assert!(delete.is_err(), "the API role can't delete posted entries");
}

#[tokio::test]
async fn invoice_list_filters_search_and_paginate() {
    let b = Books::new("listing").await;
    b.issued("2026-01-05", "2026-01-20").await;
    let second = b.issued("2026-02-05", "2099-02-20").await;
    b.draft("2026-03-05", "2026-03-20").await;
    let all = b.get("/invoices?per_page=2").await;
    assert_eq!(all.body["total"], 3);
    assert_eq!(all.body["items"].as_array().map(Vec::len), Some(2));
    let overdue = b.get("/invoices?status=overdue").await;
    assert_eq!(overdue.body["total"], 1);
    let drafts_and_issued = b.get("/invoices?status=draft,issued").await;
    assert_eq!(drafts_and_issued.body["total"], 2);
    let search = b
        .get(&format!(
            "/invoices?search={}",
            second["number"].as_str().expect("number")
        ))
        .await;
    assert_eq!(search.body["total"], 1);
    let by_customer = b
        .get(&format!("/invoices?customer_id={}&sort=-total", b.customer))
        .await;
    assert_eq!(by_customer.body["total"], 3);
}

#[tokio::test]
async fn customers_show_balances_and_cannot_be_deleted_with_records() {
    let b = Books::new("customers").await;
    let invalid = b
        .post(
            "/customers",
            json!({ "name": " ", "email": "nope", "nzbn": "123", "payment_terms_days": 900 }),
        )
        .await;
    for field in ["name", "email", "nzbn", "payment_terms_days"] {
        assert!(
            invalid.field_error(field).is_some(),
            "{field}: {}",
            invalid.body
        );
    }
    b.issued("2026-01-05", "2026-01-20").await;
    let detail = b.get(&format!("/customers/{}", b.customer)).await;
    assert_eq!(detail.body["outstanding"], 115_000);
    assert_eq!(detail.body["overdue"], 115_000);
    assert_eq!(detail.body["stats"]["invoice_count"], 1);
    let listed = b.get("/customers?sort=outstanding&search=tūī").await;
    assert_eq!(listed.body["items"][0]["outstanding"], 115_000);

    let delete = b
        .app
        .delete(
            &b.path(&format!("/customers/{}", b.customer)),
            &b.user.auth(),
            None,
        )
        .await;
    assert_eq!(delete.status, StatusCode::CONFLICT);
    let deactivated = b
        .app
        .put(
            &b.path(&format!("/customers/{}", b.customer)),
            &b.user.auth(),
            json!({ "name": "Tūī Media", "is_active": false }),
        )
        .await;
    assert_eq!(deactivated.body["is_active"], false);
    assert_eq!(
        b.get("/customers").await.body["total"],
        0,
        "inactive customers are hidden by default"
    );
    assert_eq!(b.get("/customers?status=all").await.body["total"], 1);

    let spare = b.post("/customers", json!({ "name": "Spare" })).await;
    let spare_id = spare.body["id"].as_str().expect("id");
    assert_eq!(
        b.app
            .delete(
                &b.path(&format!("/customers/{spare_id}")),
                &b.user.auth(),
                None
            )
            .await
            .status,
        StatusCode::NO_CONTENT
    );
}

#[tokio::test]
async fn quotes_convert_into_invoices_once() {
    let b = Books::new("quotes").await;
    let quote = b
        .post(
            "/quotes",
            json!({ "customer_id": b.customer, "issue_date": "2099-01-01",
                    "lines": [{ "description": "Website", "quantity": "1", "unit_price": 500_000, "discount_percent": "10", "tax_treatment": "standard" }] }),
        )
        .await;
    assert_eq!(quote.status, StatusCode::CREATED, "{}", quote.body);
    assert_eq!(quote.body["number"], "QU-0001");
    assert_eq!(quote.body["total"], 517_500);
    let id = quote.body["id"].as_str().expect("id");
    let accepted = b
        .post(
            &format!("/quotes/{id}/status"),
            json!({ "status": "accepted" }),
        )
        .await;
    assert_eq!(accepted.body["display_status"], "accepted");
    let converted = b.post(&format!("/quotes/{id}/convert"), json!({})).await;
    assert_eq!(converted.status, StatusCode::CREATED, "{}", converted.body);
    let invoice = b
        .get(&format!(
            "/invoices/{}",
            converted.body["invoice_id"].as_str().expect("invoice")
        ))
        .await;
    assert_eq!(invoice.body["status"], "draft");
    assert_eq!(invoice.body["total"], 517_500);
    assert_eq!(invoice.body["quote_id"], id);
    assert_eq!(invoice.body["lines"][0]["discount_percent"], "10.00");
    assert_eq!(
        b.post(&format!("/quotes/{id}/convert"), json!({}))
            .await
            .status,
        StatusCode::CONFLICT
    );
    assert_eq!(
        b.get(&format!("/quotes/{id}")).await.body["display_status"],
        "converted"
    );
}

#[tokio::test]
async fn expenses_split_business_and_private_shares_with_gst() {
    let b = Books::new("expenses").await;
    let categories = b.get("/expense-categories").await;
    let internet = categories
        .body
        .as_array()
        .expect("categories")
        .iter()
        .find(|c| c["name"] == "Internet")
        .expect("Internet")["id"]
        .clone();
    let expense = b
        .post(
            "/expenses",
            json!({ "expense_date": "2026-06-01", "supplier": "Spark", "category_id": internet, "gross_amount": 11_500,
                    "gst_treatment": "standard", "payment_method": "business_account", "classification": "partial",
                    "business_percentage": "60" }),
        )
        .await;
    assert_eq!(expense.status, StatusCode::CREATED, "{}", expense.body);
    assert_eq!(
        expense.body["gst_amount"], 1_500,
        "GST calculated as 3/23 of the amount paid"
    );
    assert_eq!(
        expense.body["split"],
        json!({ "expense_amount": 6_000, "gst_claim": 900, "private_amount": 4_600 })
    );
    assert_eq!(expense.body["deductible_percentage"], "60.00");

    let personal = b
        .post(
            "/expenses",
            json!({ "expense_date": "2026-06-02", "supplier": "Countdown", "category_id": internet, "gross_amount": 5_000,
                    "gst_treatment": "standard", "payment_method": "personal_funds", "classification": "personal" }),
        )
        .await;
    assert_eq!(personal.body["split"]["expense_amount"], 0);

    let dashboard = b.dashboard("accrual").await;
    assert_eq!(
        dashboard["totals"]["expenses"], 6_000,
        "only the business share is an expense"
    );
    assert_eq!(dashboard["gst"]["claimable"], 900);
    assert_eq!(dashboard["expenses_by_category"][0]["name"], "Internet");

    // Deleting reverses the entry and keeps the record.
    let id = expense.body["id"].as_str().expect("id");
    let deleted = b
        .app
        .request(
            Method::DELETE,
            &b.path(&format!("/expenses/{id}")),
            &b.user.auth(),
            Some(json!({ "reason": "Wrong month" })),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    assert_eq!(b.dashboard("accrual").await["totals"]["expenses"], 0);
    let listed = b.get("/expenses").await;
    assert_eq!(listed.body["total"], 1);
    assert_eq!(
        b.get("/expenses?include_deleted=true").await.body["total"],
        2
    );
    b.assert_ledger_balances().await;

    let invalid = b
        .post("/expenses", json!({ "expense_date": "2026-06-01", "supplier": "", "category_id": Uuid::now_v7(), "gross_amount": 0,
                                    "gst_treatment": "standard", "payment_method": "cash", "classification": "partial" }))
        .await;
    for field in [
        "supplier",
        "category_id",
        "gross_amount",
        "business_percentage",
    ] {
        assert!(
            invalid.field_error(field).is_some(),
            "{field}: {}",
            invalid.body
        );
    }
}

#[tokio::test]
async fn categories_can_be_added_renamed_and_archived() {
    let b = Books::new("categories").await;
    let created = b
        .post("/expense-categories", json!({ "name": "Co-working" }))
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    assert_eq!(created.body["account_code"], "5160");
    assert_eq!(
        b.post("/expense-categories", json!({ "name": "co-working" }))
            .await
            .status,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let id = created.body["id"].as_str().expect("id");
    let renamed = b
        .app
        .put(
            &b.path(&format!("/expense-categories/{id}")),
            &b.user.auth(),
            json!({ "name": "Shared office" }),
        )
        .await;
    assert_eq!(renamed.body["name"], "Shared office");
    let archived = b
        .post(
            &format!("/expense-categories/{id}/archive"),
            json!({ "archived": true }),
        )
        .await;
    assert_eq!(archived.body["archived"], true);
}

fn multipart(bytes: &[u8], filename: &str) -> (String, Vec<u8>) {
    let boundary = "----bookkeeping";
    let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n\r\n").into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

#[tokio::test]
async fn receipts_attach_download_and_detach() {
    let b = Books::new("receipts").await;
    let category = b.get("/expense-categories").await.body[0]["id"].clone();
    let expense = b
        .post("/expenses", json!({ "expense_date": "2026-07-01", "supplier": "Noel Leeming", "category_id": category, "gross_amount": 29_900,
                                     "gst_treatment": "standard", "payment_method": "credit_card", "classification": "business" }))
        .await;
    let id = expense.body["id"].as_str().expect("id").to_owned();
    let upload = |bytes: &[u8], name: &str| {
        let (content_type, body) = multipart(bytes, name);
        Request::builder()
            .method(Method::POST)
            .uri(b.path(&format!("/expenses/{id}/attachments")))
            .header("x-client-kind", "desktop")
            .header("x-test-owner", b.user.id.to_string())
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(body))
            .expect("request")
    };
    let rejected = b
        .app
        .send(upload(b"MZ\x90\x00 not a receipt", "virus.pdf"))
        .await;
    assert_eq!(rejected.status, StatusCode::UNPROCESSABLE_ENTITY);
    let pdf = b"%PDF-1.7\n receipt";
    let attached = b.app.send(upload(pdf, "../../receipt.pdf")).await;
    assert_eq!(attached.status, StatusCode::CREATED, "{}", attached.body);
    let attachment = &attached.body["attachments"][0];
    assert_eq!(
        attachment["filename"], "receipt.pdf",
        "path components are stripped"
    );
    let attachment_id = attachment["id"].as_str().expect("id");
    let downloaded = b
        .get(&format!("/expenses/{id}/attachments/{attachment_id}"))
        .await;
    assert_eq!(downloaded.headers[header::CONTENT_TYPE], "application/pdf");
    assert_eq!(downloaded.bytes, pdf);
    assert_eq!(b.get("/expenses?has_receipt=true").await.body["total"], 1);
    let removed = b
        .app
        .delete(
            &b.path(&format!("/expenses/{id}/attachments/{attachment_id}")),
            &b.user.auth(),
            None,
        )
        .await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT);
    assert_eq!(b.get("/expenses?has_receipt=true").await.body["total"], 0);
}

#[tokio::test]
async fn income_without_an_invoice_counts_on_both_bases() {
    let b = Books::new("income").await;
    let income = b
        .post("/income", json!({ "income_date": "2026-08-01", "payer": "Market stall", "income_type": "sales", "gross_amount": 23_000,
                                  "gst_treatment": "standard", "payment_method": "cash" }))
        .await;
    assert_eq!(income.status, StatusCode::CREATED, "{}", income.body);
    assert_eq!(income.body["gst_amount"], 3_000);
    for basis in ["cash", "accrual"] {
        assert_eq!(
            b.dashboard(basis).await["totals"]["income"],
            20_000,
            "{basis}"
        );
    }
    b.assert_ledger_balances().await;
}

#[tokio::test]
async fn invoices_and_quotes_render_as_pdf() {
    let b = Books::new("pdf").await;
    let invoice = b.issued("2026-09-01", "2026-09-20").await;
    let pdf = b
        .get(&format!(
            "/invoices/{}/pdf",
            invoice["id"].as_str().expect("id")
        ))
        .await;
    assert_eq!(pdf.status, StatusCode::OK);
    assert_eq!(pdf.headers[header::CONTENT_TYPE], "application/pdf");
    assert!(pdf.bytes.starts_with(b"%PDF-"));
    assert!(pdf.bytes.len() > 5_000);
    let quote = b
        .post("/quotes", json!({ "customer_id": b.customer, "issue_date": "2026-09-01",
                                 "lines": [{ "description": "Logo", "quantity": "1", "unit_price": 90_000, "tax_treatment": "standard" }] }))
        .await;
    let pdf = b
        .get(&format!(
            "/quotes/{}/pdf",
            quote.body["id"].as_str().expect("id")
        ))
        .await;
    assert!(pdf.bytes.starts_with(b"%PDF-"));
}

#[tokio::test]
async fn sending_an_invoice_emails_the_pdf_and_is_logged() {
    let b = Books::new("sending").await;
    let invoice = b.issued("2026-10-01", "2026-10-20").await;
    let id = invoice["id"].as_str().expect("id");
    let sent = b
        .post(
            &format!("/invoices/{id}/send"),
            json!({ "message": "Thanks for your business!" }),
        )
        .await;
    assert_eq!(sent.status, StatusCode::OK, "{}", sent.body);
    assert!(sent.body["sent_at"].is_string());
    assert_eq!(sent.body["emails"][0]["status"], "sent");
    assert_eq!(sent.body["emails"][0]["to_email"], "accounts@tui.example");
    let email = b
        .app
        .email("accounts@tui.example", "Invoice INV-0001")
        .await;
    assert_eq!(email.attachments.len(), 1);
    assert!(email.attachments[0].bytes.starts_with(b"%PDF-"));
    assert!(email.text.contains("Thanks for your business!"));
    assert!(
        email.reply_to.is_none(),
        "no business email set, so no reply-to"
    );
}

#[tokio::test]
async fn bookkeeping_records_are_isolated_between_businesses() {
    let b = Books::new("isolated").await;
    let invoice = b.issued("2026-02-01", "2026-02-28").await;
    let invoice_id = invoice["id"].as_str().expect("id");
    let intruder = b.app.register("bk-intruder").await;
    for path in [
        format!("/invoices/{invoice_id}"),
        format!("/invoices/{invoice_id}/pdf"),
        format!("/customers/{}", b.customer),
        "/invoices".to_owned(),
        "/expenses".to_owned(),
        "/dashboard".to_owned(),
    ] {
        assert_eq!(
            b.app.get(&b.path(&path), &intruder.auth()).await.status,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    // The intruder's own business can't reference the victim's customer or invoice.
    let own = b.app.create_business(&intruder, "Intruder Ltd").await;
    let forged = b
        .app
        .post(
            &format!("/api/v1/businesses/{own}/invoices"),
            &intruder.auth(),
            json!({ "customer_id": b.customer, "issue_date": "2026-02-01",
                    "lines": [{ "description": "x", "quantity": "1", "unit_price": 100, "tax_treatment": "standard" }] }),
        )
        .await;
    assert!(
        forged.field_error("customer_id").is_some(),
        "{}",
        forged.body
    );
    let mut tx = b
        .app
        .state
        .db
        .begin_business(intruder.id, own)
        .await
        .expect("tx");
    let visible: i64 = sqlx::query_scalar("SELECT count(*) FROM journal_entry_lines")
        .fetch_one(&mut *tx)
        .await
        .expect("count");
    assert_eq!(
        visible, 0,
        "row-level security hides the other business's ledger"
    );
}

#[tokio::test]
async fn export_and_deletion_cover_bookkeeping_records() {
    let b = Books::new("export-books").await;
    b.issued("2026-02-01", "2026-02-28").await;
    let export = b.app.get("/api/v1/users/me/export", &b.user.auth()).await;
    let records = &export.body["businesses"][0]["records"];
    assert_eq!(records["invoices"].as_array().map(Vec::len), Some(1));
    assert_eq!(records["customers"].as_array().map(Vec::len), Some(1));
    assert!(
        records["journal_entry_lines"]
            .as_array()
            .is_some_and(|l| l.len() >= 3)
    );
    // Deleting the business removes its ledger, even though the API can't delete journal rows directly.
    let business = b.get("").await.body;
    let deleted = b
        .app
        .delete(
            &b.path(""),
            &b.user.auth(),
            Some(json!({ "confirmation_name": business["name"] })),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{}", deleted.body);
}

#[tokio::test]
async fn invoices_sort_by_every_column_both_ways() {
    let b = Books::new("sorting").await;
    let other = b
        .post("/customers", json!({ "name": "Aardvark Media" }))
        .await;
    assert_eq!(other.status, StatusCode::CREATED, "{}", other.body);
    // A: the books' customer, issued, part paid, the largest; B: Aardvark, issued and unpaid; C: a draft.
    let line = |hours: &str| json!([{ "description": "Work", "quantity": hours, "unit_price": 12_500, "tax_treatment": "standard" }]);
    let draft_a = b
        .post(
            "/invoices",
            json!({ "customer_id": b.customer, "issue_date": "2026-03-01", "due_date": "2026-03-31", "lines": line("20") }),
        )
        .await;
    let a_id = draft_a.body["id"].as_str().expect("id").to_owned();
    b.post(&format!("/invoices/{a_id}/issue"), json!({})).await;
    let paid = b
        .post(
            &format!("/invoices/{a_id}/payments"),
            json!({ "payment_date": "2026-03-10", "amount": 10_000, "method": "bank_transfer" }),
        )
        .await;
    assert_eq!(paid.status, StatusCode::CREATED, "{}", paid.body);
    let draft_b = b
        .post(
            "/invoices",
            json!({ "customer_id": other.body["id"], "issue_date": "2026-02-01", "due_date": "2099-02-01", "lines": line("2") }),
        )
        .await;
    let b_id = draft_b.body["id"].as_str().expect("id").to_owned();
    b.post(&format!("/invoices/{b_id}/issue"), json!({})).await;
    let c = b.draft("2026-04-01", "2026-04-30").await;
    let c_id = c["id"].as_str().expect("id").to_owned();

    let order = |sort: &'static str| {
        let b = &b;
        async move {
            let list = b.get(&format!("/invoices?sort={sort}")).await;
            assert_eq!(list.status, StatusCode::OK, "{}", list.body);
            list.body["items"]
                .as_array()
                .expect("items")
                .iter()
                .map(|i| i["id"].as_str().expect("id").to_owned())
                .collect::<Vec<_>>()
        }
    };
    let (a, b_, c) = (a_id, b_id, c_id);
    assert_eq!(
        order("issue_date").await,
        [b_.clone(), a.clone(), c.clone()]
    );
    assert_eq!(
        order("-issue_date").await,
        [c.clone(), a.clone(), b_.clone()]
    );
    assert_eq!(
        order("unknown").await,
        [c.clone(), a.clone(), b_.clone()],
        "the default is newest first"
    );
    assert_eq!(order("customer").await[0], b_, "Aardvark first");
    assert_eq!(order("-customer").await[2], b_, "Aardvark last");
    // Display statuses: draft < issued < overdue < partially_paid … (A is part paid and overdue).
    assert_eq!(order("status").await[0], c, "draft first");
    assert_eq!(order("-status").await[2], c, "draft last");
    assert_eq!(
        order("total").await[0],
        b_,
        "the two-hour invoice is smallest"
    );
    assert_eq!(order("-total").await[2], b_);
    // Balances: the draft owes nothing, B owes all of it, A owes the most even after a payment.
    assert_eq!(order("balance").await, [c.clone(), b_.clone(), a.clone()]);
    assert_eq!(order("-balance").await, [a, b_, c]);
}

#[tokio::test]
async fn the_accountant_pack_has_the_years_reports_journal_and_receipts() {
    let b = Books::new("accountant-pack").await;
    let category = b.get("/expense-categories").await.body[0]["id"].clone();
    let expense = b
        .post("/expenses", json!({ "expense_date": "2026-07-01", "supplier": "Stationery: shop/online", "category_id": category,
                                     "gross_amount": 29_900, "gst_treatment": "standard", "payment_method": "credit_card",
                                     "classification": "business" }))
        .await;
    let id = expense.body["id"].as_str().expect("id").to_owned();
    let (content_type, body) = multipart(b"%PDF-1.7\n receipt", "receipt.pdf");
    let attached = b
        .app
        .send(
            Request::builder()
                .method(Method::POST)
                .uri(b.path(&format!("/expenses/{id}/attachments")))
                .header("x-client-kind", "desktop")
                .header("x-test-owner", b.user.id.to_string())
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(body))
                .expect("request"),
        )
        .await;
    assert_eq!(attached.status, StatusCode::CREATED, "{}", attached.body);

    let not_a_year_end = b.get("/reports/accountant-pack?year_end=2026-12-31").await;
    assert_eq!(not_a_year_end.status, StatusCode::UNPROCESSABLE_ENTITY);

    let pack = b.get("/reports/accountant-pack?year_end=2027-03-31").await;
    assert_eq!(pack.status, StatusCode::OK);
    assert_eq!(pack.headers[header::CONTENT_TYPE], "application/zip");
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(pack.bytes)).expect("a zip");
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).expect("file").name().to_owned())
        .collect();
    for expected in [
        "Reports 2026-04-01 to 2027-03-31.pdf",
        "Reports 2026-04-01 to 2027-03-31.xlsx",
        "General journal.csv",
        "Mileage log.csv",
        "README.txt",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "{expected} in {names:?}"
        );
    }
    let receipt = names
        .iter()
        .find(|n| n.starts_with("Receipts/"))
        .expect("the receipt");
    assert!(
        receipt.starts_with("Receipts/2026-07-01 Stationery- shop-online")
            && receipt.ends_with("receipt.pdf"),
        "{receipt}"
    );
    let mut journal = String::new();
    std::io::Read::read_to_string(
        &mut zip.by_name("General journal.csv").expect("journal"),
        &mut journal,
    )
    .expect("text");
    assert!(journal.contains("Stationery: shop/online"), "{journal}");
}

#[tokio::test]
async fn invoices_and_expenses_export_in_xeros_import_format() {
    let b = Books::new("xero-export").await;
    let invoice = b.issued("2026-07-01", "2026-07-21").await;
    let category = b.get("/expense-categories").await.body[0]["id"].clone();
    b.post("/expenses", json!({ "expense_date": "2026-07-03", "supplier": "Phone company", "category_id": category,
                                 "gross_amount": 11_500, "gst_treatment": "standard", "payment_method": "credit_card",
                                 "classification": "partial", "business_percentage": "50" }))
        .await;

    let sales = b
        .get("/reports/xero-export?kind=sales&from=2026-07-01&to=2026-07-31")
        .await;
    assert_eq!(sales.status, StatusCode::OK, "{}", sales.body);
    let text = String::from_utf8_lossy(&sales.bytes).to_string();
    let mut lines = text.lines();
    assert!(
        lines.next().expect("header").contains("*ContactName"),
        "{text}"
    );
    let number = invoice["number"].as_str().expect("number");
    assert!(
        text.contains(number) && text.contains("15% GST on Income") && text.contains("01/07/2026"),
        "{text}"
    );
    // The lines add up to the invoice's GST-exclusive subtotal.
    let subtotal: f64 = text
        .lines()
        .skip(1)
        .map(|l| {
            l.split(',')
                .nth(8)
                .expect("amount")
                .trim_matches('"')
                .parse::<f64>()
                .expect("number")
        })
        .sum();
    assert_eq!(
        (subtotal * 100.0).round() as i64,
        invoice["subtotal"].as_i64().expect("subtotal")
    );

    let bills = b
        .get("/reports/xero-export?kind=bills&from=2026-07-01&to=2026-07-31")
        .await;
    let text = String::from_utf8_lossy(&bills.bytes).to_string();
    assert!(
        text.contains("Phone company") && text.contains("15% GST on Expenses"),
        "{text}"
    );
    assert!(
        text.contains(",50,") || text.contains(",50.00,") || text.contains("\"50\""),
        "the business half of $100: {text}"
    );
}

#[tokio::test]
async fn a_statement_is_emailed_to_the_customer_with_its_pdf_and_logged() {
    let b = Books::new("statement-email").await;
    b.issued("2026-07-01", "2026-07-21").await;
    let customer = b.customer;
    let sent = b
        .post(
            &format!("/customers/{customer}/statement-emails"),
            json!({ "from": "2026-07-01", "to": "2026-07-31" }),
        )
        .await;
    assert_eq!(sent.status, StatusCode::OK, "{}", sent.body);
    assert_eq!(sent.body[0]["status"], "sent");
    let email = b.app.email("accounts@tui.example", "Statement from").await;
    assert_eq!(email.attachments.len(), 1);
    assert!(email.attachments[0].bytes.starts_with(b"%PDF"));
    assert!(email.text.contains("Amount owing"), "{}", email.text);

    let history = b
        .get(&format!("/customers/{customer}/statement-emails"))
        .await;
    assert_eq!(history.body.as_array().map(Vec::len), Some(1));

    // A bad address is a field error, before anything is sent.
    let bad = b
        .post(
            &format!("/customers/{customer}/statement-emails"),
            json!({ "email": "nope" }),
        )
        .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(bad.field_error("email").is_some());
}

#[tokio::test]
async fn reminders_go_once_for_the_reminder_due_and_respect_the_customers_choice() {
    let b = Books::new("reminders").await;
    let today = chrono::Utc::now().date_naive();
    let day = |offset: i64| (today + chrono::Duration::days(offset)).to_string();
    // Due two days ago: the "1 day after" reminder is due.
    let invoice = b.issued(&day(-20), &day(-2)).await;
    let put = |rest: &str, body: Value| {
        let path = b.path(rest);
        let auth = b.user.auth();
        let app = &b.app;
        async move { app.put(&path, &auth, body).await }
    };

    // Off by default: nothing is sent.
    let off = b.post("/reminders/run", json!({})).await;
    assert_eq!(off.status, StatusCode::OK, "{}", off.body);
    assert_eq!(off.body["sent"].as_array().map(Vec::len), Some(0));

    let invalid = put(
        "/reminders/settings",
        json!({ "reminders_enabled": true, "reminder_days_before": 90, "reminder_days_after": [0] }),
    )
    .await;
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(invalid.field_error("reminder_days_before").is_some());
    assert!(invalid.field_error("reminder_days_after").is_some());

    let saved = put(
        "/reminders/settings",
        json!({ "reminders_enabled": true, "reminder_days_before": 3, "reminder_days_after": [7, 1, 14] }),
    )
    .await;
    assert_eq!(saved.status, StatusCode::OK, "{}", saved.body);
    assert_eq!(saved.body["reminder_days_after"], json!([1, 7, 14]));

    let run = b.post("/reminders/run", json!({})).await;
    assert_eq!(
        run.body["sent"].as_array().map(Vec::len),
        Some(1),
        "{}",
        run.body
    );
    assert_eq!(run.body["sent"][0]["day"], 1);
    let email = b
        .app
        .email("accounts@tui.example", "Overdue: invoice")
        .await;
    assert!(email.attachments[0].bytes.starts_with(b"%PDF"));
    assert!(email.text.contains("still owing"), "{}", email.text);

    // Each reminder goes once.
    let again = b.post("/reminders/run", json!({})).await;
    assert_eq!(again.body["sent"].as_array().map(Vec::len), Some(0));

    // It's in the invoice's emails.
    let id = invoice["id"].as_str().expect("id");
    let detail = b.get(&format!("/invoices/{id}")).await;
    assert!(
        detail.body["emails"]
            .as_array()
            .expect("emails")
            .iter()
            .any(|e| e["subject"]
                .as_str()
                .unwrap_or_default()
                .starts_with("Overdue")),
        "{}",
        detail.body["emails"]
    );

    // A customer who has opted out gets none.
    let customer = b.customer;
    let opted = put(
        &format!("/customers/{customer}/reminders"),
        json!({ "send_reminders": false }),
    )
    .await;
    assert_eq!(opted.status, StatusCode::NO_CONTENT);
    b.issued(&day(-10), &day(2)).await; // the "3 days before" reminder is due
    let none = b.post("/reminders/run", json!({})).await;
    assert_eq!(
        none.body["sent"].as_array().map(Vec::len),
        Some(0),
        "{}",
        none.body
    );
    assert_eq!(
        b.get(&format!("/customers/{customer}")).await.body["send_reminders"],
        false
    );
}

#[tokio::test]
async fn the_weekly_notices_email_goes_once_a_week_when_turned_on() {
    let b = Books::new("notices-email").await;
    let today = chrono::Utc::now().date_naive();
    let day = |offset: i64| (today + chrono::Duration::days(offset)).to_string();
    b.issued(&day(-60), &day(-40)).await; // overdue: a notice
    let auth = b.user.auth();
    let run = |now: bool| {
        let auth = auth.clone();
        let app = &b.app;
        async move {
            app.post(
                "/api/v1/users/me/notices-email/run",
                &auth,
                json!({ "now": now }),
            )
            .await
        }
    };

    let off = run(false).await;
    assert_eq!(off.status, StatusCode::OK, "{}", off.body);
    assert_eq!(off.body["sent"], false);

    let bad = b
        .app
        .put(
            "/api/v1/users/me/notices-email",
            &auth,
            json!({ "enabled": true, "to": "nope" }),
        )
        .await;
    assert!(bad.field_error("to").is_some(), "{}", bad.body);
    let on = b
        .app
        .put(
            "/api/v1/users/me/notices-email",
            &auth,
            json!({ "enabled": true, "to": "owner@notices.example" }),
        )
        .await;
    assert_eq!(on.status, StatusCode::OK, "{}", on.body);

    let first = run(false).await;
    assert_eq!(first.body["sent"], true, "{}", first.body);
    assert!(first.body["notices"].as_u64().unwrap_or(0) >= 1);
    let email = b.app.email("owner@notices.example", "this week").await;
    assert!(email.text.contains("dismissed"), "{}", email.text);

    let again = run(false).await;
    assert_eq!(again.body["sent"], false, "once a week");
    let now = run(true).await;
    assert_eq!(now.body["sent"], true, "Send now always sends");
    let settings = b.app.get("/api/v1/users/me/notices-email", &auth).await;
    assert!(settings.body["last_sent_at"].is_string());
}
