//! Customers and their balances (from the `invoice_balances` view).

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CustomerRow {
    pub id: Uuid,
    pub name: String,
    pub business_name: String,
    pub contact_person: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub billing_address: serde_json::Value,
    pub business_address: serde_json::Value,
    pub nzbn: Option<String>,
    pub tax_number: Option<String>,
    pub notes: String,
    pub payment_terms_days: Option<i32>,
    pub default_currency: Option<String>,
    pub is_active: bool,
    pub send_reminders: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub outstanding: i64,
    pub overdue: i64,
}

macro_rules! columns {
    () => {
        "c.id, c.name, c.business_name, c.contact_person, c.email, c.phone, c.billing_address, c.business_address, c.nzbn,
         c.tax_number, c.notes, c.payment_terms_days, c.default_currency, c.is_active, c.send_reminders, c.created_at, c.updated_at,
         coalesce((SELECT sum(b.total - b.paid) FROM invoice_balances b WHERE b.customer_id = c.id AND b.status = 'issued'), 0)::bigint AS outstanding,
         coalesce((SELECT sum(b.total - b.paid) FROM invoice_balances b WHERE b.customer_id = c.id AND b.status = 'issued' AND b.due_date < $1), 0)::bigint AS overdue"
    };
}

#[derive(Debug, Clone)]
pub struct CustomerFields {
    pub name: String,
    pub business_name: String,
    pub contact_person: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub billing_address: serde_json::Value,
    pub business_address: serde_json::Value,
    pub nzbn: Option<String>,
    pub tax_number: Option<String>,
    pub notes: String,
    pub payment_terms_days: Option<i32>,
    pub default_currency: Option<String>,
    pub is_active: bool,
}

pub struct ListFilter {
    pub search: Option<String>,
    /// `Some(true)` active only, `Some(false)` inactive only, `None` all.
    pub active: Option<bool>,
    /// `name`, `outstanding` or `created`.
    pub sort: String,
    pub limit: i64,
    pub offset: i64,
}

pub async fn list(
    conn: &mut PgConnection,
    today: NaiveDate,
    f: &ListFilter,
) -> Result<(Vec<CustomerRow>, i64), sqlx::Error> {
    let search = f.search.as_ref().map(|s| {
        format!(
            "%{}%",
            s.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    });
    let items: Vec<CustomerRow> = sqlx::query_as(concat!(
        "SELECT * FROM (SELECT ",
        columns!(),
        " FROM customers c
           WHERE ($2::text IS NULL OR c.name ILIKE $2 OR c.business_name ILIKE $2 OR c.contact_person ILIKE $2 OR c.email::text ILIKE $2)
             AND ($3::boolean IS NULL OR c.is_active = $3)) c
         ORDER BY
           CASE WHEN $4 = 'outstanding' THEN c.outstanding END DESC NULLS LAST,
           CASE WHEN $4 = 'created' THEN c.created_at END DESC NULLS LAST,
           lower(c.name), c.id
         LIMIT $5 OFFSET $6"
    ))
    .bind(today)
    .bind(&search)
    .bind(f.active)
    .bind(&f.sort)
    .bind(f.limit)
    .bind(f.offset)
    .fetch_all(&mut *conn)
    .await?;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM customers c
         WHERE ($1::text IS NULL OR c.name ILIKE $1 OR c.business_name ILIKE $1 OR c.contact_person ILIKE $1 OR c.email::text ILIKE $1)
           AND ($2::boolean IS NULL OR c.is_active = $2)",
    )
    .bind(&search)
    .bind(f.active)
    .fetch_one(conn)
    .await?;
    Ok((items, total))
}

pub async fn find(
    conn: &mut PgConnection,
    today: NaiveDate,
    id: Uuid,
) -> Result<Option<CustomerRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM customers c WHERE c.id = $2"
    ))
    .bind(today)
    .bind(id)
    .fetch_optional(conn)
    .await
}

