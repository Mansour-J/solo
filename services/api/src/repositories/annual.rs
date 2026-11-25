//! Records an accountant's annual report needs beyond day-to-day bookkeeping (ADR 0013): money
//! moved in and out of the business account, fixed assets with their yearly depreciation, and
//! opening balances. Row-level security limits every query to the current business.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

// ─── Money in and out ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct MovementRow {
    pub id: Uuid,
    pub movement_date: NaiveDate,
    /// `drawing`, `funds_introduced`, `gst_payment`, `gst_refund`, `income_tax_payment` or
    /// `income_tax_refund`.
    pub kind: String,
    /// Minor units.
    pub amount: i64,
    pub currency: String,
    pub description: String,
    pub reference: String,
    pub voided_at: Option<DateTime<Utc>>,
    pub void_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

macro_rules! movement_columns {
    () => {
        "id, movement_date, kind, amount, currency, description, reference, voided_at, void_reason, created_at FROM cash_movements"
    };
}

pub struct MovementFields {
    pub movement_date: NaiveDate,
    pub kind: String,
    pub amount: i64,
    pub currency: String,
    pub description: String,
    pub reference: String,
}

/// Movements in `[from, to]` (all when unset), optionally of one kind, newest first.
pub async fn list_movements(
    conn: &mut PgConnection,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    kind: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<(Vec<MovementRow>, i64), sqlx::Error> {
    let rows = sqlx::query_as(concat!(
        "SELECT ",
        movement_columns!(),
        " WHERE voided_at IS NULL AND ($1::date IS NULL OR movement_date >= $1) AND ($2::date IS NULL OR movement_date <= $2)
            AND ($3::text IS NULL OR kind = $3)
          ORDER BY movement_date DESC, created_at DESC LIMIT $4 OFFSET $5"
    ))
    .bind(from)
    .bind(to)
    .bind(kind)
    .bind(limit)
    .bind(offset)
    .fetch_all(&mut *conn)
    .await?;
    let total = sqlx::query_scalar(
        "SELECT count(*) FROM cash_movements WHERE voided_at IS NULL AND ($1::date IS NULL OR movement_date >= $1)
           AND ($2::date IS NULL OR movement_date <= $2) AND ($3::text IS NULL OR kind = $3)",
    )
    .bind(from)
    .bind(to)
    .bind(kind)
    .fetch_one(conn)
    .await?;
    Ok((rows, total))
}

pub async fn find_movement(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<MovementRow>, sqlx::Error> {
    sqlx::query_as(concat!("SELECT ", movement_columns!(), " WHERE id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn insert_movement(
    conn: &mut PgConnection,
    business_id: Uuid,
    user_id: Uuid,
    f: &MovementFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO cash_movements (business_id, movement_date, kind, amount, currency, description, reference, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
    )
    .bind(business_id)
    .bind(f.movement_date)
    .bind(&f.kind)
    .bind(f.amount)
    .bind(&f.currency)
    .bind(&f.description)
    .bind(&f.reference)
    .bind(user_id)
    .fetch_one(conn)
    .await
}

pub async fn update_movement(
    conn: &mut PgConnection,
    id: Uuid,
    f: &MovementFields,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE cash_movements SET movement_date = $2, kind = $3, amount = $4, description = $5, reference = $6
         WHERE id = $1 AND voided_at IS NULL",
    )
    .bind(id)
    .bind(f.movement_date)
    .bind(&f.kind)
    .bind(f.amount)
    .bind(&f.description)
    .bind(&f.reference)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn void_movement(
    conn: &mut PgConnection,
    id: Uuid,
    reason: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE cash_movements SET voided_at = now(), void_reason = $2 WHERE id = $1 AND voided_at IS NULL")
        .bind(id)
        .bind(reason)
        .execute(conn)
        .await
        .map(|_| ())
}

// ─── Fixed assets ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct AssetRow {
    pub id: Uuid,
    pub name: String,
    pub asset_group: String,
    /// Minor units, excluding GST.
    pub cost: i64,
    pub purchase_gst: i64,
    pub acquired_on: NaiveDate,
    /// `dv` or `sl`.
    pub method: String,
    /// Percent a year, e.g. "50.00".
    #[schema(value_type = String)]
    pub rate: Decimal,
    #[schema(value_type = String)]
    pub private_use: Decimal,
    /// Book value at the opening balance date, for an asset brought forward.
    pub opening_value: Option<i64>,
    /// Book value after the latest depreciation posted (or the opening value or cost).
    pub book_value: i64,
    pub notes: String,
    pub archived: bool,
    pub created_at: DateTime<Utc>,
    /// When it was sold or scrapped, if it was.
    pub disposed_on: Option<NaiveDate>,
}

macro_rules! asset_columns {
    () => {
        "a.id, a.name, a.asset_group, a.cost, a.purchase_gst, a.acquired_on, a.method, a.rate, a.private_use, a.opening_value,
         coalesce((SELECT d.closing_value FROM depreciation_entries d WHERE d.asset_id = a.id ORDER BY d.year_end DESC LIMIT 1),
                  a.opening_value, a.cost) AS book_value,
         a.notes, a.archived_at IS NOT NULL AS archived, a.created_at,
         (SELECT x.disposed_on FROM asset_disposals x WHERE x.asset_id = a.id) AS disposed_on
         FROM fixed_assets a"
    };
}

pub struct AssetFields {
    pub name: String,
    pub asset_group: String,
    pub cost: i64,
    pub purchase_gst: i64,
    pub acquired_on: NaiveDate,
    pub method: String,
    pub rate: Decimal,
    pub private_use: Decimal,
    pub opening_value: Option<i64>,
    pub notes: String,
}

pub async fn list_assets(conn: &mut PgConnection) -> Result<Vec<AssetRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        asset_columns!(),
        " ORDER BY a.archived_at IS NOT NULL, lower(a.asset_group), a.acquired_on, lower(a.name)"
    ))
    .fetch_all(conn)
    .await
}

