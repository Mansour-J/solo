//! Invoices and their payments. Paid amounts come from the `invoice_balances` view.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::domain::documents::Totals;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InvoiceRow {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub number: Option<String>,
    pub status: String,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub reference: String,
    pub currency: String,
    pub prices_include_gst: bool,
    pub notes: String,
    pub footer: String,
    pub subtotal: i64,
    pub discount_total: i64,
    pub gst_total: i64,
    pub total: i64,
    pub paid: i64,
    /// Of `paid`, the amount written off as a bad debt.
    pub written_off: i64,
    pub quote_id: Option<Uuid>,
    pub issued_at: Option<DateTime<Utc>>,
    pub sent_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub cancellation_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

macro_rules! columns {
    () => {
        "i.id, i.customer_id, c.name AS customer_name, i.number, i.status, i.issue_date, i.due_date, i.reference, i.currency,
         i.prices_include_gst, i.notes, i.footer, i.subtotal, i.discount_total, i.gst_total, i.total, b.paid, b.written_off, i.quote_id,
         i.issued_at, i.sent_at, i.cancelled_at, i.cancellation_reason, i.created_at, i.updated_at"
    };
}

macro_rules! from {
    () => {
        " FROM invoices i JOIN customers c ON c.id = i.customer_id JOIN invoice_balances b ON b.invoice_id = i.id "
    };
}

/// The status users see, in SQL. Must match `domain::documents::invoice_display_status` (tested).
macro_rules! display_status {
    () => {
        "CASE WHEN i.status = 'draft' THEN 'draft' WHEN i.status = 'cancelled' THEN 'cancelled'
              WHEN b.paid >= i.total AND b.written_off > 0 THEN 'written_off'
              WHEN b.paid >= i.total THEN 'paid' WHEN i.due_date < $1 THEN 'overdue'
              WHEN b.paid > 0 THEN 'partially_paid' WHEN i.sent_at IS NOT NULL THEN 'sent' ELSE 'issued' END"
    };
}

pub struct InvoiceFields {
    pub customer_id: Uuid,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub reference: String,
    pub currency: String,
    pub prices_include_gst: bool,
    pub notes: String,
    pub footer: String,
    pub quote_id: Option<Uuid>,
}

pub async fn insert(
    conn: &mut PgConnection,
    business_id: Uuid,
    user_id: Uuid,
    f: &InvoiceFields,
    totals: Totals,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO invoices (business_id, customer_id, issue_date, due_date, reference, currency, prices_include_gst, notes, footer,
            subtotal, discount_total, gst_total, total, quote_id, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15) RETURNING id",
    )
    .bind(business_id)
    .bind(f.customer_id)
    .bind(f.issue_date)
    .bind(f.due_date)
    .bind(&f.reference)
    .bind(&f.currency)
    .bind(f.prices_include_gst)
    .bind(&f.notes)
    .bind(&f.footer)
    .bind(totals.subtotal)
    .bind(totals.discount_total)
    .bind(totals.gst_total)
    .bind(totals.total)
    .bind(f.quote_id)
    .bind(user_id)
    .fetch_one(conn)
    .await
}

