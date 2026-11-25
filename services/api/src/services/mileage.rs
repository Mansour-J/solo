//! Mileage: vehicles, mileage rates with dated versions, and trips.
//!
//! A trip's claim is its business distance × the rate per kilometre in force on the trip date (or a
//! rate entered for the trip). The trip keeps the rate it used, so editing a rate never changes past
//! claims. Each claim posts to the ledger as Dr Mileage claims · Cr Owner funds introduced.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::context::BusinessContext;
use crate::domain::business::financial_year;
use crate::domain::ledger;
use crate::domain::mileage::{
    self as calc, DistanceError, MAX_ODOMETER_KM, MAX_RATE_PER_KM, RateVersion, Tiers,
    business_distance, claim_amount, km_display, rate_on, tier2_business_km, tiered_claim,
    trip_distance,
};
use crate::domain::money::Currency;
use crate::error::{AppError, AppResult, FieldErrors};
use crate::repositories::audit;
use crate::repositories::businesses::BusinessRow;
use crate::repositories::ledger::{self as ledger_repo, JournalEntryView};
use crate::repositories::mileage::{
    self as repo, FrequentTrip, MonthMileage, RateRow, RateVersionRow, TripFields, TripFilter,
    TripRow, TripTotals, VehicleFields, VehicleMileage, VehicleRow,
};
use crate::services::businesses::business_today;
use crate::services::export::{CsvWriter, number_cell, text_cell};
use crate::services::invoices::{clean, load_business};
use crate::services::ledger::{self as ledger_service, Context, Desired, SOURCE_MILEAGE};
use crate::state::AppState;

fn decimal_100() -> Decimal {
    Decimal::ONE_HUNDRED
}

fn format_date(date: NaiveDate) -> String {
    date.format("%d/%m/%Y").to_string()
}