pub async fn find_asset(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<AssetRow>, sqlx::Error> {
    sqlx::query_as(concat!("SELECT ", asset_columns!(), " WHERE a.id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn insert_asset(
    conn: &mut PgConnection,
    business_id: Uuid,
    f: &AssetFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO fixed_assets (business_id, name, asset_group, cost, purchase_gst, acquired_on, method, rate, private_use,
           opening_value, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING id",
    )
    .bind(business_id)
    .bind(&f.name)
    .bind(&f.asset_group)
    .bind(f.cost)
    .bind(f.purchase_gst)
    .bind(f.acquired_on)
    .bind(&f.method)
    .bind(f.rate)
    .bind(f.private_use)
    .bind(f.opening_value)
    .bind(&f.notes)
    .fetch_one(conn)
    .await
}

pub async fn update_asset(
    conn: &mut PgConnection,
    id: Uuid,
    f: &AssetFields,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE fixed_assets SET name = $2, asset_group = $3, cost = $4, purchase_gst = $5, acquired_on = $6, method = $7,
           rate = $8, private_use = $9, opening_value = $10, notes = $11
         WHERE id = $1",
    )
    .bind(id)
    .bind(&f.name)
    .bind(&f.asset_group)
    .bind(f.cost)
    .bind(f.purchase_gst)
    .bind(f.acquired_on)
    .bind(&f.method)
    .bind(f.rate)
    .bind(f.private_use)
    .bind(f.opening_value)
    .bind(&f.notes)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn set_asset_archived(
    conn: &mut PgConnection,
    id: Uuid,
    archived: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE fixed_assets SET archived_at = CASE WHEN $2 THEN now() ELSE NULL END WHERE id = $1",
    )
    .bind(id)
    .bind(archived)
    .execute(conn)
    .await
    .map(|_| ())
}

// ─── Depreciation ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct DepreciationRow {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub year_end: NaiveDate,
    pub opening_value: i64,
    pub purchases: i64,
    pub depreciation: i64,
    pub private_amount: i64,
    pub closing_value: i64,
}

pub struct DepreciationFields {
    pub opening_value: i64,
    pub purchases: i64,
    pub depreciation: i64,
    pub private_amount: i64,
    pub closing_value: i64,
}

/// Depreciation entries of every asset (or one), oldest year first.
pub async fn depreciation_entries(
    conn: &mut PgConnection,
    asset_id: Option<Uuid>,
) -> Result<Vec<DepreciationRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, asset_id, year_end, opening_value, purchases, depreciation, private_amount, closing_value
         FROM depreciation_entries WHERE $1::uuid IS NULL OR asset_id = $1 ORDER BY year_end, asset_id",
    )
    .bind(asset_id)
    .fetch_all(conn)
    .await
}

