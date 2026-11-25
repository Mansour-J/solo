//! Recurring invoice schedules.

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgConnection;
use sqlx::types::Json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::documents::LineInput;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct RecurringRow {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub name: String,
    /// weekly, fortnightly, monthly, quarterly or yearly.
    pub frequency: String,
    #[serde(skip)]
    pub anchor_day: i16,
    /// The date the next invoice is created for.
    pub next_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub payment_terms_days: Option<i32>,
    pub prices_include_gst: bool,
    pub reference: String,
    pub notes: String,
    pub footer: String,
    #[schema(value_type = Vec<LineInput>)]
    pub lines: Json<Vec<LineInput>>,
    /// Issue each invoice as it's created (otherwise it's a draft to check).
    pub auto_issue: bool,
    pub invoices_created: i32,
    pub last_invoice_id: Option<Uuid>,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
}

macro_rules! columns {
    () => {
        "r.id, r.customer_id, c.name AS customer_name, r.name, r.frequency, r.anchor_day, r.next_date, r.end_date,
         r.payment_terms_days, r.prices_include_gst, r.reference, r.notes, r.footer, r.lines, r.auto_issue,
         r.invoices_created, r.last_invoice_id, r.archived_at IS NOT NULL AS archived, r.created_at
         FROM recurring_invoices r JOIN customers c ON c.id = r.customer_id"
    };
}

pub async fn list(conn: &mut PgConnection) -> Result<Vec<RecurringRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " ORDER BY r.archived_at IS NOT NULL, r.next_date, lower(r.name)"
    ))
    .fetch_all(conn)
    .await
}

pub async fn find(conn: &mut PgConnection, id: Uuid) -> Result<Option<RecurringRow>, sqlx::Error> {
    sqlx::query_as(concat!("SELECT ", columns!(), " WHERE r.id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

/// Active schedules with an invoice due on or before `date`.
pub async fn due(
    conn: &mut PgConnection,
    date: NaiveDate,
) -> Result<Vec<RecurringRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " WHERE r.archived_at IS NULL AND r.next_date <= $1 ORDER BY r.next_date"
    ))
    .bind(date)
    .fetch_all(conn)
    .await
}

pub struct RecurringFields {
    pub customer_id: Uuid,
    pub name: String,
    pub frequency: String,
    pub anchor_day: i16,
    pub next_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub payment_terms_days: Option<i32>,
    pub prices_include_gst: bool,
    pub reference: String,
    pub notes: String,
    pub footer: String,
    pub lines: Vec<LineInput>,
    pub auto_issue: bool,
}

pub async fn insert(
    conn: &mut PgConnection,
    business_id: Uuid,
    f: &RecurringFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO recurring_invoices (business_id, customer_id, name, frequency, anchor_day, next_date, end_date,
           payment_terms_days, prices_include_gst, reference, notes, footer, lines, auto_issue)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) RETURNING id",
    )
    .bind(business_id)
    .bind(f.customer_id)
    .bind(&f.name)
    .bind(&f.frequency)
    .bind(f.anchor_day)
    .bind(f.next_date)
    .bind(f.end_date)
    .bind(f.payment_terms_days)
    .bind(f.prices_include_gst)
    .bind(&f.reference)
    .bind(&f.notes)
    .bind(&f.footer)
    .bind(Json(&f.lines))
    .bind(f.auto_issue)
    .fetch_one(conn)
    .await
}

/// Changes the schedule (not its lines, which come from the invoice it was made from).
pub struct ScheduleChange<'a> {
    pub name: &'a str,
    pub frequency: &'a str,
    pub anchor_day: i16,
    pub next_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub auto_issue: bool,
}

pub async fn update(
    conn: &mut PgConnection,
    id: Uuid,
    change: &ScheduleChange<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE recurring_invoices SET name = $2, frequency = $3, anchor_day = $4, next_date = $5, end_date = $6,
           auto_issue = $7 WHERE id = $1",
    )
    .bind(id)
    .bind(change.name)
    .bind(change.frequency)
    .bind(change.anchor_day)
    .bind(change.next_date)
    .bind(change.end_date)
    .bind(change.auto_issue)
    .execute(conn)
    .await
    .map(|_| ())
}

/// After creating an invoice: the next date, and archived when the schedule has ended.
pub async fn advance(
    conn: &mut PgConnection,
    id: Uuid,
    next_date: NaiveDate,
    invoice_id: Uuid,
    finished: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE recurring_invoices SET next_date = $2, last_invoice_id = $3, invoices_created = invoices_created + 1,
           archived_at = CASE WHEN $4 THEN now() ELSE archived_at END
         WHERE id = $1",
    )
    .bind(id)
    .bind(next_date)
    .bind(invoice_id)
    .bind(finished)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn set_archived(
    conn: &mut PgConnection,
    id: Uuid,
    archived: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE recurring_invoices SET archived_at = CASE WHEN $2 THEN coalesce(archived_at, now()) END WHERE id = $1",
    )
    .bind(id)
    .bind(archived)
    .execute(conn)
    .await
    .map(|_| ())
}