async fn audit_record(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    action: &str,
    entity_type: &str,
    id: Uuid,
    summary: String,
    changes: serde_json::Value,
) -> AppResult<()> {
    audit::record(
        conn,
        &ctx.actor(),
        audit::Entry {
            business_id: Some(ctx.business_id),
            action,
            entity_type,
            entity_id: Some(id),
            summary,
            changes,
        },
    )
    .await
    .map_err(Into::into)
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ArchiveInput {
    pub archived: bool,
}

// ─── Rates ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct NewRateInput {
    pub name: String,
    #[serde(default)]
    pub notes: String,
    /// First day the rate applies.
    pub effective_from: NaiveDate,
    /// Amount per kilometre in the business's currency, e.g. "1.04".
    #[schema(value_type = String)]
    pub rate_per_km: Decimal,
    /// IRD's tier 1 limit in kilometres a year (e.g. "14000"); with `tier2_rate_per_km`, or neither.
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub tier1_limit_km: Option<Decimal>,
    /// The rate per kilometre after the tier 1 limit.
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub tier2_rate_per_km: Option<Decimal>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RateInput {
    pub name: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct RateVersionInput {
    pub effective_from: NaiveDate,
    #[schema(value_type = String)]
    pub rate_per_km: Decimal,
    /// IRD's tier 1 limit in kilometres a year (e.g. "14000"); with `tier2_rate_per_km`, or neither.
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub tier1_limit_km: Option<Decimal>,
    /// The rate per kilometre after the tier 1 limit.
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub tier2_rate_per_km: Option<Decimal>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MileageRate {
    pub id: Uuid,
    pub name: String,
    pub notes: String,
    pub archived: bool,
    /// Active vehicles using this rate by default.
    pub vehicle_count: i64,
    /// Trips that took their rate from this one.
    pub trip_count: i64,
    /// Every version, oldest first.
    pub versions: Vec<RateVersionRow>,
    /// The version in force today, if any.
    pub current: Option<RateVersionRow>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn to_versions(rows: &[RateVersionRow]) -> Vec<RateVersion> {
    rows.iter()
        .map(|v| RateVersion {
            effective_from: v.effective_from,
            rate_per_km: v.rate_per_km,
            tiers: v.tier1_limit_km.zip(v.tier2_rate_per_km).map(
                |(tier1_limit_km, tier2_rate_per_km)| Tiers {
                    tier1_limit_km,
                    tier2_rate_per_km,
                },
            ),
        })
        .collect()
}

fn mileage_rate(row: RateRow, versions: Vec<RateVersionRow>, today: NaiveDate) -> MileageRate {
    let current = rate_on(&to_versions(&versions), today).and_then(|c| {
        versions
            .iter()
            .find(|v| v.effective_from == c.effective_from)
            .cloned()
    });
    MileageRate {
        id: row.id,
        name: row.name,
        notes: row.notes,
        archived: row.archived,
        vehicle_count: row.vehicle_count,
        trip_count: row.trip_count,
        versions,
        current,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

async fn load_rate(conn: &mut PgConnection, id: Uuid, today: NaiveDate) -> AppResult<MileageRate> {
    let row = repo::find_rate(conn, id)
        .await?
        .ok_or(AppError::NotFound("Mileage rate"))?;
    let versions = repo::rate_versions(conn, Some(id)).await?;
    Ok(mileage_rate(row, versions, today))
}

/// A rate per kilometre rounded to 4 decimal places, or a field error.
fn check_rate(rate: Decimal, field: &str, errors: &mut FieldErrors) -> Decimal {
    let rate = rate.round_dp(4);
    errors.check(
        rate >= Decimal::ZERO && rate <= MAX_RATE_PER_KM,
        field,
        "Enter a rate between 0 and 100 per kilometre.",
    );
    rate
}

/// IRD's tiers from the two optional fields: both, or neither.
fn check_tiers(
    limit: Option<Decimal>,
    tier2_rate: Option<Decimal>,
    prefix: &str,
    errors: &mut FieldErrors,
) -> Option<Tiers> {
    let limit_field = format!("{prefix}tier1_limit_km");
    let rate_field = format!("{prefix}tier2_rate_per_km");
    match (limit, tier2_rate) {
        (None, None) => None,
        (Some(_), None) => {
            errors.add(
                &rate_field,
                "Enter the tier 2 rate, or clear the tier 1 limit.",
            );
            None
        }
        (None, Some(_)) => {
            errors.add(
                &limit_field,
                "Enter the tier 1 limit, or clear the tier 2 rate.",
            );
            None
        }
        (Some(limit), Some(rate)) => {
            let limit = calc::round_km(limit);
            errors.check(
                limit > Decimal::ZERO && limit <= MAX_ODOMETER_KM,
                &limit_field,
                "Enter a limit of more than 0 km.",
            );
            let rate = check_rate(rate, &rate_field, errors);
            Some(Tiers {
                tier1_limit_km: limit,
                tier2_rate_per_km: rate,
            })
        }
    }
}

fn rate_name(name: &str, notes: &str, errors: &mut FieldErrors) -> (String, String) {
    let name = clean(name, 80, "name", errors);
    errors.check(!name.is_empty(), "name", "Enter a name for the rate.");
    let notes = clean(notes, 1000, "notes", errors);
    (name, notes)
}

pub async fn list_rates(state: &AppState, ctx: &BusinessContext) -> AppResult<Vec<MileageRate>> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let today = business_today(&business.timezone);
    let rows = repo::list_rates(&mut tx).await?;
    let versions = repo::rate_versions(&mut tx, None).await?;
    tx.commit().await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let own = versions
                .iter()
                .filter(|v| v.rate_id == row.id)
                .cloned()
                .collect();
            mileage_rate(row, own, today)
        })
        .collect())
}

/// Creates a rate with its first version (inside the caller's transaction).
async fn insert_rate(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    input: &NewRateInput,
    field_prefix: &str,
) -> AppResult<Uuid> {
    let field = |name: &str| format!("{field_prefix}{name}");
    let mut errors = FieldErrors::new();
    let name = clean(&input.name, 80, &field("name"), &mut errors);
    errors.check(
        !name.is_empty(),
        &field("name"),
        "Enter a name for the rate.",
    );
    let notes = clean(&input.notes, 1000, &field("notes"), &mut errors);
    let rate = check_rate(input.rate_per_km, &field("rate_per_km"), &mut errors);
    let tiers = check_tiers(
        input.tier1_limit_km,
        input.tier2_rate_per_km,
        field_prefix,
        &mut errors,
    );
    errors.into_result()?;
    if repo::rate_name_taken(conn, &name, None).await? {
        return Err(AppError::validation(
            &field("name"),
            "A rate with this name already exists.",
        ));
    }
    let id = repo::insert_rate(conn, ctx.business_id, &name, &notes).await?;
    repo::insert_version(conn, ctx.business_id, id, input.effective_from, rate, tiers).await?;
    audit_record(
        conn,
        ctx,
        "mileage_rate.created",
        "mileage_rate",
        id,
        format!("Mileage rate “{name}” added"),
        serde_json::json!({
            "effective_from": input.effective_from,
            "rate_per_km": rate,
            "tier1_limit_km": tiers.map(|t| t.tier1_limit_km),
            "tier2_rate_per_km": tiers.map(|t| t.tier2_rate_per_km),
        }),
    )
    .await?;
    Ok(id)
}

pub async fn create_rate(
    state: &AppState,
    ctx: &BusinessContext,
    input: NewRateInput,
) -> AppResult<MileageRate> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let id = insert_rate(&mut tx, ctx, &input, "").await?;
    let rate = load_rate(&mut tx, id, business_today(&business.timezone)).await?;
    tx.commit().await?;
    Ok(rate)
}

pub async fn update_rate(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: RateInput,
) -> AppResult<MileageRate> {
    let mut errors = FieldErrors::new();
    let (name, notes) = rate_name(&input.name, &input.notes, &mut errors);
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let before = repo::find_rate(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Mileage rate"))?;
    if !before.archived && repo::rate_name_taken(&mut tx, &name, Some(id)).await? {
        return Err(AppError::validation(
            "name",
            "A rate with this name already exists.",
        ));
    }
    repo::update_rate(&mut tx, id, &name, &notes).await?;
    audit_record(
        &mut tx,
        ctx,
        "mileage_rate.updated",
        "mileage_rate",
        id,
        format!("Mileage rate “{name}” updated"),
        serde_json::json!({ "name": { "from": before.name, "to": name } }),
    )
    .await?;
    let rate = load_rate(&mut tx, id, business_today(&business.timezone)).await?;
    tx.commit().await?;
    Ok(rate)
}

pub async fn archive_rate(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    archived: bool,
) -> AppResult<MileageRate> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let row = repo::find_rate(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Mileage rate"))?;
    if archived && row.vehicle_count > 0 {
        return Err(AppError::Conflict(
            "Vehicles use this rate by default. Choose another rate for them first.".into(),
        ));
    }
    if !archived && row.archived && repo::rate_name_taken(&mut tx, &row.name, Some(id)).await? {
        return Err(AppError::Conflict(
            "Another rate has this name. Rename one of them first.".into(),
        ));
    }
    repo::set_rate_archived(&mut tx, id, archived).await?;
    audit_record(
        &mut tx,
        ctx,
        if archived {
            "mileage_rate.archived"
        } else {
            "mileage_rate.restored"
        },
        "mileage_rate",
        id,
        format!(
            "Mileage rate “{}” {}",
            row.name,
            if archived { "archived" } else { "restored" }
        ),
        serde_json::json!({}),
    )
    .await?;
    let rate = load_rate(&mut tx, id, business_today(&business.timezone)).await?;
    tx.commit().await?;
    Ok(rate)
}

pub async fn add_version(
    state: &AppState,
    ctx: &BusinessContext,
    rate_id: Uuid,
    input: RateVersionInput,
) -> AppResult<MileageRate> {
    let mut errors = FieldErrors::new();
    let rate = check_rate(input.rate_per_km, "rate_per_km", &mut errors);
    let tiers = check_tiers(
        input.tier1_limit_km,
        input.tier2_rate_per_km,
        "",
        &mut errors,
    );
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let row = repo::find_rate(&mut tx, rate_id)
        .await?
        .ok_or(AppError::NotFound("Mileage rate"))?;
    if repo::version_date_taken(&mut tx, rate_id, input.effective_from, None).await? {
        return Err(AppError::validation(
            "effective_from",
            "This rate already has a version from this date.",
        ));
    }
    let version_id = repo::insert_version(
        &mut tx,
        ctx.business_id,
        rate_id,
        input.effective_from,
        rate,
        tiers,
    )
    .await?;
    audit_record(
        &mut tx,
        ctx,
        "mileage_rate.version_added",
        "mileage_rate",
        rate_id,
        format!(
            "Mileage rate “{}”: {rate} per km from {}",
            row.name,
            format_date(input.effective_from)
        ),
        serde_json::json!({
            "version_id": version_id,
            "effective_from": input.effective_from,
            "rate_per_km": rate,
            "tier1_limit_km": tiers.map(|t| t.tier1_limit_km),
            "tier2_rate_per_km": tiers.map(|t| t.tier2_rate_per_km),
        }),
    )
    .await?;
    let result = load_rate(&mut tx, rate_id, business_today(&business.timezone)).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn update_version(
    state: &AppState,
    ctx: &BusinessContext,
    rate_id: Uuid,
    version_id: Uuid,
    input: RateVersionInput,
) -> AppResult<MileageRate> {
    let mut errors = FieldErrors::new();
    let rate = check_rate(input.rate_per_km, "rate_per_km", &mut errors);
    let tiers = check_tiers(
        input.tier1_limit_km,
        input.tier2_rate_per_km,
        "",
        &mut errors,
    );
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let row = repo::find_rate(&mut tx, rate_id)
        .await?
        .ok_or(AppError::NotFound("Mileage rate"))?;
    let before = repo::find_version(&mut tx, rate_id, version_id)
        .await?
        .ok_or(AppError::NotFound("Rate version"))?;
    if repo::version_date_taken(&mut tx, rate_id, input.effective_from, Some(version_id)).await? {
        return Err(AppError::validation(
            "effective_from",
            "This rate already has a version from this date.",
        ));
    }
    repo::update_version(&mut tx, version_id, input.effective_from, rate, tiers).await?;
    audit_record(
        &mut tx,
        ctx,
        "mileage_rate.version_updated",
        "mileage_rate",
        rate_id,
        format!("Mileage rate “{}” version changed", row.name),
        serde_json::json!({
            "version_id": version_id,
            "effective_from": { "from": before.effective_from, "to": input.effective_from },
            "rate_per_km": { "from": before.rate_per_km, "to": rate },
            "tier1_limit_km": { "from": before.tier1_limit_km, "to": tiers.map(|t| t.tier1_limit_km) },
            "tier2_rate_per_km": { "from": before.tier2_rate_per_km, "to": tiers.map(|t| t.tier2_rate_per_km) },
        }),
    )
    .await?;
    let result = load_rate(&mut tx, rate_id, business_today(&business.timezone)).await?;
    tx.commit().await?;
    Ok(result)
}

pub async fn delete_version(
    state: &AppState,
    ctx: &BusinessContext,
    rate_id: Uuid,
    version_id: Uuid,
) -> AppResult<MileageRate> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let row = repo::find_rate(&mut tx, rate_id)
        .await?
        .ok_or(AppError::NotFound("Mileage rate"))?;
    let version = repo::find_version(&mut tx, rate_id, version_id)
        .await?
        .ok_or(AppError::NotFound("Rate version"))?;
    if repo::rate_versions(&mut tx, Some(rate_id)).await?.len() <= 1 {
        return Err(AppError::Conflict(
            "A rate needs at least one version. Archive the rate instead.".into(),
        ));
    }
    repo::delete_version(&mut tx, version_id).await?;
    audit_record(
        &mut tx,
        ctx,
        "mileage_rate.version_deleted",
        "mileage_rate",
        rate_id,
        format!(
            "Mileage rate “{}”: version from {} removed",
            row.name,
            format_date(version.effective_from)
        ),
        serde_json::json!({ "version_id": version_id, "effective_from": version.effective_from, "rate_per_km": version.rate_per_km }),
    )
    .await?;
    let result = load_rate(&mut tx, rate_id, business_today(&business.timezone)).await?;
    tx.commit().await?;
    Ok(result)
}

// ─── Vehicles ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VehicleType {
    Car,
    Van,
    Ute,
    Motorcycle,
    Truck,
    Other,
}

impl VehicleType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Car => "car",
            Self::Van => "van",
            Self::Ute => "ute",
            Self::Motorcycle => "motorcycle",
            Self::Truck => "truck",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FuelType {
    Petrol,
    Diesel,
    Hybrid,
    PlugInHybrid,
    Electric,
    Other,
}

impl FuelType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Petrol => "petrol",
            Self::Diesel => "diesel",
            Self::Hybrid => "hybrid",
            Self::PlugInHybrid => "plug_in_hybrid",
            Self::Electric => "electric",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct VehicleInput {
    pub name: String,
    #[serde(default)]
    pub registration: String,
    #[serde(default)]
    pub make: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub year: Option<i16>,
    pub vehicle_type: VehicleType,
    pub fuel_type: FuelType,
    /// Rate for new trips with this vehicle.
    #[serde(default)]
    pub default_rate_id: Option<Uuid>,
    /// Creates a rate and makes it the default (instead of `default_rate_id`).
    #[serde(default)]
    pub new_rate: Option<NewRateInput>,
    /// Make this the vehicle new trips start with. The first vehicle always is.
    #[serde(default)]
    pub is_default: bool,
    #[serde(default)]
    pub notes: String,
}

async fn vehicle_fields(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    input: &VehicleInput,
    before: Option<&VehicleRow>,
) -> AppResult<VehicleFields> {
    let mut errors = FieldErrors::new();
    let name = clean(&input.name, 80, "name", &mut errors);
    errors.check(!name.is_empty(), "name", "Enter a name for the vehicle.");
    let registration = clean(&input.registration, 12, "registration", &mut errors).to_uppercase();
    let make = clean(&input.make, 60, "make", &mut errors);
    let model = clean(&input.model, 60, "model", &mut errors);
    let notes = clean(&input.notes, 1000, "notes", &mut errors);
    if let Some(year) = input.year {
        errors.check(
            (1900..=2100).contains(&year),
            "year",
            "Enter a year between 1900 and 2100.",
        );
    }
    errors.into_result()?;
    if repo::vehicle_name_taken(conn, &name, before.map(|v| v.id)).await? {
        return Err(AppError::validation(
            "name",
            "A vehicle with this name already exists.",
        ));
    }
    let default_rate_id = match (&input.new_rate, input.default_rate_id) {
        (Some(new_rate), _) => Some(insert_rate(conn, ctx, new_rate, "new_rate.").await?),
        (None, Some(rate_id)) => {
            let rate = repo::find_rate(conn, rate_id).await?;
            let unchanged = before.is_some_and(|v| v.default_rate_id == Some(rate_id));
            match rate {
                Some(rate) if !rate.archived || unchanged => Some(rate_id),
                _ => {
                    return Err(AppError::validation(
                        "default_rate_id",
                        "Choose a rate that isn’t archived.",
                    ));
                }
            }
        }
        (None, None) => None,
    };
    let first_vehicle = before.is_none() && repo::active_vehicle_count(conn).await? == 0;
    Ok(VehicleFields {
        name,
        registration,
        make,
        model,
        year: input.year,
        vehicle_type: input.vehicle_type.as_str().into(),
        fuel_type: input.fuel_type.as_str().into(),
        default_rate_id,
        is_default: input.is_default || first_vehicle,
        notes,
    })
}

async fn find_vehicle(conn: &mut PgConnection, id: Uuid) -> AppResult<VehicleRow> {
    repo::find_vehicle(conn, id)
        .await?
        .ok_or(AppError::NotFound("Vehicle"))
}

pub async fn list_vehicles(state: &AppState, ctx: &BusinessContext) -> AppResult<Vec<VehicleRow>> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let rows = repo::list_vehicles(&mut tx).await?;
    tx.commit().await?;
    Ok(rows)
}

