//! Businesses: creation (onboarding), profile, settings, logo, activity, export and deletion.

use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::{BusinessContext, Owner};
use crate::domain::business::{self, Address, BusinessType, GstBasis, ReportingBasis};
use crate::domain::gst_returns::GstPeriod;
use crate::domain::validation;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::audit::{self, AuditFilter, AuditLogRow};
use crate::repositories::businesses::{self, BusinessFields, BusinessRow, SettingsRow};
use crate::services::storage::FileKind;
use crate::state::AppState;

const MAX_LOGO_BYTES: usize = 2 * 1024 * 1024;

/// Business profile as entered in onboarding and settings. Optional fields can be completed later.
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct BusinessInput {
    pub name: String,
    #[serde(default)]
    pub trading_name: String,
    #[serde(default = "default_business_type")]
    pub business_type: BusinessType,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub address: Address,
    #[serde(default)]
    pub nzbn: Option<String>,
    #[serde(default)]
    pub gst_registered: bool,
    #[serde(default)]
    pub gst_number: Option<String>,
    #[serde(default)]
    pub gst_registered_from: Option<NaiveDate>,
    #[serde(default = "default_gst_basis")]
    pub gst_basis: GstBasis,
    /// How often GST returns are filed (default two-monthly).
    #[serde(default = "default_gst_period")]
    pub gst_period: GstPeriod,
    /// A month a GST return period ends in, 1–12 (default 3, March).
    #[serde(default = "default_gst_period_end_month")]
    pub gst_period_end_month: i16,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default = "default_fy_month")]
    pub financial_year_start_month: i16,
    #[serde(default = "default_fy_day")]
    pub financial_year_start_day: i16,
    #[serde(default = "default_reporting_basis")]
    pub reporting_basis: ReportingBasis,
    /// Marks onboarding as finished (the user reached the end of the setup flow).
    #[serde(default)]
    pub complete_onboarding: bool,
}