pub async fn insert(
    conn: &mut PgConnection,
    business_id: Uuid,
    f: &CustomerFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO customers (business_id, name, business_name, contact_person, email, phone, billing_address, business_address,
            nzbn, tax_number, notes, payment_terms_days, default_currency, is_active)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) RETURNING id",
    )
    .bind(business_id)
    .bind(&f.name)
    .bind(&f.business_name)
    .bind(&f.contact_person)
    .bind(&f.email)
    .bind(&f.phone)
    .bind(&f.billing_address)
    .bind(&f.business_address)
    .bind(&f.nzbn)
    .bind(&f.tax_number)
    .bind(&f.notes)
    .bind(f.payment_terms_days)
    .bind(&f.default_currency)
    .bind(f.is_active)
    .fetch_one(conn)
    .await
}

pub async fn update(
    conn: &mut PgConnection,
    id: Uuid,
    f: &CustomerFields,
) -> Result<bool, sqlx::Error> {
    sqlx::query(
        "UPDATE customers SET name = $2, business_name = $3, contact_person = $4, email = $5, phone = $6, billing_address = $7,
            business_address = $8, nzbn = $9, tax_number = $10, notes = $11, payment_terms_days = $12, default_currency = $13,
            is_active = $14
         WHERE id = $1",
    )
    .bind(id)
    .bind(&f.name)
    .bind(&f.business_name)
    .bind(&f.contact_person)
    .bind(&f.email)
    .bind(&f.phone)
    .bind(&f.billing_address)
    .bind(&f.business_address)
    .bind(&f.nzbn)
    .bind(&f.tax_number)
    .bind(&f.notes)
    .bind(f.payment_terms_days)
    .bind(&f.default_currency)
    .bind(f.is_active)
    .execute(conn)
    .await
    .map(|r| r.rows_affected() > 0)
}

/// Whether any document or record refers to the customer (then it can only be deactivated).
pub async fn has_records(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM invoices WHERE customer_id = $1)
             OR EXISTS (SELECT 1 FROM quotes WHERE customer_id = $1)
             OR EXISTS (SELECT 1 FROM income_records WHERE customer_id = $1)
             OR EXISTS (SELECT 1 FROM expenses WHERE rechargeable_customer_id = $1)
             OR EXISTS (SELECT 1 FROM journal_entry_lines WHERE customer_id = $1)",
    )
    .bind(id)
    .fetch_one(conn)
    .await
}

pub async fn delete(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query("DELETE FROM customers WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await
        .map(|r| r.rows_affected() > 0)
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema, sqlx::FromRow)]
pub struct CustomerStats {
    pub invoice_count: i64,
    pub total_invoiced: i64,
    pub total_paid: i64,
    pub last_invoice_date: Option<NaiveDate>,
    /// Average days from issue to the final payment, over fully paid invoices.
    pub average_days_to_pay: Option<f64>,
}

pub async fn stats(conn: &mut PgConnection, id: Uuid) -> Result<CustomerStats, sqlx::Error> {
    sqlx::query_as(
        "SELECT count(*) AS invoice_count,
                coalesce(sum(b.total), 0)::bigint AS total_invoiced,
                coalesce(sum(b.paid), 0)::bigint AS total_paid,
                max(b.issue_date) AS last_invoice_date,
                avg((SELECT max(p.payment_date) FROM invoice_payments p WHERE p.invoice_id = b.invoice_id AND p.voided_at IS NULL) - b.issue_date)
                  FILTER (WHERE b.paid >= b.total AND b.total > 0)::float8 AS average_days_to_pay
         FROM invoice_balances b WHERE b.customer_id = $1 AND b.status = 'issued'",
    )
    .bind(id)
    .fetch_one(conn)
    .await
}

/// Turns invoice reminders on or off for one customer.
pub async fn set_send_reminders(
    conn: &mut PgConnection,
    id: Uuid,
    on: bool,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query("UPDATE customers SET send_reminders = $2 WHERE id = $1")
        .bind(id)
        .bind(on)
        .execute(conn)
        .await?;
    Ok(done.rows_affected() == 1)
}