pub async fn update(
    conn: &mut PgConnection,
    id: Uuid,
    f: &InvoiceFields,
    totals: Totals,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE invoices SET customer_id = $2, issue_date = $3, due_date = $4, reference = $5, prices_include_gst = $6, notes = $7,
            footer = $8, subtotal = $9, discount_total = $10, gst_total = $11, total = $12
         WHERE id = $1",
    )
    .bind(id)
    .bind(f.customer_id)
    .bind(f.issue_date)
    .bind(f.due_date)
    .bind(&f.reference)
    .bind(f.prices_include_gst)
    .bind(&f.notes)
    .bind(&f.footer)
    .bind(totals.subtotal)
    .bind(totals.discount_total)
    .bind(totals.gst_total)
    .bind(totals.total)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn find(conn: &mut PgConnection, id: Uuid) -> Result<Option<InvoiceRow>, sqlx::Error> {
    sqlx::query_as(concat!("SELECT ", columns!(), from!(), "WHERE i.id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

/// Locks the invoice row for the rest of the transaction (serialises payments and edits).
pub async fn lock(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM invoices WHERE id = $1 FOR UPDATE)")
        .bind(id)
        .fetch_one(conn)
        .await
}

pub async fn issue(conn: &mut PgConnection, id: Uuid, number: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE invoices SET status = 'issued', number = $2, issued_at = now() WHERE id = $1 AND status = 'draft'")
        .bind(id)
        .bind(number)
        .execute(conn)
        .await
        .map(|_| ())
}

pub async fn cancel(conn: &mut PgConnection, id: Uuid, reason: &str) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE invoices SET status = 'cancelled', cancelled_at = now(), cancellation_reason = $2 WHERE id = $1")
        .bind(id)
        .bind(reason)
        .execute(conn)
        .await
        .map(|_| ())
}

pub async fn mark_sent(conn: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE invoices SET sent_at = coalesce(sent_at, now()) WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await
        .map(|_| ())
}

pub async fn delete(conn: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM invoices WHERE id = $1 AND status = 'draft'")
        .bind(id)
        .execute(conn)
        .await
        .map(|_| ())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct InvoiceListRow {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub number: Option<String>,
    pub status: String,
    pub display_status: String,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub reference: String,
    pub currency: String,
    pub total: i64,
    pub paid: i64,
    pub sent_at: Option<DateTime<Utc>>,
    pub total_count: i64,
}

pub struct ListFilter {
    pub statuses: Option<Vec<String>>,
    pub customer_id: Option<Uuid>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub search: Option<String>,
    /// `issue_date`, `due_date`, `number`, `customer`, `status`, `total`, `balance`; prefix `-` for
    /// descending. Anything else: newest first.
    pub sort: String,
    pub limit: i64,
    pub offset: i64,
}

pub async fn list(
    conn: &mut PgConnection,
    today: NaiveDate,
    f: &ListFilter,
) -> Result<(Vec<InvoiceListRow>, i64), sqlx::Error> {
    let search = f.search.as_ref().map(|s| {
        format!(
            "%{}%",
            s.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    });
    let rows: Vec<InvoiceListRow> = sqlx::query_as(concat!(
        "SELECT x.*, count(*) OVER () AS total_count FROM (SELECT i.id, i.customer_id, c.name AS customer_name, i.number, i.status, ",
        display_status!(),
        " AS display_status, i.issue_date, i.due_date, i.reference, i.currency, i.total, b.paid, i.sent_at, i.created_at,
           CASE WHEN i.status = 'issued' THEN i.total - b.paid ELSE 0 END AS balance ",
        from!(),
        "WHERE ($3::uuid IS NULL OR i.customer_id = $3)
           AND ($4::date IS NULL OR i.issue_date >= $4)
           AND ($5::date IS NULL OR i.issue_date <= $5)
           AND ($6::text IS NULL OR i.number ILIKE $6 OR i.reference ILIKE $6 OR c.name ILIKE $6)) x
         WHERE ($2::text[] IS NULL OR x.display_status = ANY($2))
         ORDER BY
           CASE WHEN $7 = 'due_date' THEN x.due_date END ASC,
           CASE WHEN $7 = '-due_date' THEN x.due_date END DESC,
           CASE WHEN $7 = 'number' THEN x.number END ASC,
           CASE WHEN $7 = '-number' THEN x.number END DESC,
           CASE WHEN $7 = 'total' THEN x.total END ASC,
           CASE WHEN $7 = '-total' THEN x.total END DESC,
           CASE WHEN $7 = 'customer' THEN lower(x.customer_name) END ASC,
           CASE WHEN $7 = '-customer' THEN lower(x.customer_name) END DESC,
           CASE WHEN $7 = 'status' THEN x.display_status END ASC,
           CASE WHEN $7 = '-status' THEN x.display_status END DESC,
           CASE WHEN $7 = 'balance' THEN x.balance END ASC,
           CASE WHEN $7 = '-balance' THEN x.balance END DESC,
           CASE WHEN $7 = 'issue_date' THEN x.issue_date END ASC,
           x.issue_date DESC, x.created_at DESC
         LIMIT $8 OFFSET $9"
    ))
    .bind(today)
    .bind(&f.statuses)
    .bind(f.customer_id)
    .bind(f.from)
    .bind(f.to)
    .bind(&search)
    .bind(&f.sort)
    .bind(f.limit)
    .bind(f.offset)
    .fetch_all(&mut *conn)
    .await?;
    let total = rows.first().map(|r| r.total_count).unwrap_or(0);
    let total = if rows.is_empty() && f.offset > 0 {
        count(conn, today, f, &search).await?
    } else {
        total
    };
    Ok((rows, total))
}

async fn count(
    conn: &mut PgConnection,
    today: NaiveDate,
    f: &ListFilter,
    search: &Option<String>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(concat!(
        "SELECT count(*) FROM (SELECT ",
        display_status!(),
        " AS display_status ",
        from!(),
        "WHERE ($3::uuid IS NULL OR i.customer_id = $3) AND ($4::date IS NULL OR i.issue_date >= $4)
           AND ($5::date IS NULL OR i.issue_date <= $5)
           AND ($6::text IS NULL OR i.number ILIKE $6 OR i.reference ILIKE $6 OR c.name ILIKE $6)) x
         WHERE ($2::text[] IS NULL OR x.display_status = ANY($2))"
    ))
    .bind(today)
    .bind(&f.statuses)
    .bind(f.customer_id)
    .bind(f.from)
    .bind(f.to)
    .bind(search)
    .fetch_one(conn)
    .await
}

// ─── Payments ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema, sqlx::FromRow)]
pub struct PaymentRow {
    pub id: Uuid,
    pub invoice_id: Uuid,
    pub payment_date: NaiveDate,
    /// Minor units settled on the invoice.
    pub amount: i64,
    /// Part of `amount` the customer withheld and paid to IRD (schedular payments).
    pub withholding_tax: i64,
    pub method: String,
    pub reference: String,
    pub notes: String,
    pub voided_at: Option<DateTime<Utc>>,
    pub void_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

macro_rules! payment_columns {
    () => {
        "id, invoice_id, payment_date, amount, withholding_tax, method, reference, notes, voided_at, void_reason, created_at, updated_at"
    };
}

pub async fn payments(
    conn: &mut PgConnection,
    invoice_id: Uuid,
) -> Result<Vec<PaymentRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        payment_columns!(),
        " FROM invoice_payments WHERE invoice_id = $1 ORDER BY payment_date, created_at"
    ))
    .bind(invoice_id)
    .fetch_all(conn)
    .await
}