pub async fn create_vehicle(
    state: &AppState,
    ctx: &BusinessContext,
    input: VehicleInput,
) -> AppResult<VehicleRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let fields = vehicle_fields(&mut tx, ctx, &input, None).await?;
    if fields.is_default {
        repo::clear_default_vehicle(&mut tx, None).await?;
    }
    let id = repo::insert_vehicle(&mut tx, ctx.business_id, &fields).await?;
    audit_record(
        &mut tx,
        ctx,
        "vehicle.created",
        "vehicle",
        id,
        format!("Vehicle “{}” added", fields.name),
        serde_json::json!({ "registration": fields.registration, "default_rate_id": fields.default_rate_id }),
    )
    .await?;
    let row = find_vehicle(&mut tx, id).await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn update_vehicle(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: VehicleInput,
) -> AppResult<VehicleRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let before = find_vehicle(&mut tx, id).await?;
    if before.archived {
        return Err(AppError::Conflict(
            "Restore this vehicle before changing it.".into(),
        ));
    }
    let fields = vehicle_fields(&mut tx, ctx, &input, Some(&before)).await?;
    if fields.is_default {
        repo::clear_default_vehicle(&mut tx, Some(id)).await?;
    }
    repo::update_vehicle(&mut tx, id, &fields).await?;
    audit_record(
        &mut tx,
        ctx,
        "vehicle.updated",
        "vehicle",
        id,
        format!("Vehicle “{}” updated", fields.name),
        serde_json::json!({
            "name": { "from": before.name, "to": fields.name },
            "default_rate_id": { "from": before.default_rate_id, "to": fields.default_rate_id },
            "is_default": { "from": before.is_default, "to": fields.is_default },
        }),
    )
    .await?;
    let row = find_vehicle(&mut tx, id).await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn archive_vehicle(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    archived: bool,
) -> AppResult<VehicleRow> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let row = find_vehicle(&mut tx, id).await?;
    if !archived && row.archived && repo::vehicle_name_taken(&mut tx, &row.name, Some(id)).await? {
        return Err(AppError::Conflict(
            "Another vehicle has this name. Rename one of them first.".into(),
        ));
    }
    repo::set_vehicle_archived(&mut tx, id, archived).await?;
    audit_record(
        &mut tx,
        ctx,
        if archived {
            "vehicle.archived"
        } else {
            "vehicle.restored"
        },
        "vehicle",
        id,
        format!(
            "Vehicle “{}” {}",
            row.name,
            if archived { "archived" } else { "restored" }
        ),
        serde_json::json!({}),
    )
    .await?;
    let row = find_vehicle(&mut tx, id).await?;
    tx.commit().await?;
    Ok(row)
}

