//! Allocation destinations, rules, allocations and their append-only entries. Row-level security
//! limits every query to the current business.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

/// Seeds the default destinations (no amounts or rules) for a new business.
pub async fn seed_business(conn: &mut PgConnection, business_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT seed_allocation_destinations($1)")
        .bind(business_id)
        .execute(conn)
        .await
        .map(|_| ())
}

// ─── Destinations ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct DestinationRow {
    pub id: Uuid,
    pub name: String,
    /// `income_tax`, `gst`, `acc`, `savings`, `emergency_fund`, `kiwisaver`, `investment`,
    /// `mortgage`, `personal_spending` or `custom`.
    pub kind: String,
    pub position: i32,
    /// Minor units.
    pub target_amount: Option<i64>,
    /// Minor units.
    pub minimum_balance: Option<i64>,
    pub notes: String,
    pub archived: bool,
    /// Sum of confirmed entries (minor units).
    pub balance: i64,
    /// Rules (not archived) that allocate to it.
    pub rule_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

macro_rules! destination_columns {
    () => {
        "d.id, d.name, d.kind, d.position, d.target_amount, d.minimum_balance, d.notes,
         d.archived_at IS NOT NULL AS archived,
         (coalesce((SELECT sum(e.amount) FROM allocation_entries e WHERE e.destination_id = d.id), 0)
          - coalesce((SELECT sum(s.amount) FROM allocation_spending s WHERE s.destination_id = d.id AND s.voided_at IS NULL), 0))::bigint AS balance,
         (SELECT count(*) FROM allocation_rules r WHERE r.destination_id = d.id AND r.archived_at IS NULL) AS rule_count,
         d.created_at, d.updated_at FROM allocation_destinations d"
    };
}

pub struct DestinationFields {
    pub name: String,
    pub kind: String,
    pub target_amount: Option<i64>,
    pub minimum_balance: Option<i64>,
    pub notes: String,
}

pub async fn list_destinations(
    conn: &mut PgConnection,
) -> Result<Vec<DestinationRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        destination_columns!(),
        " ORDER BY d.archived_at IS NOT NULL, d.position, lower(d.name)"
    ))
    .fetch_all(conn)
    .await
}

pub async fn find_destination(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<DestinationRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        destination_columns!(),
        " WHERE d.id = $1"
    ))
    .bind(id)
    .fetch_optional(conn)
    .await
}

pub async fn destination_name_taken(
    conn: &mut PgConnection,
    name: &str,
    except: Option<Uuid>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM allocation_destinations WHERE lower(name) = lower($1)
           AND archived_at IS NULL AND ($2::uuid IS NULL OR id <> $2))",
    )
    .bind(name)
    .bind(except)
    .fetch_one(conn)
    .await
}

pub async fn insert_destination(
    conn: &mut PgConnection,
    business_id: Uuid,
    fields: &DestinationFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO allocation_destinations (business_id, name, kind, target_amount, minimum_balance, notes, position)
         VALUES ($1, $2, $3, $4, $5, $6,
                 (SELECT coalesce(max(position), 0) + 1 FROM allocation_destinations)) RETURNING id",
    )
    .bind(business_id)
    .bind(&fields.name)
    .bind(&fields.kind)
    .bind(fields.target_amount)
    .bind(fields.minimum_balance)
    .bind(&fields.notes)
    .fetch_one(conn)
    .await
}

pub async fn update_destination(
    conn: &mut PgConnection,
    id: Uuid,
    fields: &DestinationFields,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE allocation_destinations SET name = $2, kind = $3, target_amount = $4, minimum_balance = $5, notes = $6
         WHERE id = $1",
    )
    .bind(id)
    .bind(&fields.name)
    .bind(&fields.kind)
    .bind(fields.target_amount)
    .bind(fields.minimum_balance)
    .bind(&fields.notes)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn set_destination_archived(
    conn: &mut PgConnection,
    id: Uuid,
    archived: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE allocation_destinations SET archived_at = CASE WHEN $2 THEN now() ELSE NULL END WHERE id = $1",
    )
    .bind(id)
    .bind(archived)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn set_destination_position(
    conn: &mut PgConnection,
    id: Uuid,
    position: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE allocation_destinations SET position = $2 WHERE id = $1")
        .bind(id)
        .bind(position)
        .execute(conn)
        .await
        .map(|_| ())
}