pub struct PaymentFields {
    pub payment_date: NaiveDate,
    pub amount: i64,
    pub withholding_tax: i64,
    pub method: String,
    pub reference: String,
    pub notes: String,
}

pub async fn insert_payment(
    conn: &mut PgConnection,
    business_id: Uuid,
    invoice_id: Uuid,
    user_id: Uuid,
    f: &PaymentFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO invoice_payments (business_id, invoice_id, payment_date, amount, withholding_tax, method, reference, notes, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
    )
    .bind(business_id)
    .bind(invoice_id)
    .bind(f.payment_date)
    .bind(f.amount)
    .bind(f.withholding_tax)
    .bind(&f.method)
    .bind(&f.reference)
    .bind(&f.notes)
    .bind(user_id)
    .fetch_one(conn)
    .await
}

pub async fn update_payment(
    conn: &mut PgConnection,
    id: Uuid,
    f: &PaymentFields,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE invoice_payments SET payment_date = $2, amount = $3, withholding_tax = $4, method = $5, reference = $6, notes = $7 WHERE id = $1 AND voided_at IS NULL")
        .bind(id)
        .bind(f.payment_date)
        .bind(f.amount)
        .bind(f.withholding_tax)
        .bind(&f.method)
        .bind(&f.reference)
        .bind(&f.notes)
        .execute(conn)
        .await
        .map(|_| ())
}

pub async fn void_payment(
    conn: &mut PgConnection,
    id: Uuid,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE invoice_payments SET voided_at = now(), void_reason = $2 WHERE id = $1 AND voided_at IS NULL")
        .bind(id)
        .bind(reason)
        .execute(conn)
        .await
        .map(|_| ())
}

// ─── Email history ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema, sqlx::FromRow)]
pub struct EmailLogRow {
    pub id: Uuid,
    pub to_email: String,
    pub subject: String,
    pub status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

pub async fn email_history(
    conn: &mut PgConnection,
    document_type: &str,
    document_id: Uuid,
) -> Result<Vec<EmailLogRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, to_email::text AS to_email, subject, status, error, created_at FROM email_logs
         WHERE document_type = $1 AND document_id = $2 ORDER BY created_at DESC",
    )
    .bind(document_type)
    .bind(document_id)
    .fetch_all(conn)
    .await
}

pub struct NewEmailLog<'a> {
    pub business_id: Uuid,
    pub document_type: &'a str,
    pub document_id: Uuid,
    pub to_email: &'a str,
    pub subject: &'a str,
    pub status: &'a str,
    pub provider_reference: Option<&'a str>,
    pub error: Option<&'a str>,
    pub sent_by: Uuid,
    /// For a reminder: its day relative to the due date.
    pub reminder_day: Option<i32>,
}

pub async fn log_email(conn: &mut PgConnection, log: NewEmailLog<'_>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO email_logs (business_id, document_type, document_id, to_email, subject, status, provider_reference, error, sent_by, reminder_day)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(log.business_id)
    .bind(log.document_type)
    .bind(log.document_id)
    .bind(log.to_email)
    .bind(log.subject)
    .bind(log.status)
    .bind(log.provider_reference.map(|r| r.chars().take(500).collect::<String>()))
    .bind(log.error.map(|e| e.chars().take(1000).collect::<String>()))
    .bind(log.sent_by)
    .bind(log.reminder_day)
    .execute(conn)
    .await
    .map(|_| ())
}
