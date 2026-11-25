//! Businesses, memberships and settings. All queries run in a transaction with request context, so
//! row-level security only exposes the caller's businesses.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::domain::business::{BusinessType, GstBasis, ReportingBasis};
use crate::domain::gst_returns::GstPeriod;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BusinessRow {
    pub id: Uuid,
    pub name: String,
    pub trading_name: String,
    pub business_type: BusinessType,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: serde_json::Value,
    pub nzbn: Option<String>,
    pub gst_registered: bool,
    pub gst_number: Option<String>,
    pub gst_registered_from: Option<NaiveDate>,
    pub gst_basis: GstBasis,
    pub gst_period: GstPeriod,
    pub gst_period_end_month: i16,
    pub currency: String,
    pub locale: String,
    pub timezone: String,
    pub financial_year_start_month: i16,
    pub financial_year_start_day: i16,
    pub reporting_basis: ReportingBasis,
    pub logo_object_key: Option<String>,
    pub onboarding_completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

macro_rules! columns {
    () => {
        "id, name, trading_name, business_type, email, phone, address, nzbn, gst_registered, gst_number, \
    gst_registered_from, gst_basis, gst_period, gst_period_end_month, currency, locale, timezone, financial_year_start_month, financial_year_start_day, \
    reporting_basis, logo_object_key, onboarding_completed_at, created_at, updated_at"
    };
}

/// Everything a business profile can hold; used for both create and update.
#[derive(Debug, Clone)]
pub struct BusinessFields {
    pub name: String,
    pub trading_name: String,
    pub business_type: BusinessType,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: serde_json::Value,
    pub nzbn: Option<String>,
    pub gst_registered: bool,
    pub gst_number: Option<String>,
    pub gst_registered_from: Option<NaiveDate>,
    pub gst_basis: GstBasis,
    pub gst_period: GstPeriod,
    pub gst_period_end_month: i16,
    pub currency: String,
    pub financial_year_start_month: i16,
    pub financial_year_start_day: i16,
    pub reporting_basis: ReportingBasis,
}

impl BusinessFields {
    pub fn from_row(row: &BusinessRow) -> Self {
        Self {
            name: row.name.clone(),
            trading_name: row.trading_name.clone(),
            business_type: row.business_type,
            email: row.email.clone(),
            phone: row.phone.clone(),
            address: row.address.clone(),
            nzbn: row.nzbn.clone(),
            gst_registered: row.gst_registered,
            gst_number: row.gst_number.clone(),
            gst_registered_from: row.gst_registered_from,
            gst_basis: row.gst_basis,
            gst_period: row.gst_period,
            gst_period_end_month: row.gst_period_end_month,
            currency: row.currency.clone(),
            financial_year_start_month: row.financial_year_start_month,
            financial_year_start_day: row.financial_year_start_day,
            reporting_basis: row.reporting_basis,
        }
    }
}

/// Creates the business, the owner's membership and default settings. The transaction must have
/// `solo.business_id` set to `id` (row-level security checks new rows against it).
pub async fn insert(
    conn: &mut PgConnection,
    id: Uuid,
    owner: Uuid,
    fields: &BusinessFields,
    onboarding_complete: bool,
) -> Result<BusinessRow, sqlx::Error> {
    let row: BusinessRow = sqlx::query_as(concat!(
        "INSERT INTO businesses (id, name, trading_name, business_type, email, phone, address, nzbn, gst_registered,
            gst_number, gst_registered_from, gst_basis, currency, financial_year_start_month, financial_year_start_day,
            reporting_basis, onboarding_completed_at, created_by, gst_period, gst_period_end_month)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16,
                 CASE WHEN $17 THEN now() END, $18, $19, $20)
         RETURNING ", columns!(), ""
    ))
    .bind(id)
    .bind(&fields.name)
    .bind(&fields.trading_name)
    .bind(fields.business_type)
    .bind(&fields.email)
    .bind(&fields.phone)
    .bind(&fields.address)
    .bind(&fields.nzbn)
    .bind(fields.gst_registered)
    .bind(&fields.gst_number)
    .bind(fields.gst_registered_from)
    .bind(fields.gst_basis)
    .bind(&fields.currency)
    .bind(fields.financial_year_start_month)
    .bind(fields.financial_year_start_day)
    .bind(fields.reporting_basis)
    .bind(onboarding_complete)
    .bind(owner)
    .bind(fields.gst_period)
    .bind(fields.gst_period_end_month)
    .fetch_one(&mut *conn)
    .await?;
    sqlx::query(
        "INSERT INTO business_members (business_id, user_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(id)
    .bind(owner)
    .execute(&mut *conn)
    .await?;
    sqlx::query("INSERT INTO business_settings (business_id) VALUES ($1)")
        .bind(id)
        .execute(conn)
        .await?;
    Ok(row)
}