fn default_business_type() -> BusinessType {
    BusinessType::SoleTrader
}
fn default_gst_period() -> GstPeriod {
    GstPeriod::TwoMonthly
}
fn default_gst_period_end_month() -> i16 {
    3
}
fn default_gst_basis() -> GstBasis {
    GstBasis::Payments
}
fn default_currency() -> String {
    "NZD".into()
}
fn default_fy_month() -> i16 {
    4
}
fn default_fy_day() -> i16 {
    1
}
fn default_reporting_basis() -> ReportingBasis {
    ReportingBasis::Accrual
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FinancialYear {
    pub start: NaiveDate,
    pub end: NaiveDate,
    /// e.g. `FY2027` for the year ending 31 March 2027.
    pub label: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Business {
    pub id: Uuid,
    pub name: String,
    pub trading_name: String,
    pub business_type: BusinessType,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Address,
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
    pub has_logo: bool,
    pub onboarding_completed: bool,
    pub current_financial_year: FinancialYear,
    /// Today's date in the business's time zone.
    pub today: NaiveDate,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BusinessSummary {
    pub id: Uuid,
    pub name: String,
    pub currency: String,
    pub has_logo: bool,
    pub onboarding_completed: bool,
}

/// Invoice, quote and expense defaults for a business.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BusinessSettings {
    pub invoice_prefix: String,
    pub next_invoice_number: i32,
    pub quote_prefix: String,
    pub next_quote_number: i32,
    /// Digits in document numbers (4 → INV-0001).
    pub number_padding: i16,
    pub default_payment_terms_days: i32,
    pub default_quote_validity_days: i32,
    pub default_invoice_notes: String,
    pub default_invoice_footer: String,
    pub payment_instructions: String,
    pub bank_account_name: String,
    pub bank_account_number: String,
    /// `#rrggbb`, or empty for the product's brand colour.
    pub invoice_accent_color: String,
    #[schema(value_type = String, example = "100")]
    pub default_deductible_percentage: Decimal,
}

impl From<SettingsRow> for BusinessSettings {
    fn from(s: SettingsRow) -> Self {
        Self {
            invoice_prefix: s.invoice_prefix,
            next_invoice_number: s.next_invoice_number,
            quote_prefix: s.quote_prefix,
            next_quote_number: s.next_quote_number,
            number_padding: s.number_padding,
            default_payment_terms_days: s.default_payment_terms_days,
            default_quote_validity_days: s.default_quote_validity_days,
            default_invoice_notes: s.default_invoice_notes,
            default_invoice_footer: s.default_invoice_footer,
            payment_instructions: s.payment_instructions,
            bank_account_name: s.bank_account_name,
            bank_account_number: s.bank_account_number,
            invoice_accent_color: s.invoice_accent_color,
            default_deductible_percentage: s.default_deductible_percentage,
        }
    }
}

/// Today's date where the business operates.
pub fn business_today(timezone: &str) -> NaiveDate {
    let tz: Tz = timezone.parse().unwrap_or(chrono_tz::Pacific::Auckland);
    Utc::now().with_timezone(&tz).date_naive()
}

fn to_business(row: BusinessRow) -> Business {
    let today = business_today(&row.timezone);
    let (start, end) = business::financial_year(
        today,
        row.financial_year_start_month as u32,
        row.financial_year_start_day as u32,
    );
    Business {
        id: row.id,
        address: serde_json::from_value(row.address).unwrap_or_default(),
        has_logo: row.logo_object_key.is_some(),
        onboarding_completed: row.onboarding_completed_at.is_some(),
        current_financial_year: FinancialYear {
            start,
            end,
            label: business::financial_year_label(end),
        },
        today,
        name: row.name,
        trading_name: row.trading_name,
        business_type: row.business_type,
        email: row.email,
        phone: row.phone,
        nzbn: row.nzbn,
        gst_registered: row.gst_registered,
        gst_number: row.gst_number,
        gst_registered_from: row.gst_registered_from,
        gst_basis: row.gst_basis,
        gst_period: row.gst_period,
        gst_period_end_month: row.gst_period_end_month,
        currency: row.currency,
        locale: row.locale,
        timezone: row.timezone,
        financial_year_start_month: row.financial_year_start_month,
        financial_year_start_day: row.financial_year_start_day,
        reporting_basis: row.reporting_basis,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn optional(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

/// Validates and normalises the profile into storable fields.
fn validate(input: BusinessInput) -> AppResult<BusinessFields> {
    let mut errors = FieldErrors::new();
    let name = input.name.trim().to_owned();
    errors.check(!name.is_empty(), "name", "Enter your business name.");
    errors.check(
        name.chars().count() <= 200,
        "name",
        "Use 200 characters or fewer.",
    );
    let trading_name = input.trading_name.trim().to_owned();
    errors.check(
        trading_name.chars().count() <= 200,
        "trading_name",
        "Use 200 characters or fewer.",
    );

    let email = optional(input.email).map(|e| validation::normalize_email(&e));
    if let Some(email) = &email {
        errors.check(
            validation::is_valid_email(email),
            "email",
            "Enter a valid email address.",
        );
    }
    errors.check(
        (1..=12).contains(&input.gst_period_end_month),
        "gst_period_end_month",
        "Choose a month.",
    );
    let phone = optional(input.phone);
    if let Some(phone) = &phone {
        let ok = phone.chars().count() <= 40
            && phone
                .chars()
                .all(|c| c.is_ascii_digit() || " +()-.".contains(c));
        errors.check(
            ok,
            "phone",
            "Enter a phone number using digits, spaces and + ( ) - only.",
        );
    }
    let address = input.address.trimmed();
    for (field, value) in address.fields() {
        errors.check(
            value.chars().count() <= Address::MAX_FIELD,
            &format!("address.{field}"),
            "Use 120 characters or fewer.",
        );
    }
    let nzbn = optional(input.nzbn).map(|n| validation::digits_only(&n));
    if let Some(nzbn) = &nzbn {
        errors.check(
            validation::is_valid_nzbn(nzbn),
            "nzbn",
            "Enter a valid 13-digit NZBN.",
        );
    }
    let gst_number = if input.gst_registered {
        optional(input.gst_number).map(|n| validation::digits_only(&n))
    } else {
        None
    };
    if let Some(number) = &gst_number {
        errors.check(
            validation::is_valid_ird_number(number),
            "gst_number",
            "Enter a valid GST number (8 or 9 digits).",
        );
    }
    let currency = input.currency.trim().to_uppercase();
    errors.check(
        validation::is_supported_currency(&currency),
        "currency",
        "Choose a supported currency.",
    );
    errors.check(
        (1..=12).contains(&input.financial_year_start_month),
        "financial_year_start_month",
        "Choose a month.",
    );
    errors.check(
        (1..=28).contains(&input.financial_year_start_day),
        "financial_year_start_day",
        "Choose a day between 1 and 28.",
    );
    errors.into_result()?;

    Ok(BusinessFields {
        name,
        trading_name,
        business_type: input.business_type,
        email,
        phone,
        address: serde_json::to_value(address).map_err(AppError::internal)?,
        nzbn,
        gst_registered: input.gst_registered,
        gst_number,
        gst_registered_from: if input.gst_registered {
            input.gst_registered_from
        } else {
            None
        },
        gst_basis: input.gst_basis,
        gst_period: input.gst_period,
        gst_period_end_month: input.gst_period_end_month,
        currency,
        financial_year_start_month: input.financial_year_start_month,
        financial_year_start_day: input.financial_year_start_day,
        reporting_basis: input.reporting_basis,
    })
}

pub async fn list(state: &AppState, auth: &Owner) -> AppResult<Vec<BusinessSummary>> {
    let mut tx = state.db.begin_user(auth.user_id).await?;
    let rows = businesses::list_for_user(&mut tx, auth.user_id).await?;
    tx.commit().await?;
    Ok(rows
        .into_iter()
        .map(|m| BusinessSummary {
            id: m.business_id,
            name: m.name,
            currency: m.currency,
            has_logo: m.logo_object_key.is_some(),
            onboarding_completed: m.onboarding_completed_at.is_some(),
        })
        .collect())
}

pub async fn create(state: &AppState, auth: &Owner, input: BusinessInput) -> AppResult<Business> {
    let complete = input.complete_onboarding;
    let fields = validate(input)?;
    let id = Uuid::now_v7();
    let mut tx = state.db.begin_business(auth.user_id, id).await?;
    let row = businesses::insert(&mut tx, id, auth.user_id, &fields, complete).await?;
    crate::services::hooks::on_business_created(&mut tx, id).await?;
    audit::record(
        &mut tx,
        &auth.actor(),
        audit::Entry {
            business_id: Some(id),
            action: "business.created",
            entity_type: "business",
            entity_id: Some(id),
            summary: format!("Business “{}” created", row.name),
            changes: serde_json::json!({}),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(to_business(row))
}

pub async fn get(state: &AppState, ctx: &BusinessContext) -> AppResult<Business> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = businesses::find(&mut tx, ctx.business_id)
        .await?
        .ok_or(AppError::NotFound("Business"))?;
    tx.commit().await?;
    Ok(to_business(row))
}

/// Field-level differences between two JSON objects, for the audit log.
pub fn diff(before: &serde_json::Value, after: &serde_json::Value) -> serde_json::Value {
    let mut changes = serde_json::Map::new();
    if let (Some(before), Some(after)) = (before.as_object(), after.as_object()) {
        for (key, new) in after {
            let old = before.get(key).unwrap_or(&serde_json::Value::Null);
            if old != new {
                changes.insert(key.clone(), serde_json::json!({ "from": old, "to": new }));
            }
        }
    }
    serde_json::Value::Object(changes)
}

fn fields_json(fields: &BusinessFields) -> serde_json::Value {
    serde_json::json!({
        "name": fields.name, "trading_name": fields.trading_name, "business_type": fields.business_type,
        "email": fields.email, "phone": fields.phone, "address": fields.address, "nzbn": fields.nzbn,
        "gst_registered": fields.gst_registered, "gst_number": fields.gst_number,
        "gst_registered_from": fields.gst_registered_from, "gst_basis": fields.gst_basis,
        "gst_period": fields.gst_period, "gst_period_end_month": fields.gst_period_end_month, "currency": fields.currency,
        "financial_year_start_month": fields.financial_year_start_month,
        "financial_year_start_day": fields.financial_year_start_day, "reporting_basis": fields.reporting_basis,
    })
}

pub async fn update(
    state: &AppState,
    ctx: &BusinessContext,
    input: BusinessInput,
) -> AppResult<Business> {
    let complete = input.complete_onboarding;
    let fields = validate(input)?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let before = businesses::find(&mut tx, ctx.business_id)
        .await?
        .ok_or(AppError::NotFound("Business"))?;
    let before_fields = BusinessFields::from_row(&before);
    if before_fields.currency != fields.currency
        && crate::services::hooks::has_financial_records(&mut tx, ctx.business_id).await?
    {
        return Err(AppError::validation(
            "currency",
            "The currency can’t change once financial records exist.",
        ));
    }
    let row = businesses::update(&mut tx, ctx.business_id, &fields, complete).await?;
    let changes = diff(&fields_json(&before_fields), &fields_json(&fields));
    if changes.as_object().is_some_and(|c| !c.is_empty()) {
        audit::record(
            &mut tx,
            &ctx.actor(),
            audit::Entry {
                business_id: Some(ctx.business_id),
                action: "business.updated",
                entity_type: "business",
                entity_id: Some(ctx.business_id),
                summary: "Business details updated".into(),
                changes,
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(to_business(row))
}

pub async fn settings(state: &AppState, ctx: &BusinessContext) -> AppResult<BusinessSettings> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = businesses::settings(&mut tx, ctx.business_id).await?;
    tx.commit().await?;
    Ok(row.into())
}

pub async fn update_settings(
    state: &AppState,
    ctx: &BusinessContext,
    input: BusinessSettings,
) -> AppResult<BusinessSettings> {
    let mut errors = FieldErrors::new();
    let clean = |errors: &mut FieldErrors, field: &str, value: &str, max: usize| {
        match validation::clean_text(value, max) {
            Ok(v) => v,
            Err(message) => {
                errors.add(field, message);
                String::new()
            }
        }
    };
    let next = SettingsRow {
        invoice_prefix: clean(&mut errors, "invoice_prefix", &input.invoice_prefix, 20),
        quote_prefix: clean(&mut errors, "quote_prefix", &input.quote_prefix, 20),
        default_invoice_notes: clean(
            &mut errors,
            "default_invoice_notes",
            &input.default_invoice_notes,
            2000,
        ),
        default_invoice_footer: clean(
            &mut errors,
            "default_invoice_footer",
            &input.default_invoice_footer,
            2000,
        ),
        payment_instructions: clean(
            &mut errors,
            "payment_instructions",
            &input.payment_instructions,
            2000,
        ),
        bank_account_name: clean(
            &mut errors,
            "bank_account_name",
            &input.bank_account_name,
            120,
        ),
        bank_account_number: clean(
            &mut errors,
            "bank_account_number",
            &input.bank_account_number,
            40,
        ),
        invoice_accent_color: input.invoice_accent_color.trim().to_owned(),
        next_invoice_number: input.next_invoice_number,
        next_quote_number: input.next_quote_number,
        number_padding: input.number_padding,
        default_payment_terms_days: input.default_payment_terms_days,
        default_quote_validity_days: input.default_quote_validity_days,
        default_deductible_percentage: input.default_deductible_percentage,
    };
    errors.check(
        next.next_invoice_number >= 1,
        "next_invoice_number",
        "Use a number of 1 or more.",
    );
    errors.check(
        next.next_quote_number >= 1,
        "next_quote_number",
        "Use a number of 1 or more.",
    );
    errors.check(
        (1..=10).contains(&next.number_padding),
        "number_padding",
        "Use between 1 and 10 digits.",
    );
    errors.check(
        (0..=365).contains(&next.default_payment_terms_days),
        "default_payment_terms_days",
        "Use 0 to 365 days.",
    );
    errors.check(
        (1..=365).contains(&next.default_quote_validity_days),
        "default_quote_validity_days",
        "Use 1 to 365 days.",
    );
    errors.check(
        next.invoice_accent_color.is_empty()
            || validation::is_hex_colour(&next.invoice_accent_color),
        "invoice_accent_color",
        "Use a colour like #13795b.",
    );
    errors.check(
        next.default_deductible_percentage >= Decimal::ZERO
            && next.default_deductible_percentage <= Decimal::ONE_HUNDRED,
        "default_deductible_percentage",
        "Use a percentage between 0 and 100.",
    );
    errors.into_result()?;

    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let before = BusinessSettings::from(businesses::settings(&mut tx, ctx.business_id).await?);
    // Lowering a counter could reissue numbers already used; the invoicing module checks for clashes.
    crate::services::hooks::check_document_numbering(&mut tx, ctx.business_id, &next).await?;
    let row = businesses::update_settings(&mut tx, ctx.business_id, &next).await?;
    let after = BusinessSettings::from(row);
    let changes = diff(
        &serde_json::to_value(&before).map_err(AppError::internal)?,
        &serde_json::to_value(&after).map_err(AppError::internal)?,
    );
    if changes.as_object().is_some_and(|c| !c.is_empty()) {
        audit::record(
            &mut tx,
            &ctx.actor(),
            audit::Entry {
                business_id: Some(ctx.business_id),
                action: "business.settings_updated",
                entity_type: "business",
                entity_id: Some(ctx.business_id),
                summary: "Invoice and expense settings updated".into(),
                changes,
            },
        )
        .await?;
    }
    tx.commit().await?;
    Ok(after)
}

pub async fn upload_logo(
    state: &AppState,
    ctx: &BusinessContext,
    bytes: Vec<u8>,
) -> AppResult<Business> {
    if bytes.len() > MAX_LOGO_BYTES {
        return Err(AppError::PayloadTooLarge("Logos can be up to 2 MB.".into()));
    }
    let kind = FileKind::sniff(&bytes)
        .filter(|k| k.is_image())
        .ok_or_else(|| AppError::validation("file", "Upload a PNG, JPEG or WebP image."))?;
    let key = format!(
        "businesses/{}/logo/{}.{}",
        ctx.business_id,
        Uuid::now_v7(),
        kind.extension()
    );
    state.storage.put(&key, bytes).await?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let previous = businesses::set_logo(&mut tx, ctx.business_id, Some(&key)).await?;
    audit::record(
        &mut tx,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action: "business.logo_updated",
            entity_type: "business",
            entity_id: Some(ctx.business_id),
            summary: "Logo uploaded".into(),
            changes: serde_json::json!({}),
        },
    )
    .await?;
    let row = businesses::find(&mut tx, ctx.business_id)
        .await?
        .ok_or(AppError::NotFound("Business"))?;
    tx.commit().await?;
    if let Some(previous) = previous
        && let Err(error) = state.storage.delete(&previous).await
    {
        tracing::warn!(%error, "could not delete the previous logo");
    }
    Ok(to_business(row))
}

pub async fn remove_logo(state: &AppState, ctx: &BusinessContext) -> AppResult<()> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let previous = businesses::set_logo(&mut tx, ctx.business_id, None).await?;
    tx.commit().await?;
    if let Some(previous) = previous {
        state.storage.delete(&previous).await?;
    }
    Ok(())
}

/// The logo's bytes and content type, if one is set.
pub async fn logo(state: &AppState, ctx: &BusinessContext) -> AppResult<(Vec<u8>, &'static str)> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = businesses::find(&mut tx, ctx.business_id)
        .await?
        .ok_or(AppError::NotFound("Business"))?;
    tx.commit().await?;
    let key = row.logo_object_key.ok_or(AppError::NotFound("Logo"))?;
    let kind = FileKind::from_extension(&key).ok_or(AppError::NotFound("Logo"))?;
    let bytes = state
        .storage
        .get(&key)
        .await?
        .ok_or(AppError::NotFound("Logo"))?;
    Ok((bytes, kind.content_type()))
}

pub async fn activity(
    state: &AppState,
    ctx: &BusinessContext,
    filter: AuditFilter,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<AuditLogRow>, i64)> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let result = audit::list_for_business(&mut tx, &filter, limit, offset).await?;
    tx.commit().await?;
    Ok(result)
}

/// Deletes the business (owner only) after the user retypes its name.
pub async fn delete(
    state: &AppState,
    ctx: &BusinessContext,
    confirmation_name: &str,
) -> AppResult<()> {
    let business = get(state, ctx).await?;
    if confirmation_name.trim() != business.name {
        return Err(AppError::validation(
            "confirmation_name",
            "Type the business name exactly to confirm.",
        ));
    }
    delete_business_data(state, ctx.user_id(), ctx.business_id).await
}

/// Removes a business, its records (cascade) and its stored files.
pub async fn delete_business_data(
    state: &AppState,
    user_id: Uuid,
    business_id: Uuid,
) -> AppResult<()> {
    let mut tx = state.db.begin_business(user_id, business_id).await?;
    let keys = crate::services::hooks::stored_files(&mut tx, business_id).await?;
    businesses::delete(&mut tx, business_id).await?;
    tx.commit().await?;
    for key in keys {
        if let Err(error) = state.storage.delete(&key).await {
            tracing::warn!(%error, key, "could not delete a stored file of a deleted business");
        }
    }
    tracing::info!(%business_id, "business deleted");
    Ok(())
}

/// Everything stored for one business, for the account export.
pub async fn export_business(
    state: &AppState,
    user_id: Uuid,
    business_id: Uuid,
) -> AppResult<serde_json::Value> {
    let mut tx = state.db.begin_business(user_id, business_id).await?;
    if !businesses::is_member(&mut tx, business_id, user_id).await? {
        return Err(AppError::NotFound("Business"));
    }
    let row = businesses::find(&mut tx, business_id)
        .await?
        .ok_or(AppError::NotFound("Business"))?;
    let settings = BusinessSettings::from(businesses::settings(&mut tx, business_id).await?);
    let (activity, _) = audit::list_for_business(
        &mut tx,
        &AuditFilter {
            entity_type: None,
            entity_id: None,
        },
        10_000,
        0,
    )
    .await?;
    let records = crate::services::hooks::export_records(&mut tx, business_id).await?;
    tx.commit().await?;
    Ok(serde_json::json!({
        "business": to_business(row),
        "settings": settings,
        "records": records,
        "activity": activity,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> BusinessInput {
        serde_json::from_value(serde_json::json!({ "name": "  Kowhai Design  " }))
            .expect("minimal input deserialises")
    }

    #[test]
    fn minimal_input_gets_nz_defaults() {
        let fields = validate(input()).expect("valid");
        assert_eq!(fields.name, "Kowhai Design");
        assert_eq!(fields.currency, "NZD");
        assert_eq!(fields.financial_year_start_month, 4);
        assert_eq!(fields.business_type, BusinessType::SoleTrader);
        assert!(!fields.gst_registered);
    }

    #[test]
    fn identifiers_are_checked_and_normalised() {
        let mut bad = input();
        bad.nzbn = Some("123".into());
        bad.gst_registered = true;
        bad.gst_number = Some("136-410-133".into());
        let Err(AppError::Validation(errors)) = validate(bad) else {
            panic!("expected validation errors")
        };
        assert!(errors.0.contains_key("nzbn"));
        assert!(errors.0.contains_key("gst_number"));

        let mut good = input();
        good.gst_registered = true;
        good.gst_number = Some("136-410-132".into());
        assert_eq!(
            validate(good).expect("valid").gst_number.as_deref(),
            Some("136410132")
        );
    }

    #[test]
    fn gst_details_are_dropped_when_not_registered() {
        let mut value = input();
        value.gst_number = Some("136410132".into());
        value.gst_registered_from = NaiveDate::from_ymd_opt(2024, 4, 1);
        let fields = validate(value).expect("valid");
        assert!(fields.gst_number.is_none());
        assert!(fields.gst_registered_from.is_none());
    }

    #[test]
    fn diff_reports_changed_fields_only() {
        let changes = diff(
            &serde_json::json!({"a": 1, "b": 2}),
            &serde_json::json!({"a": 1, "b": 3}),
        );
        assert_eq!(changes, serde_json::json!({"b": {"from": 2, "to": 3}}));
    }
}