/// Inserts or replaces an asset's depreciation for a year; returns the entry's id.
pub async fn upsert_depreciation(
    conn: &mut PgConnection,
    business_id: Uuid,
    asset_id: Uuid,
    year_end: NaiveDate,
    f: &DepreciationFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO depreciation_entries (business_id, asset_id, year_end, opening_value, purchases, depreciation,
           private_amount, closing_value)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (asset_id, year_end) DO UPDATE SET opening_value = EXCLUDED.opening_value,
           purchases = EXCLUDED.purchases, depreciation = EXCLUDED.depreciation,
           private_amount = EXCLUDED.private_amount, closing_value = EXCLUDED.closing_value
         RETURNING id",
    )
    .bind(business_id)
    .bind(asset_id)
    .bind(year_end)
    .bind(f.opening_value)
    .bind(f.purchases)
    .bind(f.depreciation)
    .bind(f.private_amount)
    .bind(f.closing_value)
    .fetch_one(conn)
    .await
}

/// Removes depreciation for years after `year_end` for an asset (they'd be out of date); returns
/// the removed entries' ids so their postings can be reversed.
pub async fn delete_depreciation_after(
    conn: &mut PgConnection,
    asset_id: Uuid,
    year_end: NaiveDate,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "DELETE FROM depreciation_entries WHERE asset_id = $1 AND year_end > $2 RETURNING id",
    )
    .bind(asset_id)
    .bind(year_end)
    .fetch_all(conn)
    .await
}

pub async fn delete_depreciation(conn: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM depreciation_entries WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await
        .map(|_| ())
}

// ─── Opening balances ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct OpeningRow {
    pub id: Uuid,
    pub as_at: NaiveDate,
    pub bank: i64,
    pub gst_payable: i64,
    pub drawings: i64,
    pub funds_introduced: i64,
    pub personal_accounts: i64,
    pub withholding_tax: i64,
    pub income_tax: i64,
    pub updated_at: DateTime<Utc>,
}

pub struct OpeningFields {
    pub as_at: NaiveDate,
    pub bank: i64,
    pub gst_payable: i64,
    pub drawings: i64,
    pub funds_introduced: i64,
    pub personal_accounts: i64,
    pub withholding_tax: i64,
    pub income_tax: i64,
}

pub async fn find_opening(conn: &mut PgConnection) -> Result<Option<OpeningRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, as_at, bank, gst_payable, drawings, funds_introduced, personal_accounts, withholding_tax, income_tax, updated_at
         FROM opening_balances",
    )
    .fetch_optional(conn)
    .await
}

pub async fn save_opening(
    conn: &mut PgConnection,
    business_id: Uuid,
    f: &OpeningFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO opening_balances (business_id, as_at, bank, gst_payable, drawings, funds_introduced, personal_accounts,
           withholding_tax, income_tax)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
         ON CONFLICT (business_id) DO UPDATE SET as_at = EXCLUDED.as_at, bank = EXCLUDED.bank,
           gst_payable = EXCLUDED.gst_payable, drawings = EXCLUDED.drawings, funds_introduced = EXCLUDED.funds_introduced,
           personal_accounts = EXCLUDED.personal_accounts, withholding_tax = EXCLUDED.withholding_tax,
           income_tax = EXCLUDED.income_tax
         RETURNING id",
    )
    .bind(business_id)
    .bind(f.as_at)
    .bind(f.bank)
    .bind(f.gst_payable)
    .bind(f.drawings)
    .bind(f.funds_introduced)
    .bind(f.personal_accounts)
    .bind(f.withholding_tax)
    .bind(f.income_tax)
    .fetch_one(conn)
    .await
}

pub async fn delete_opening(conn: &mut PgConnection) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar("DELETE FROM opening_balances RETURNING id")
        .fetch_optional(conn)
        .await
}

/// Sum of brought-forward asset values (assets acquired by the opening date with an opening value).
pub async fn opening_asset_total(conn: &mut PgConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT coalesce(sum(opening_value), 0)::bigint FROM fixed_assets WHERE opening_value IS NOT NULL")
        .fetch_one(conn)
        .await
}

// ─── GST change-of-use reviews ──────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UseReviewRow {
    pub id: Uuid,
    pub period_end: NaiveDate,
    pub business_use: Decimal,
    pub notes: String,
}