// ─── Rules ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct RuleRow {
    pub id: Uuid,
    pub destination_id: Uuid,
    pub destination_name: String,
    /// Lower runs first.
    pub priority: i32,
    /// `percentage` or `fixed`.
    pub method: String,
    #[schema(value_type = Option<String>)]
    pub percentage: Option<Decimal>,
    /// `received` (of the amount received) or `remaining` (of what earlier rules left).
    pub basis: String,
    pub fixed_amount: Option<i64>,
    /// `any`, `invoice_payment` or `income`.
    pub source: String,
    /// `sales` or `other` (income only).
    pub income_type: Option<String>,
    pub customer_id: Option<Uuid>,
    pub customer_name: Option<String>,
    pub active_from: Option<NaiveDate>,
    pub active_to: Option<NaiveDate>,
    pub max_amount: Option<i64>,
    pub cap_amount: Option<i64>,
    /// `week`, `fortnight`, `month` or `custom`.
    pub cap_period: Option<String>,
    pub cap_period_days: Option<i32>,
    pub cap_anchor: Option<NaiveDate>,
    pub stop_at_target: bool,
    pub notes: String,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

macro_rules! rule_columns {
    () => {
        "r.id, r.destination_id, d.name AS destination_name, r.priority, r.method, r.percentage, r.basis,
         r.fixed_amount, r.source, r.income_type, r.customer_id, c.name AS customer_name, r.active_from,
         r.active_to, r.max_amount, r.cap_amount, r.cap_period, r.cap_period_days, r.cap_anchor,
         r.stop_at_target, r.notes, r.archived_at IS NOT NULL AS archived, r.created_at, r.updated_at
         FROM allocation_rules r
         JOIN allocation_destinations d ON d.id = r.destination_id
         LEFT JOIN customers c ON c.id = r.customer_id"
    };
}

pub struct RuleFields {
    pub destination_id: Uuid,
    pub method: String,
    pub percentage: Option<Decimal>,
    pub basis: String,
    pub fixed_amount: Option<i64>,
    pub source: String,
    pub income_type: Option<String>,
    pub customer_id: Option<Uuid>,
    pub active_from: Option<NaiveDate>,
    pub active_to: Option<NaiveDate>,
    pub max_amount: Option<i64>,
    pub cap_amount: Option<i64>,
    pub cap_period: Option<String>,
    pub cap_period_days: Option<i32>,
    pub cap_anchor: Option<NaiveDate>,
    pub stop_at_target: bool,
    pub notes: String,
}

/// Every rule, active ones first in priority order.
pub async fn list_rules(conn: &mut PgConnection) -> Result<Vec<RuleRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        rule_columns!(),
        " ORDER BY r.archived_at IS NOT NULL, r.priority, r.created_at"
    ))
    .fetch_all(conn)
    .await
}

pub async fn find_rule(conn: &mut PgConnection, id: Uuid) -> Result<Option<RuleRow>, sqlx::Error> {
    sqlx::query_as(concat!("SELECT ", rule_columns!(), " WHERE r.id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn customer_exists(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM customers WHERE id = $1)")
        .bind(id)
        .fetch_one(conn)
        .await
}

pub async fn active_rule_count(conn: &mut PgConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM allocation_rules WHERE archived_at IS NULL")
        .fetch_one(conn)
        .await
}

pub async fn insert_rule(
    conn: &mut PgConnection,
    business_id: Uuid,
    fields: &RuleFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO allocation_rules (business_id, destination_id, method, percentage, basis, fixed_amount, source,
           income_type, customer_id, active_from, active_to, max_amount, cap_amount, cap_period, cap_period_days,
           cap_anchor, stop_at_target, notes, priority)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18,
                 (SELECT coalesce(max(priority), 0) + 1 FROM allocation_rules)) RETURNING id",
    )
    .bind(business_id)
    .bind(fields.destination_id)
    .bind(&fields.method)
    .bind(fields.percentage)
    .bind(&fields.basis)
    .bind(fields.fixed_amount)
    .bind(&fields.source)
    .bind(&fields.income_type)
    .bind(fields.customer_id)
    .bind(fields.active_from)
    .bind(fields.active_to)
    .bind(fields.max_amount)
    .bind(fields.cap_amount)
    .bind(&fields.cap_period)
    .bind(fields.cap_period_days)
    .bind(fields.cap_anchor)
    .bind(fields.stop_at_target)
    .bind(&fields.notes)
    .fetch_one(conn)
    .await
}