pub async fn update(
    conn: &mut PgConnection,
    id: Uuid,
    fields: &BusinessFields,
    complete_onboarding: bool,
) -> Result<BusinessRow, sqlx::Error> {
    sqlx::query_as(concat!(
        "UPDATE businesses SET name = $2, trading_name = $3, business_type = $4, email = $5, phone = $6, address = $7,
            nzbn = $8, gst_registered = $9, gst_number = $10, gst_registered_from = $11, gst_basis = $12, currency = $13,
            financial_year_start_month = $14, financial_year_start_day = $15, reporting_basis = $16,
            onboarding_completed_at = CASE WHEN $17 THEN COALESCE(onboarding_completed_at, now()) ELSE onboarding_completed_at END,
            gst_period = $18, gst_period_end_month = $19
         WHERE id = $1 RETURNING ", columns!(), ""
    ))
    .bind(id)
    .bind(&fields.name)
    .bind(&fields.trading_name)
    .bind(fields.business_type)
    .bind(&fields.email)
    .bind(&fields.phone)
    .bind(&fields.address)
    .bind(&fields.nzbn)
    .bind(fields.gst_registered)
    .bind(&fields.gst_number)
    .bind(fields.gst_registered_from)
    .bind(fields.gst_basis)
    .bind(&fields.currency)
    .bind(fields.financial_year_start_month)
    .bind(fields.financial_year_start_day)
    .bind(fields.reporting_basis)
    .bind(complete_onboarding)
    .bind(fields.gst_period)
    .bind(fields.gst_period_end_month)
    .fetch_one(conn)
    .await
}

pub async fn find(conn: &mut PgConnection, id: Uuid) -> Result<Option<BusinessRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM businesses WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(conn)
    .await
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct MembershipRow {
    pub business_id: Uuid,
    pub name: String,
    pub logo_object_key: Option<String>,
    pub onboarding_completed_at: Option<DateTime<Utc>>,
    pub currency: String,
}

/// Businesses the current user belongs to (requires `solo.user_id`).
pub async fn list_for_user(
    conn: &mut PgConnection,
    user_id: Uuid,
) -> Result<Vec<MembershipRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT b.id AS business_id, b.name, b.logo_object_key, b.onboarding_completed_at, b.currency
         FROM business_members m JOIN businesses b ON b.id = m.business_id
         WHERE m.user_id = $1
         ORDER BY b.created_at",
    )
    .bind(user_id)
    .fetch_all(conn)
    .await
}

/// Whether the user owns the business.
pub async fn is_member(
    conn: &mut PgConnection,
    business_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM business_members WHERE business_id = $1 AND user_id = $2)",
    )
    .bind(business_id)
    .bind(user_id)
    .fetch_one(conn)
    .await
}

pub async fn set_logo(
    conn: &mut PgConnection,
    id: Uuid,
    key: Option<&str>,
) -> Result<Option<String>, sqlx::Error> {
    let previous: Option<Option<String>> =
        sqlx::query_scalar("SELECT logo_object_key FROM businesses WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;
    sqlx::query("UPDATE businesses SET logo_object_key = $2 WHERE id = $1")
        .bind(id)
        .bind(key)
        .execute(conn)
        .await?;
    Ok(previous.flatten())
}

pub async fn delete(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    sqlx::query("DELETE FROM businesses WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await
        .map(|r| r.rows_affected() > 0)
}

// ─── Settings ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SettingsRow {
    pub invoice_prefix: String,
    pub next_invoice_number: i32,
    pub quote_prefix: String,
    pub next_quote_number: i32,
    pub number_padding: i16,
    pub default_payment_terms_days: i32,
    pub default_quote_validity_days: i32,
    pub default_invoice_notes: String,
    pub default_invoice_footer: String,
    pub payment_instructions: String,
    pub bank_account_name: String,
    pub bank_account_number: String,
    pub invoice_accent_color: String,
    pub default_deductible_percentage: Decimal,
}

macro_rules! settings_columns {
    () => {
        "invoice_prefix, next_invoice_number, quote_prefix, next_quote_number, number_padding, \
    default_payment_terms_days, default_quote_validity_days, default_invoice_notes, default_invoice_footer, \
    payment_instructions, bank_account_name, bank_account_number, invoice_accent_color, default_deductible_percentage"
    };
}

pub async fn settings(
    conn: &mut PgConnection,
    business_id: Uuid,
) -> Result<SettingsRow, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        settings_columns!(),
        " FROM business_settings WHERE business_id = $1"
    ))
    .bind(business_id)
    .fetch_one(conn)
    .await
}

pub async fn update_settings(
    conn: &mut PgConnection,
    business_id: Uuid,
    s: &SettingsRow,
) -> Result<SettingsRow, sqlx::Error> {
    sqlx::query_as(concat!(
        "UPDATE business_settings SET invoice_prefix = $2, next_invoice_number = $3, quote_prefix = $4,
            next_quote_number = $5, number_padding = $6, default_payment_terms_days = $7,
            default_quote_validity_days = $8, default_invoice_notes = $9, default_invoice_footer = $10,
            payment_instructions = $11, bank_account_name = $12, bank_account_number = $13,
            invoice_accent_color = $14, default_deductible_percentage = $15
         WHERE business_id = $1 RETURNING ", settings_columns!(), ""
    ))
    .bind(business_id)
    .bind(&s.invoice_prefix)
    .bind(s.next_invoice_number)
    .bind(&s.quote_prefix)
    .bind(s.next_quote_number)
    .bind(s.number_padding)
    .bind(s.default_payment_terms_days)
    .bind(s.default_quote_validity_days)
    .bind(&s.default_invoice_notes)
    .bind(&s.default_invoice_footer)
    .bind(&s.payment_instructions)
    .bind(&s.bank_account_name)
    .bind(&s.bank_account_number)
    .bind(&s.invoice_accent_color)
    .bind(s.default_deductible_percentage)
    .fetch_one(conn)
    .await
}