/// An asset's reviews, oldest first.
pub async fn use_reviews(
    conn: &mut PgConnection,
    asset_id: Uuid,
) -> Result<Vec<UseReviewRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, period_end, business_use, notes FROM asset_use_reviews WHERE asset_id = $1 ORDER BY period_end",
    )
    .bind(asset_id)
    .fetch_all(conn)
    .await
}

pub async fn save_use_review(
    conn: &mut PgConnection,
    business_id: Uuid,
    asset_id: Uuid,
    period_end: NaiveDate,
    business_use: Decimal,
    notes: &str,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO asset_use_reviews (business_id, asset_id, period_end, business_use, notes) VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (asset_id, period_end) DO UPDATE SET business_use = EXCLUDED.business_use, notes = EXCLUDED.notes
         RETURNING id",
    )
    .bind(business_id)
    .bind(asset_id)
    .bind(period_end)
    .bind(business_use)
    .bind(notes)
    .fetch_one(conn)
    .await
}

pub async fn delete_use_review(
    conn: &mut PgConnection,
    asset_id: Uuid,
    period_end: NaiveDate,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM asset_use_reviews WHERE asset_id = $1 AND period_end >= $2")
        .bind(asset_id)
        .bind(period_end)
        .execute(conn)
        .await
        .map(|_| ())
}

// ─── Disposals ──────────────────────────────────────────────────────────────

/// An asset sold or scrapped, with what it came to (minor units).
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct DisposalRow {
    pub id: Uuid,
    pub asset_id: Uuid,
    pub disposed_on: NaiveDate,
    /// Received, including any GST (0 when scrapped).
    pub sale_price: i64,
    pub sale_gst: i64,
    /// Adjusted tax value at the start of the year of disposal.
    pub book_value: i64,
    /// Depreciation recovered (business share, taxable income).
    pub recovered: i64,
    /// Loss on disposal (business share, deductible).
    pub loss: i64,
    /// The sale above cost (not taxable).
    pub capital_gain: i64,
    pub notes: String,
}

macro_rules! disposal_columns {
    () => {
        "id, asset_id, disposed_on, sale_price, sale_gst, book_value, recovered, loss, capital_gain, notes FROM asset_disposals"
    };
}

pub async fn find_disposal(
    conn: &mut PgConnection,
    asset_id: Uuid,
) -> Result<Option<DisposalRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        disposal_columns!(),
        " WHERE asset_id = $1"
    ))
    .bind(asset_id)
    .fetch_optional(conn)
    .await
}

pub async fn list_disposals(conn: &mut PgConnection) -> Result<Vec<DisposalRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        disposal_columns!(),
        " ORDER BY disposed_on"
    ))
    .fetch_all(conn)
    .await
}

pub struct DisposalFields {
    pub disposed_on: NaiveDate,
    pub sale_price: i64,
    pub sale_gst: i64,
    pub book_value: i64,
    pub recovered: i64,
    pub loss: i64,
    pub capital_gain: i64,
    pub notes: String,
}

pub async fn save_disposal(
    conn: &mut PgConnection,
    business_id: Uuid,
    asset_id: Uuid,
    f: &DisposalFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO asset_disposals (business_id, asset_id, disposed_on, sale_price, sale_gst, book_value, recovered,
           loss, capital_gain, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         ON CONFLICT (asset_id) DO UPDATE SET disposed_on = EXCLUDED.disposed_on, sale_price = EXCLUDED.sale_price,
           sale_gst = EXCLUDED.sale_gst, book_value = EXCLUDED.book_value, recovered = EXCLUDED.recovered,
           loss = EXCLUDED.loss, capital_gain = EXCLUDED.capital_gain, notes = EXCLUDED.notes
         RETURNING id",
    )
    .bind(business_id)
    .bind(asset_id)
    .bind(f.disposed_on)
    .bind(f.sale_price)
    .bind(f.sale_gst)
    .bind(f.book_value)
    .bind(f.recovered)
    .bind(f.loss)
    .bind(f.capital_gain)
    .bind(&f.notes)
    .fetch_one(conn)
    .await
}

pub async fn delete_disposal(
    conn: &mut PgConnection,
    asset_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar("DELETE FROM asset_disposals WHERE asset_id = $1 RETURNING id")
        .bind(asset_id)
        .fetch_optional(conn)
        .await
}
