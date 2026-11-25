//! Quotes.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::domain::documents::Totals;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct QuoteRow {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub number: String,
    pub status: String,
    pub issue_date: NaiveDate,
    pub expiry_date: NaiveDate,
    pub reference: String,
    pub currency: String,
    pub prices_include_gst: bool,
    pub notes: String,
    pub footer: String,
    pub subtotal: i64,
    pub discount_total: i64,
    pub gst_total: i64,
    pub total: i64,
    pub sent_at: Option<DateTime<Utc>>,
    pub decided_at: Option<DateTime<Utc>>,
    pub converted_invoice_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

macro_rules! columns {
    () => {
        "q.id, q.customer_id, c.name AS customer_name, q.number, q.status, q.issue_date, q.expiry_date, q.reference, q.currency,
         q.prices_include_gst, q.notes, q.footer, q.subtotal, q.discount_total, q.gst_total, q.total, q.sent_at, q.decided_at,
         q.converted_invoice_id, q.created_at, q.updated_at"
    };
}

pub struct QuoteFields {
    pub customer_id: Uuid,
    pub issue_date: NaiveDate,
    pub expiry_date: NaiveDate,
    pub reference: String,
    pub currency: String,
    pub prices_include_gst: bool,
    pub notes: String,
    pub footer: String,
}

pub async fn insert(
    conn: &mut PgConnection,
    business_id: Uuid,
    user_id: Uuid,
    number: &str,
    f: &QuoteFields,
    t: Totals,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO quotes (business_id, customer_id, number, issue_date, expiry_date, reference, currency, prices_include_gst, notes,
            footer, subtotal, discount_total, gst_total, total, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15) RETURNING id",
    )
    .bind(business_id)
    .bind(f.customer_id)
    .bind(number)
    .bind(f.issue_date)
    .bind(f.expiry_date)
    .bind(&f.reference)
    .bind(&f.currency)
    .bind(f.prices_include_gst)
    .bind(&f.notes)
    .bind(&f.footer)
    .bind(t.subtotal)
    .bind(t.discount_total)
    .bind(t.gst_total)
    .bind(t.total)
    .bind(user_id)
    .fetch_one(conn)
    .await
}

pub async fn update(
    conn: &mut PgConnection,
    id: Uuid,
    f: &QuoteFields,
    t: Totals,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE quotes SET customer_id = $2, issue_date = $3, expiry_date = $4, reference = $5, prices_include_gst = $6, notes = $7,
            footer = $8, subtotal = $9, discount_total = $10, gst_total = $11, total = $12
         WHERE id = $1",
    )
    .bind(id)
    .bind(f.customer_id)
    .bind(f.issue_date)
    .bind(f.expiry_date)
    .bind(&f.reference)
    .bind(f.prices_include_gst)
    .bind(&f.notes)
    .bind(&f.footer)
    .bind(t.subtotal)
    .bind(t.discount_total)
    .bind(t.gst_total)
    .bind(t.total)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn find(conn: &mut PgConnection, id: Uuid) -> Result<Option<QuoteRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM quotes q JOIN customers c ON c.id = q.customer_id WHERE q.id = $1 FOR UPDATE OF q"
    ))
    .bind(id)
    .fetch_optional(conn)
    .await
}

pub async fn set_status(
    conn: &mut PgConnection,
    id: Uuid,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE quotes SET status = $2,
            sent_at = CASE WHEN $2 = 'sent' THEN coalesce(sent_at, now()) ELSE sent_at END,
            decided_at = CASE WHEN $2 IN ('accepted', 'rejected') THEN now() ELSE decided_at END
         WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn set_converted(
    conn: &mut PgConnection,
    id: Uuid,
    invoice_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE quotes SET converted_invoice_id = $2, status = 'accepted', decided_at = coalesce(decided_at, now()) WHERE id = $1")
        .bind(id)
        .bind(invoice_id)
        .execute(conn)
        .await
        .map(|_| ())
}

pub async fn delete(conn: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM quotes WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await
        .map(|_| ())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct QuoteListRow {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub number: String,
    pub status: String,
    pub display_status: String,
    pub issue_date: NaiveDate,
    pub expiry_date: NaiveDate,
    pub reference: String,
    pub currency: String,
    pub total: i64,
    pub converted_invoice_id: Option<Uuid>,
    pub total_count: i64,
}

pub struct ListFilter {
    pub statuses: Option<Vec<String>>,
    pub customer_id: Option<Uuid>,
    pub search: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

pub async fn list(
    conn: &mut PgConnection,
    today: NaiveDate,
    f: &ListFilter,
) -> Result<(Vec<QuoteListRow>, i64), sqlx::Error> {
    let search = f.search.as_ref().map(|s| {
        format!(
            "%{}%",
            s.replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    });
    let rows: Vec<QuoteListRow> = sqlx::query_as(
        "SELECT x.*, count(*) OVER () AS total_count FROM (
           SELECT q.id, q.customer_id, c.name AS customer_name, q.number, q.status,
                  CASE WHEN q.converted_invoice_id IS NOT NULL THEN 'converted' WHEN q.status = 'accepted' THEN 'accepted'
                       WHEN q.status = 'rejected' THEN 'rejected' WHEN q.expiry_date < $1 THEN 'expired'
                       WHEN q.status = 'sent' THEN 'sent' ELSE 'draft' END AS display_status,
                  q.issue_date, q.expiry_date, q.reference, q.currency, q.total, q.converted_invoice_id, q.created_at
           FROM quotes q JOIN customers c ON c.id = q.customer_id
           WHERE ($3::uuid IS NULL OR q.customer_id = $3)
             AND ($4::text IS NULL OR q.number ILIKE $4 OR q.reference ILIKE $4 OR c.name ILIKE $4)) x
         WHERE ($2::text[] IS NULL OR x.display_status = ANY($2))
         ORDER BY x.issue_date DESC, x.created_at DESC
         LIMIT $5 OFFSET $6",
    )
    .bind(today)
    .bind(&f.statuses)
    .bind(f.customer_id)
    .bind(&search)
    .bind(f.limit)
    .bind(f.offset)
    .fetch_all(conn)
    .await?;
    let total = rows.first().map(|r| r.total_count).unwrap_or(0);
    Ok((rows, total))
}