pub async fn update_rule(
    conn: &mut PgConnection,
    id: Uuid,
    fields: &RuleFields,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE allocation_rules SET destination_id = $2, method = $3, percentage = $4, basis = $5, fixed_amount = $6,
           source = $7, income_type = $8, customer_id = $9, active_from = $10, active_to = $11, max_amount = $12,
           cap_amount = $13, cap_period = $14, cap_period_days = $15, cap_anchor = $16, stop_at_target = $17,
           notes = $18
         WHERE id = $1",
    )
    .bind(id)
    .bind(fields.destination_id)
    .bind(&fields.method)
    .bind(fields.percentage)
    .bind(&fields.basis)
    .bind(fields.fixed_amount)
    .bind(&fields.source)
    .bind(&fields.income_type)
    .bind(fields.customer_id)
    .bind(fields.active_from)
    .bind(fields.active_to)
    .bind(fields.max_amount)
    .bind(fields.cap_amount)
    .bind(&fields.cap_period)
    .bind(fields.cap_period_days)
    .bind(fields.cap_anchor)
    .bind(fields.stop_at_target)
    .bind(&fields.notes)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn set_rule_archived(
    conn: &mut PgConnection,
    id: Uuid,
    archived: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE allocation_rules SET archived_at = CASE WHEN $2 THEN now() ELSE NULL END WHERE id = $1",
    )
    .bind(id)
    .bind(archived)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn set_rule_priority(
    conn: &mut PgConnection,
    id: Uuid,
    priority: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE allocation_rules SET priority = $2 WHERE id = $1")
        .bind(id)
        .bind(priority)
        .execute(conn)
        .await
        .map(|_| ())
}

// ─── Allocations ────────────────────────────────────────────────────────────

/// A receipt being allocated: an invoice payment or an income record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRef {
    InvoicePayment(Uuid),
    Income(Uuid),
}