// ─── Trips ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct TripInput {
    pub vehicle_id: Uuid,
    pub trip_date: NaiveDate,
    #[serde(default)]
    pub start_location: String,
    #[serde(default)]
    pub destination: String,
    /// Why the trip was made (required to save; optional for a calculation).
    #[serde(default)]
    pub purpose: String,
    /// Odometer readings in km: both, or neither (then `distance_km` is required).
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub start_odometer: Option<Decimal>,
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub end_odometer: Option<Decimal>,
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub distance_km: Option<Decimal>,
    /// There and back: `distance_km` is one way and the trip counts it twice (not with odometer
    /// readings, which already cover the whole trip).
    #[serde(default)]
    pub return_trip: bool,
    /// Share of the trip that was for the business (0–100). Defaults to 100.
    #[serde(default = "decimal_100")]
    #[schema(value_type = String)]
    pub business_percentage: Decimal,
    /// Rate to use; defaults to the vehicle's rate.
    #[serde(default)]
    pub rate_id: Option<Uuid>,
    /// A rate per km entered for this trip, instead of a mileage rate.
    #[serde(default)]
    #[schema(value_type = Option<String>)]
    pub rate_per_km: Option<Decimal>,
    #[serde(default)]
    pub notes: String,
}

/// How a trip's claim is worked out.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TripCalculation {
    #[schema(value_type = String)]
    pub distance_km: Decimal,
    #[schema(value_type = String)]
    pub business_distance_km: Decimal,
    #[schema(value_type = String)]
    pub personal_distance_km: Decimal,
    /// The rate used.
    #[schema(value_type = String)]
    pub rate_per_km: Decimal,
    pub rate_id: Option<Uuid>,
    pub rate_name: Option<String>,
    /// First day of the rate version used (None when the rate was entered for the trip).
    pub rate_effective_from: Option<NaiveDate>,
    pub rate_overridden: bool,
    /// IRD's tier 1 limit the rate had (None without tiers).
    #[schema(value_type = Option<String>)]
    pub tier1_limit_km: Option<Decimal>,
    /// The rate after the tier 1 limit.
    #[schema(value_type = Option<String>)]
    pub tier2_rate_per_km: Option<Decimal>,
    /// Of the business kilometres, those at the tier 2 rate.
    #[schema(value_type = String)]
    pub tier2_distance_km: Decimal,
    /// The vehicle's recorded travel earlier in the financial year (when the rate has tiers).
    #[schema(value_type = Option<String>)]
    pub travelled_before_km: Option<Decimal>,
    pub currency: String,
    /// Minor units.
    pub claim_amount: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Trip {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub vehicle_name: String,
    pub trip_date: NaiveDate,
    pub start_location: String,
    pub destination: String,
    pub purpose: String,
    #[schema(value_type = Option<String>)]
    pub start_odometer: Option<Decimal>,
    #[schema(value_type = Option<String>)]
    pub end_odometer: Option<Decimal>,
    #[schema(value_type = String)]
    pub distance_km: Decimal,
    #[schema(value_type = String)]
    pub business_percentage: Decimal,
    #[schema(value_type = String)]
    pub business_distance_km: Decimal,
    #[schema(value_type = String)]
    pub personal_distance_km: Decimal,
    pub rate_id: Option<Uuid>,
    pub rate_name: Option<String>,
    #[schema(value_type = String)]
    pub rate_per_km: Decimal,
    /// The rate was entered for this trip rather than taken from a mileage rate.
    pub rate_overridden: bool,
    /// There and back: `distance_km` is the whole trip (twice the distance entered).
    pub return_trip: bool,
    /// IRD's tier 1 limit the rate had (None without tiers).
    #[schema(value_type = Option<String>)]
    pub tier1_limit_km: Option<Decimal>,
    /// The rate after the tier 1 limit.
    #[schema(value_type = Option<String>)]
    pub tier2_rate_per_km: Option<Decimal>,
    /// Of the business kilometres, those at the tier 2 rate.
    #[schema(value_type = String)]
    pub tier2_distance_km: Decimal,
    pub currency: String,
    /// Minor units.
    pub claim_amount: i64,
    pub notes: String,
    pub voided: bool,
    pub void_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TripRow> for Trip {
    fn from(r: TripRow) -> Self {
        Self {
            id: r.id,
            vehicle_id: r.vehicle_id,
            vehicle_name: r.vehicle_name,
            trip_date: r.trip_date,
            start_location: r.start_location,
            destination: r.destination,
            purpose: r.purpose,
            start_odometer: r.start_odometer,
            end_odometer: r.end_odometer,
            distance_km: km_display(r.distance_km),
            business_percentage: r.business_percentage,
            business_distance_km: km_display(r.business_distance_km),
            personal_distance_km: km_display(r.personal_distance_km),
            rate_id: r.rate_id,
            rate_name: r.rate_name,
            rate_per_km: r.rate_per_km,
            rate_overridden: r.rate_overridden,
            tier1_limit_km: r.tier1_limit_km,
            tier2_rate_per_km: r.tier2_rate_per_km,
            tier2_distance_km: km_display(r.tier2_distance_km),
            return_trip: r.return_trip,
            currency: r.currency,
            claim_amount: r.claim_amount,
            notes: r.notes,
            voided: r.voided_at.is_some(),
            void_reason: r.void_reason,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TripDetail {
    #[serde(flatten)]
    pub trip: Trip,
    /// Ledger entries for this trip, including reversals.
    pub journal: Vec<JournalEntryView>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TripPage {
    pub items: Vec<Trip>,
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
    /// Totals of every matching trip (not only this page).
    pub totals: TripTotals,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MileageSummary {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub vehicle_id: Option<Uuid>,
    pub currency: String,
    pub totals: TripTotals,
    /// Each calendar month of the period, including months without trips.
    pub monthly: Vec<MonthMileage>,
    /// Vehicles with trips in the period.
    pub vehicles: Vec<VehicleMileage>,
}

fn currency_of(business: &BusinessRow) -> AppResult<Currency> {
    Currency::from_code(&business.currency).ok_or_else(|| {
        AppError::internal(anyhow::anyhow!(
            "unsupported currency {}",
            business.currency
        ))
    })
}

fn distance_error(error: DistanceError, errors: &mut FieldErrors) {
    match error {
        DistanceError::MissingOdometer { start_missing } => errors.add(
            if start_missing {
                "start_odometer"
            } else {
                "end_odometer"
            },
            "Enter both odometer readings, or only the distance.",
        ),
        DistanceError::OdometerOrder => errors.add(
            "end_odometer",
            "The end reading must be more than the start reading.",
        ),
        DistanceError::OdometerRange { start } => errors.add(
            if start {
                "start_odometer"
            } else {
                "end_odometer"
            },
            "Enter a reading between 0 and 9,999,999 km.",
        ),
        DistanceError::MissingDistance => errors.add(
            "distance_km",
            "Enter the distance, or the odometer readings.",
        ),
        DistanceError::DistanceRange => {
            errors.add("distance_km", "Enter a distance between 0.1 and 10,000 km.")
        }
        DistanceError::DistanceMismatch => {
            errors.add("distance_km", "This doesn’t match the odometer readings.")
        }
    }
}

/// The rate a trip uses: from a mileage rate version, or entered for the trip.
struct AppliedRate {
    rate_per_km: Decimal,
    tiers: Option<Tiers>,
    rate_id: Option<Uuid>,
    name: Option<String>,
    effective_from: Option<NaiveDate>,
}

/// Works out a trip's distance, rate and claim, adding field errors for anything invalid.
/// `existing` is the trip being edited: its vehicle and rate stay usable even if since archived.
async fn calculate_trip(
    conn: &mut PgConnection,
    business: &BusinessRow,
    input: &TripInput,
    existing: Option<&TripRow>,
    errors: &mut FieldErrors,
) -> AppResult<Option<TripCalculation>> {
    let vehicle = repo::find_vehicle(conn, input.vehicle_id).await?;
    match &vehicle {
        None => errors.add("vehicle_id", "Choose a vehicle."),
        Some(v) if v.archived && existing.is_none_or(|t| t.vehicle_id != v.id) => errors.add(
            "vehicle_id",
            "This vehicle is archived. Restore it or choose another.",
        ),
        Some(_) => {}
    }
    let distance = match trip_distance(input.start_odometer, input.end_odometer, input.distance_km)
    {
        Ok((calc::DistanceInput::Odometer { .. }, _)) if input.return_trip => {
            errors.add(
                "return_trip",
                "Odometer readings already cover the whole trip: enter the distance one way instead.",
            );
            None
        }
        Ok((_, km)) if input.return_trip => Some(km * Decimal::TWO),
        Ok((_, km)) => Some(km),
        Err(error) => {
            distance_error(error, errors);
            None
        }
    };
    let percentage = input.business_percentage.round_dp(2);
    let percentage_ok = percentage >= Decimal::ZERO && percentage <= Decimal::ONE_HUNDRED;
    errors.check(
        percentage_ok,
        "business_percentage",
        "Enter a percentage between 0 and 100.",
    );

    // The rate: entered for the trip, or the chosen (or vehicle's) mileage rate on the trip date.
    let mut rate: Option<AppliedRate> = None;
    if let Some(entered) = input.rate_per_km {
        let entered = check_rate(entered, "rate_per_km", errors);
        rate = Some(AppliedRate {
            rate_per_km: entered,
            tiers: None,
            rate_id: None,
            name: None,
            effective_from: None,
        });
    } else if let Some(rate_id) = input
        .rate_id
        .or_else(|| vehicle.as_ref().and_then(|v| v.default_rate_id))
    {
        match repo::find_rate(conn, rate_id).await? {
            None => errors.add("rate_id", "Choose a rate."),
            Some(row) if row.archived && existing.is_none_or(|t| t.rate_id != Some(rate_id)) => {
                errors.add("rate_id", "This rate is archived. Choose another rate.")
            }
            Some(row) => {
                let versions = repo::rate_versions(conn, Some(rate_id)).await?;
                match rate_on(&to_versions(&versions), input.trip_date) {
                    Some(v) => {
                        rate = Some(AppliedRate {
                            rate_per_km: v.rate_per_km,
                            tiers: v.tiers,
                            rate_id: Some(rate_id),
                            name: Some(row.name),
                            effective_from: Some(v.effective_from),
                        })
                    }
                    None => errors.add(
                        "rate_id",
                        format!(
                            "“{}” has no rate on {}. Add one from that date in Settings, or enter a rate for this trip.",
                            row.name,
                            format_date(input.trip_date)
                        ),
                    ),
                }
            }
        }
    } else if vehicle.is_some() {
        errors.add(
            "rate_id",
            "This vehicle has no default rate. Choose a rate, or enter one for this trip.",
        );
    }

    let (Some(distance), Some(rate)) = (distance, rate) else {
        return Ok(None);
    };
    if !percentage_ok || !errors.is_empty() {
        return Ok(None);
    }
    let business_km = business_distance(distance, percentage);
    let currency = currency_of(business)?;
    let (claim, tier2_km, travelled) = match rate.tiers {
        None => (
            claim_amount(business_km, rate.rate_per_km, currency),
            Decimal::ZERO,
            None,
        ),
        Some(tiers) => {
            let (year_start, _) = financial_year(
                input.trip_date,
                business.financial_year_start_month as u32,
                business.financial_year_start_day as u32,
            );
            let before = repo::travelled_before(
                conn,
                input.vehicle_id,
                year_start,
                input.trip_date,
                existing.map(|t| t.id),
            )
            .await?;
            let tier2 = tier2_business_km(distance, business_km, before, tiers.tier1_limit_km);
            (
                tiered_claim(
                    business_km,
                    tier2,
                    rate.rate_per_km,
                    tiers.tier2_rate_per_km,
                    currency,
                ),
                tier2,
                Some(km_display(before)),
            )
        }
    };
    let Some(claim) = claim else {
        errors.add("rate_per_km", "This claim is too large.");
        return Ok(None);
    };
    Ok(Some(TripCalculation {
        distance_km: km_display(distance),
        business_distance_km: km_display(business_km),
        personal_distance_km: km_display(distance - business_km),
        rate_per_km: rate.rate_per_km,
        rate_id: rate.rate_id,
        rate_name: rate.name,
        rate_effective_from: rate.effective_from,
        rate_overridden: input.rate_per_km.is_some(),
        tier1_limit_km: rate.tiers.map(|t| t.tier1_limit_km),
        tier2_rate_per_km: rate.tiers.map(|t| t.tier2_rate_per_km),
        tier2_distance_km: km_display(tier2_km),
        travelled_before_km: travelled,
        currency: business.currency.clone(),
        claim_amount: claim,
    }))
}

async fn trip_fields(
    conn: &mut PgConnection,
    business: &BusinessRow,
    input: &TripInput,
    existing: Option<&TripRow>,
) -> AppResult<TripFields> {
    let mut errors = FieldErrors::new();
    let purpose = clean(&input.purpose, 300, "purpose", &mut errors);
    errors.check(
        !purpose.is_empty(),
        "purpose",
        "Enter the purpose of the trip.",
    );
    let start_location = clean(&input.start_location, 200, "start_location", &mut errors);
    let destination = clean(&input.destination, 200, "destination", &mut errors);
    let notes = clean(&input.notes, 2000, "notes", &mut errors);
    let calculation = calculate_trip(conn, business, input, existing, &mut errors).await?;
    errors.into_result()?;
    let c = calculation.ok_or_else(|| {
        AppError::internal(anyhow::anyhow!("trip calculation missing without errors"))
    })?;
    let odometer = trip_distance(input.start_odometer, input.end_odometer, input.distance_km)
        .ok()
        .and_then(|(input, _)| match input {
            calc::DistanceInput::Odometer { start, end } => Some((start, end)),
            calc::DistanceInput::Distance(_) => None,
        });
    Ok(TripFields {
        vehicle_id: input.vehicle_id,
        trip_date: input.trip_date,
        start_location,
        destination,
        purpose,
        start_odometer: odometer.map(|(start, _)| start),
        end_odometer: odometer.map(|(_, end)| end),
        distance_km: c.distance_km,
        business_percentage: input.business_percentage.round_dp(2),
        business_distance_km: c.business_distance_km,
        rate_id: c.rate_id,
        rate_per_km: c.rate_per_km,
        rate_overridden: c.rate_overridden,
        tiers: c.tier1_limit_km.zip(c.tier2_rate_per_km).map(
            |(tier1_limit_km, tier2_rate_per_km)| Tiers {
                tier1_limit_km,
                tier2_rate_per_km,
            },
        ),
        tier2_distance_km: c.tier2_distance_km,
        return_trip: input.return_trip,
        currency: business.currency.clone(),
        claim_amount: c.claim_amount,
        notes,
    })
}

async fn sync_postings(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    business: &BusinessRow,
    id: Uuid,
) -> AppResult<()> {
    let row = repo::find_trip(conn, id)
        .await?
        .ok_or(AppError::NotFound("Trip"))?;
    let mut desired = Vec::new();
    if row.voided_at.is_none() && row.claim_amount > 0 {
        desired.push(Desired {
            source_type: SOURCE_MILEAGE,
            source_id: id,
            spec: ledger::mileage_claim(row.trip_date, &row.purpose, row.claim_amount),
        });
    }
    let context = Context {
        business_id: ctx.business_id,
        currency: business.currency.clone(),
        user_id: Some(ctx.user_id()),
    };
    ledger_service::reconcile(conn, &context, &[(SOURCE_MILEAGE, id)], desired).await
}

/// Reprices the tiered trips of a vehicle's financial year containing `date`, in the order they count
/// toward the tier 1 limit, after a trip in it was added, changed or deleted. Each trip keeps the
/// rates and limit it was priced with; only its tier 2 kilometres and claim follow the year's travel.
async fn reprice_year(
    conn: &mut PgConnection,
    ctx: &BusinessContext,
    business: &BusinessRow,
    vehicle_id: Uuid,
    date: NaiveDate,
) -> AppResult<()> {
    let currency = currency_of(business)?;
    let (from, to) = financial_year(
        date,
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    let mut travelled = Decimal::ZERO;
    for trip in repo::tier_trips(conn, vehicle_id, from, to).await? {
        if let (Some(limit), Some(tier2_rate)) = (trip.tier1_limit_km, trip.tier2_rate_per_km) {
            let tier2 = tier2_business_km(
                trip.distance_km,
                trip.business_distance_km,
                travelled,
                limit,
            );
            let claim = tiered_claim(
                trip.business_distance_km,
                tier2,
                trip.rate_per_km,
                tier2_rate,
                currency,
            )
            .ok_or_else(|| AppError::internal(anyhow::anyhow!("tiered claim overflow")))?;
            if tier2 != trip.tier2_distance_km || claim != trip.claim_amount {
                repo::set_trip_claim(conn, trip.id, tier2, claim).await?;
                sync_postings(conn, ctx, business, trip.id).await?;
            }
        }
        travelled += trip.distance_km;
    }
    Ok(())
}

async fn trip_detail(conn: &mut PgConnection, id: Uuid) -> AppResult<TripDetail> {
    let row = repo::find_trip(conn, id)
        .await?
        .ok_or(AppError::NotFound("Trip"))?;
    let journal = ledger_repo::history(conn, &[(SOURCE_MILEAGE, id)]).await?;
    Ok(TripDetail {
        trip: row.into(),
        journal,
    })
}

/// `trip_id` is the trip being edited, so the preview accepts what saving it would (its vehicle and
/// rate even if since archived).
pub async fn calculate(
    state: &AppState,
    ctx: &BusinessContext,
    trip_id: Option<Uuid>,
    input: TripInput,
) -> AppResult<TripCalculation> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let existing = match trip_id {
        Some(id) => Some(
            repo::find_trip(&mut tx, id)
                .await?
                .ok_or(AppError::NotFound("Trip"))?,
        ),
        None => None,
    };
    let mut errors = FieldErrors::new();
    let calculation =
        calculate_trip(&mut tx, &business, &input, existing.as_ref(), &mut errors).await?;
    tx.commit().await?;
    errors.into_result()?;
    calculation.ok_or_else(|| {
        AppError::internal(anyhow::anyhow!("trip calculation missing without errors"))
    })
}

/// Routes recorded at least twice in the last year, most often first, for filling in a new trip.
pub async fn frequent_trips(
    state: &AppState,
    ctx: &BusinessContext,
) -> AppResult<Vec<FrequentTrip>> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let since = business_today(&business.timezone) - chrono::Duration::days(365);
    let mut rows = repo::frequent_trips(&mut tx, since, 8).await?;
    tx.commit().await?;
    for row in &mut rows {
        row.distance_km = km_display(row.distance_km);
    }
    Ok(rows)
}

pub async fn list_trips(
    state: &AppState,
    ctx: &BusinessContext,
    filter: TripFilter,
    page: u32,
    per_page: i64,
) -> AppResult<TripPage> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let offset = i64::from(page.max(1) - 1) * per_page;
    let items = repo::list_trips(&mut tx, &filter, Some(per_page), offset).await?;
    let totals = repo::trip_totals(&mut tx, &filter).await?.scaled();
    tx.commit().await?;
    Ok(TripPage {
        items: items.into_iter().map(Trip::from).collect(),
        page: page.max(1),
        per_page: per_page as u32,
        total: totals.trips,
        totals,
    })
}

pub async fn get_trip(state: &AppState, ctx: &BusinessContext, id: Uuid) -> AppResult<TripDetail> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let detail = trip_detail(&mut tx, id).await?;
    tx.commit().await?;
    Ok(detail)
}

pub async fn create_trip(
    state: &AppState,
    ctx: &BusinessContext,
    input: TripInput,
) -> AppResult<TripDetail> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let fields = trip_fields(&mut tx, &business, &input, None).await?;
    let id = repo::insert_trip(&mut tx, ctx.business_id, ctx.user_id(), &fields).await?;
    sync_postings(&mut tx, ctx, &business, id).await?;
    reprice_year(&mut tx, ctx, &business, fields.vehicle_id, fields.trip_date).await?;
    audit_record(
        &mut tx,
        ctx,
        "mileage_trip.created",
        "mileage_trip",
        id,
        format!(
            "Trip of {} km recorded: {}",
            fields.distance_km, fields.purpose
        ),
        serde_json::json!({
            "distance_km": fields.distance_km,
            "business_distance_km": fields.business_distance_km,
            "rate_per_km": fields.rate_per_km,
            "claim_amount": fields.claim_amount,
        }),
    )
    .await?;
    let detail = trip_detail(&mut tx, id).await?;
    tx.commit().await?;
    Ok(detail)
}

pub async fn update_trip(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    input: TripInput,
) -> AppResult<TripDetail> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let before = repo::find_trip(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Trip"))?;
    if before.voided_at.is_some() {
        return Err(AppError::Conflict(
            "A deleted trip can’t be changed.".into(),
        ));
    }
    let fields = trip_fields(&mut tx, &business, &input, Some(&before)).await?;
    repo::update_trip(&mut tx, id, &fields).await?;
    sync_postings(&mut tx, ctx, &business, id).await?;
    reprice_year(&mut tx, ctx, &business, before.vehicle_id, before.trip_date).await?;
    reprice_year(&mut tx, ctx, &business, fields.vehicle_id, fields.trip_date).await?;
    audit_record(
        &mut tx,
        ctx,
        "mileage_trip.updated",
        "mileage_trip",
        id,
        format!("Trip updated: {}", fields.purpose),
        serde_json::json!({
            "trip_date": { "from": before.trip_date, "to": fields.trip_date },
            "distance_km": { "from": before.distance_km, "to": fields.distance_km },
            "business_percentage": { "from": before.business_percentage, "to": fields.business_percentage },
            "rate_per_km": { "from": before.rate_per_km, "to": fields.rate_per_km },
            "claim_amount": { "from": before.claim_amount, "to": fields.claim_amount },
        }),
    )
    .await?;
    let detail = trip_detail(&mut tx, id).await?;
    tx.commit().await?;
    Ok(detail)
}

pub async fn void_trip(
    state: &AppState,
    ctx: &BusinessContext,
    id: Uuid,
    reason: String,
) -> AppResult<()> {
    let mut errors = FieldErrors::new();
    let reason = clean(&reason, 500, "reason", &mut errors);
    errors.into_result()?;
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let row = repo::find_trip(&mut tx, id)
        .await?
        .ok_or(AppError::NotFound("Trip"))?;
    if row.voided_at.is_some() {
        return Err(AppError::Conflict("This trip is already deleted.".into()));
    }
    repo::void_trip(&mut tx, id, &reason).await?;
    sync_postings(&mut tx, ctx, &business, id).await?;
    reprice_year(&mut tx, ctx, &business, row.vehicle_id, row.trip_date).await?;
    audit_record(
        &mut tx,
        ctx,
        "mileage_trip.deleted",
        "mileage_trip",
        id,
        format!("Trip deleted: {}", row.purpose),
        serde_json::json!({ "reason": reason, "claim_amount": row.claim_amount }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// A period from the query, defaulting to the current financial year to date.
fn period(
    business: &BusinessRow,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> AppResult<(NaiveDate, NaiveDate)> {
    let today = business_today(&business.timezone);
    let (fy_start, _) = financial_year(
        today,
        business.financial_year_start_month as u32,
        business.financial_year_start_day as u32,
    );
    let from = from.unwrap_or(fy_start);
    let to = to.unwrap_or(today);
    if to < from {
        return Err(AppError::validation(
            "to",
            "The end date can’t be before the start date.",
        ));
    }
    if (to - from).num_days() > 366 * 5 {
        return Err(AppError::validation(
            "from",
            "Choose a period of up to five years.",
        ));
    }
    Ok((from, to))
}

pub async fn summary(
    state: &AppState,
    ctx: &BusinessContext,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    vehicle_id: Option<Uuid>,
) -> AppResult<MileageSummary> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let (from, to) = period(&business, from, to)?;
    let filter = TripFilter {
        from: Some(from),
        to: Some(to),
        vehicle_id,
        ..TripFilter::default()
    };
    let totals = repo::trip_totals(&mut tx, &filter).await?.scaled();
    let monthly = repo::monthly_mileage(&mut tx, from, to, vehicle_id)
        .await?
        .into_iter()
        .map(|mut m| {
            m.totals = m.totals.scaled();
            m
        })
        .collect();
    let vehicles = repo::mileage_by_vehicle(&mut tx, from, to, vehicle_id)
        .await?
        .into_iter()
        .map(|mut v| {
            v.totals = v.totals.scaled();
            v
        })
        .collect();
    tx.commit().await?;
    Ok(MileageSummary {
        from,
        to,
        vehicle_id,
        currency: business.currency.clone(),
        totals,
        monthly,
        vehicles,
    })
}

/// Trips in a period as CSV (Excel-compatible): returns the file name and the bytes.
pub async fn export_csv(
    state: &AppState,
    ctx: &BusinessContext,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    vehicle_id: Option<Uuid>,
) -> AppResult<(String, Vec<u8>)> {
    let mut tx = state
        .db
        .begin_business(ctx.user_id(), ctx.business_id)
        .await?;
    let business = load_business(&mut tx, ctx.business_id).await?;
    let (from, to) = period(&business, from, to)?;
    let currency = currency_of(&business)?;
    let filter = TripFilter {
        from: Some(from),
        to: Some(to),
        vehicle_id,
        ..TripFilter::default()
    };
    let mut trips = repo::list_trips(&mut tx, &filter, None, 0).await?;
    let vehicles = repo::list_vehicles(&mut tx).await?;
    tx.commit().await?;
    trips.reverse(); // Oldest first, like a logbook.

    let mut csv = CsvWriter::new();
    csv.text_row(&[
        "Date",
        "Vehicle",
        "Registration",
        "From",
        "To",
        "Purpose",
        "Start odometer (km)",
        "End odometer (km)",
        "Distance (km)",
        "Business share (%)",
        "Business distance (km)",
        "Personal distance (km)",
        "Rate",
        "Rate per km",
        format!("Claim ({})", business.currency).as_str(),
        "Notes",
    ]);
    for t in &trips {
        let registration = vehicles
            .iter()
            .find(|v| v.id == t.vehicle_id)
            .map(|v| v.registration.clone())
            .unwrap_or_default();
        let rate = if t.rate_overridden {
            "Entered for this trip".to_owned()
        } else {
            t.rate_name.clone().unwrap_or_default()
        };
        let odometer =
            |km: Option<Decimal>| km.map(|k| km_display(k).to_string()).unwrap_or_default();
        csv.row(vec![
            number_cell(t.trip_date),
            text_cell(&t.vehicle_name),
            text_cell(&registration),
            text_cell(&t.start_location),
            text_cell(&t.destination),
            text_cell(&t.purpose),
            odometer(t.start_odometer),
            odometer(t.end_odometer),
            number_cell(km_display(t.distance_km)),
            number_cell(t.business_percentage.normalize()),
            number_cell(km_display(t.business_distance_km)),
            number_cell(km_display(t.personal_distance_km)),
            text_cell(&rate),
            number_cell(t.rate_per_km.normalize()),
            number_cell(currency.to_decimal(t.claim_amount)),
            text_cell(&t.notes),
        ]);
    }
    let name = format!("Mileage {from} to {to}.csv");
    Ok((name, csv.into_bytes()))
}

/// Mileage totals for the dashboard.
pub async fn totals_for_period(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
) -> AppResult<TripTotals> {
    Ok(repo::trip_totals(
        conn,
        &TripFilter {
            from: Some(from),
            to: Some(to),
            ..TripFilter::default()
        },
    )
    .await?
    .scaled())
}