impl SourceRef {
    pub fn source_type(self) -> &'static str {
        match self {
            Self::InvoicePayment(_) => "invoice_payment",
            Self::Income(_) => "income",
        }
    }

    fn ids(self) -> (Option<Uuid>, Option<Uuid>) {
        match self {
            Self::InvoicePayment(id) => (Some(id), None),
            Self::Income(id) => (None, Some(id)),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AllocationRow {
    pub id: Uuid,
    /// `invoice_payment` or `income`.
    pub source_type: String,
    pub invoice_payment_id: Option<Uuid>,
    pub income_id: Option<Uuid>,
    /// The invoice the payment belongs to.
    pub invoice_id: Option<Uuid>,
    /// Invoice number or income payer.
    pub source_label: String,
    pub customer_id: Option<Uuid>,
    pub customer_name: Option<String>,
    /// `sales` or `other` for income.
    pub income_type: Option<String>,
    pub received_date: NaiveDate,
    pub received_amount: i64,
    pub currency: String,
    /// `pending`, `confirmed`, `skipped`, `reversed` or `cancelled`.
    pub status: String,
    /// Sum of the amounts allocated when confirmed (before any reversal).
    pub allocated: i64,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub close_reason: Option<String>,
    pub replaces_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

macro_rules! allocation_columns {
    () => {
        "a.id, a.source_type, a.invoice_payment_id, a.income_id, p.invoice_id,
         coalesce(CASE WHEN a.source_type = 'invoice_payment' THEN 'Invoice ' || coalesce(i.number, '') ELSE ir.payer END, '') AS source_label,
         coalesce(i.customer_id, ir.customer_id) AS customer_id, c.name AS customer_name, ir.income_type,
         a.received_date, a.received_amount, a.currency, a.status,
         coalesce((SELECT sum(e.amount) FROM allocation_entries e WHERE e.allocation_id = a.id AND e.kind = 'allocation'), 0)::bigint AS allocated,
         a.confirmed_at, a.closed_at, a.close_reason, a.replaces_id, a.created_at
         FROM allocations a
         LEFT JOIN invoice_payments p ON p.id = a.invoice_payment_id
         LEFT JOIN invoices i ON i.id = p.invoice_id
         LEFT JOIN income_records ir ON ir.id = a.income_id
         LEFT JOIN customers c ON c.id = coalesce(i.customer_id, ir.customer_id)"
    };
}

#[derive(Debug, Clone, Default)]
pub struct AllocationFilter {
    pub status: Option<String>,
    pub destination_id: Option<Uuid>,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub source: Option<SourceRef>,
}

macro_rules! allocation_filter {
    () => {
        " WHERE ($1::text IS NULL OR a.status = $1)
            AND ($2::uuid IS NULL OR EXISTS (SELECT 1 FROM allocation_entries e WHERE e.allocation_id = a.id AND e.destination_id = $2))
            AND ($3::date IS NULL OR a.received_date >= $3)
            AND ($4::date IS NULL OR a.received_date <= $4)
            AND ($5::uuid IS NULL OR a.invoice_payment_id = $5)
            AND ($6::uuid IS NULL OR a.income_id = $6)"
    };
}

pub async fn list_allocations(
    conn: &mut PgConnection,
    filter: &AllocationFilter,
    limit: i64,
    offset: i64,
) -> Result<Vec<AllocationRow>, sqlx::Error> {
    let (payment, income) = filter.source.map_or((None, None), SourceRef::ids);
    sqlx::query_as(concat!(
        "SELECT ",
        allocation_columns!(),
        allocation_filter!(),
        " ORDER BY a.received_date DESC, a.created_at DESC LIMIT $7 OFFSET $8"
    ))
    .bind(&filter.status)
    .bind(filter.destination_id)
    .bind(filter.from)
    .bind(filter.to)
    .bind(payment)
    .bind(income)
    .bind(limit)
    .bind(offset)
    .fetch_all(conn)
    .await
}

pub async fn count_allocations(
    conn: &mut PgConnection,
    filter: &AllocationFilter,
) -> Result<i64, sqlx::Error> {
    let (payment, income) = filter.source.map_or((None, None), SourceRef::ids);
    sqlx::query_scalar(concat!(
        "SELECT count(*) FROM allocations a",
        allocation_filter!()
    ))
    .bind(&filter.status)
    .bind(filter.destination_id)
    .bind(filter.from)
    .bind(filter.to)
    .bind(payment)
    .bind(income)
    .fetch_one(conn)
    .await
}

pub async fn find_allocation(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<AllocationRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        allocation_columns!(),
        " WHERE a.id = $1"
    ))
    .bind(id)
    .fetch_optional(conn)
    .await
}

/// Locks an allocation for a status change; false when it doesn't exist.
pub async fn lock_allocation(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM allocations WHERE id = $1 FOR UPDATE)")
        .bind(id)
        .fetch_one(conn)
        .await
}

/// The open (pending, confirmed or skipped) allocation of a receipt, locked.
pub async fn open_allocation_for(
    conn: &mut PgConnection,
    source: SourceRef,
) -> Result<Option<(Uuid, String, i64)>, sqlx::Error> {
    let (payment, income) = source.ids();
    sqlx::query_as(
        "SELECT id, status, received_amount FROM allocations
         WHERE status IN ('pending', 'confirmed', 'skipped')
           AND ($1::uuid IS NULL OR invoice_payment_id = $1) AND ($2::uuid IS NULL OR income_id = $2)
         FOR UPDATE",
    )
    .bind(payment)
    .bind(income)
    .fetch_optional(conn)
    .await
}

pub async fn insert_allocation(
    conn: &mut PgConnection,
    business_id: Uuid,
    source: SourceRef,
    received_date: NaiveDate,
    received_amount: i64,
    currency: &str,
    replaces_id: Option<Uuid>,
) -> Result<Uuid, sqlx::Error> {
    let (payment, income) = source.ids();
    sqlx::query_scalar(
        "INSERT INTO allocations (business_id, source_type, invoice_payment_id, income_id, received_date,
           received_amount, currency, replaces_id)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
    )
    .bind(business_id)
    .bind(source.source_type())
    .bind(payment)
    .bind(income)
    .bind(received_date)
    .bind(received_amount)
    .bind(currency)
    .bind(replaces_id)
    .fetch_one(conn)
    .await
}

pub async fn update_received(
    conn: &mut PgConnection,
    id: Uuid,
    received_date: NaiveDate,
    received_amount: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE allocations SET received_date = $2, received_amount = $3 WHERE id = $1")
        .bind(id)
        .bind(received_date)
        .bind(received_amount)
        .execute(conn)
        .await
        .map(|_| ())
}

pub async fn set_status(
    conn: &mut PgConnection,
    id: Uuid,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE allocations SET status = $2 WHERE id = $1")
        .bind(id)
        .bind(status)
        .execute(conn)
        .await
        .map(|_| ())
}

pub async fn mark_confirmed(
    conn: &mut PgConnection,
    id: Uuid,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE allocations SET status = 'confirmed', confirmed_at = now(), confirmed_by = $2 WHERE id = $1",
    )
    .bind(id)
    .bind(user_id)
    .execute(conn)
    .await
    .map(|_| ())
}

/// Closes an allocation as `reversed` or `cancelled`.
pub async fn close(
    conn: &mut PgConnection,
    id: Uuid,
    status: &str,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE allocations SET status = $2, closed_at = now(), close_reason = $3 WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .bind(reason)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn pending_count(conn: &mut PgConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM allocations WHERE status = 'pending'")
        .fetch_one(conn)
        .await
}

// ─── Entries ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct EntryRow {
    pub id: Uuid,
    pub allocation_id: Uuid,
    pub destination_id: Uuid,
    pub destination_name: String,
    pub rule_id: Option<Uuid>,
    /// `allocation` or `reversal`.
    pub kind: String,
    /// Minor units; negative for reversals.
    pub amount: i64,
    pub reverses_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

pub async fn entries(
    conn: &mut PgConnection,
    allocation_ids: &[Uuid],
) -> Result<Vec<EntryRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT e.id, e.allocation_id, e.destination_id, d.name AS destination_name, e.rule_id, e.kind, e.amount,
                e.reverses_id, e.created_at
         FROM allocation_entries e JOIN allocation_destinations d ON d.id = e.destination_id
         WHERE e.allocation_id = ANY($1)
         ORDER BY e.created_at, d.position, e.id",
    )
    .bind(allocation_ids)
    .fetch_all(conn)
    .await
}

pub async fn insert_entry(
    conn: &mut PgConnection,
    business_id: Uuid,
    allocation_id: Uuid,
    destination_id: Uuid,
    rule_id: Option<Uuid>,
    amount: i64,
    user_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO allocation_entries (business_id, allocation_id, destination_id, rule_id, kind, amount, created_by)
         VALUES ($1, $2, $3, $4, 'allocation', $5, $6)",
    )
    .bind(business_id)
    .bind(allocation_id)
    .bind(destination_id)
    .bind(rule_id)
    .bind(amount)
    .bind(user_id)
    .execute(conn)
    .await
    .map(|_| ())
}

/// Adds a reversing entry for every allocation entry of the allocation not yet reversed.
pub async fn reverse_entries(
    conn: &mut PgConnection,
    allocation_id: Uuid,
    user_id: Uuid,
) -> Result<u64, sqlx::Error> {
    sqlx::query(
        "INSERT INTO allocation_entries (business_id, allocation_id, destination_id, rule_id, kind, amount, reverses_id, created_by)
         SELECT e.business_id, e.allocation_id, e.destination_id, e.rule_id, 'reversal', -e.amount, e.id, $2
         FROM allocation_entries e
         WHERE e.allocation_id = $1 AND e.kind = 'allocation'
           AND NOT EXISTS (SELECT 1 FROM allocation_entries r WHERE r.reverses_id = e.id)",
    )
    .bind(allocation_id)
    .bind(user_id)
    .execute(conn)
    .await
    .map(|r| r.rows_affected())
}

/// Amount each rule allocated from receipts dated within `[from, to]`, net of reversals and
/// excluding one allocation.
pub async fn rule_total_between(
    conn: &mut PgConnection,
    rule_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
    except_allocation: Uuid,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT coalesce(sum(e.amount), 0)::bigint FROM allocation_entries e JOIN allocations a ON a.id = e.allocation_id
         WHERE e.rule_id = $1 AND a.received_date BETWEEN $2 AND $3 AND a.id <> $4",
    )
    .bind(rule_id)
    .bind(from)
    .bind(to)
    .bind(except_allocation)
    .fetch_one(conn)
    .await
}

/// Balance of every destination: what was set aside less what was spent.
pub async fn balances(conn: &mut PgConnection) -> Result<Vec<(Uuid, i64)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT destination_id, sum(amount)::bigint FROM (
           SELECT destination_id, amount FROM allocation_entries
           UNION ALL SELECT destination_id, -amount FROM allocation_spending WHERE voided_at IS NULL
         ) m GROUP BY destination_id",
    )
    .fetch_all(conn)
    .await
}

// ─── Spending ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct SpendingRow {
    pub id: Uuid,
    pub destination_id: Uuid,
    pub destination_name: String,
    pub spent_on: NaiveDate,
    /// Minor units.
    pub amount: i64,
    pub description: String,
    pub notes: String,
    pub voided: bool,
    pub void_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

macro_rules! spending_columns {
    () => {
        "s.id, s.destination_id, d.name AS destination_name, s.spent_on, s.amount, s.description, s.notes,
         s.voided_at IS NOT NULL AS voided, s.void_reason, s.created_at
         FROM allocation_spending s JOIN allocation_destinations d ON d.id = s.destination_id"
    };
}

/// Spending, newest first: of one destination or all, in a period, with or without deleted ones.
pub async fn list_spending(
    conn: &mut PgConnection,
    destination_id: Option<Uuid>,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    include_voided: bool,
) -> Result<Vec<SpendingRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        spending_columns!(),
        " WHERE ($1::uuid IS NULL OR s.destination_id = $1) AND ($2::date IS NULL OR s.spent_on >= $2)
            AND ($3::date IS NULL OR s.spent_on <= $3) AND ($4 OR s.voided_at IS NULL)
          ORDER BY s.spent_on DESC, s.created_at DESC"
    ))
    .bind(destination_id)
    .bind(from)
    .bind(to)
    .bind(include_voided)
    .fetch_all(conn)
    .await
}

pub async fn find_spending(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<SpendingRow>, sqlx::Error> {
    sqlx::query_as(concat!("SELECT ", spending_columns!(), " WHERE s.id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub struct SpendingFields {
    pub destination_id: Uuid,
    pub spent_on: NaiveDate,
    pub amount: i64,
    pub description: String,
    pub notes: String,
}

pub async fn insert_spending(
    conn: &mut PgConnection,
    business_id: Uuid,
    user_id: Uuid,
    f: &SpendingFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO allocation_spending (business_id, destination_id, spent_on, amount, description, notes, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(business_id)
    .bind(f.destination_id)
    .bind(f.spent_on)
    .bind(f.amount)
    .bind(&f.description)
    .bind(&f.notes)
    .bind(user_id)
    .fetch_one(conn)
    .await
}

pub async fn void_spending(
    conn: &mut PgConnection,
    id: Uuid,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE allocation_spending SET voided_at = now(), void_reason = $2 WHERE id = $1 AND voided_at IS NULL",
    )
    .bind(id)
    .bind(reason)
    .execute(conn)
    .await
    .map(|_| ())
}

// ─── Receipts recorded before any rule ──────────────────────────────────────

/// Money received from `from` to `to` (invoice payments other than write-offs, and income; none
/// deleted) that has never had an allocation.
pub async fn receipts_without_allocation(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<(String, Uuid, NaiveDate, i64)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT 'invoice_payment', p.id, p.payment_date, p.amount FROM invoice_payments p
         WHERE p.voided_at IS NULL AND p.method <> 'write_off' AND p.payment_date BETWEEN $1 AND $2
           AND NOT EXISTS (SELECT 1 FROM allocations a WHERE a.invoice_payment_id = p.id)
         UNION ALL
         SELECT 'income', i.id, i.income_date, i.gross_amount FROM income_records i
         WHERE i.voided_at IS NULL AND i.income_date BETWEEN $1 AND $2
           AND NOT EXISTS (SELECT 1 FROM allocations a WHERE a.income_id = i.id)
         ORDER BY 3, 2",
    )
    .bind(from)
    .bind(to)
    .fetch_all(conn)
    .await
}
