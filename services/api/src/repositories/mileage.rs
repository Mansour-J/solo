//! Vehicles, mileage rates (with dated versions) and trips. Row-level security limits every query to
//! the current business.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::mileage::Tiers;

// ─── Rates ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RateRow {
    pub id: Uuid,
    pub name: String,
    pub notes: String,
    pub archived: bool,
    /// Vehicles using this rate by default.
    pub vehicle_count: i64,
    /// Trips (not deleted) that took their rate from this one.
    pub trip_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

macro_rules! rate_columns {
    () => {
        "r.id, r.name, r.notes, r.archived_at IS NOT NULL AS archived,
         (SELECT count(*) FROM vehicles v WHERE v.default_rate_id = r.id AND v.archived_at IS NULL) AS vehicle_count,
         (SELECT count(*) FROM mileage_trips t WHERE t.rate_id = r.id AND t.voided_at IS NULL) AS trip_count,
         r.created_at, r.updated_at FROM mileage_rates r"
    };
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct RateVersionRow {
    pub id: Uuid,
    #[serde(skip)]
    pub rate_id: Uuid,
    pub effective_from: NaiveDate,
    /// Amount per kilometre in the business's currency, e.g. "1.0400".
    #[schema(value_type = String)]
    pub rate_per_km: Decimal,
    /// IRD's tier 1 limit: kilometres a year at `rate_per_km` (None without tiers).
    #[schema(value_type = Option<String>)]
    pub tier1_limit_km: Option<Decimal>,
    /// The rate per kilometre after the tier 1 limit.
    #[schema(value_type = Option<String>)]
    pub tier2_rate_per_km: Option<Decimal>,
}

macro_rules! version_columns {
    () => {
        "id, rate_id, effective_from, rate_per_km, tier1_limit_km, tier2_rate_per_km FROM mileage_rate_versions"
    };
}

pub async fn list_rates(conn: &mut PgConnection) -> Result<Vec<RateRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        rate_columns!(),
        " ORDER BY r.archived_at IS NOT NULL, lower(r.name)"
    ))
    .fetch_all(conn)
    .await
}

pub async fn find_rate(conn: &mut PgConnection, id: Uuid) -> Result<Option<RateRow>, sqlx::Error> {
    sqlx::query_as(concat!("SELECT ", rate_columns!(), " WHERE r.id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

/// Versions of every rate (or of one), oldest first.
pub async fn rate_versions(
    conn: &mut PgConnection,
    rate_id: Option<Uuid>,
) -> Result<Vec<RateVersionRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        version_columns!(),
        " WHERE $1::uuid IS NULL OR rate_id = $1 ORDER BY rate_id, effective_from"
    ))
    .bind(rate_id)
    .fetch_all(conn)
    .await
}

pub async fn find_version(
    conn: &mut PgConnection,
    rate_id: Uuid,
    id: Uuid,
) -> Result<Option<RateVersionRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        version_columns!(),
        " WHERE id = $1 AND rate_id = $2"
    ))
    .bind(id)
    .bind(rate_id)
    .fetch_optional(conn)
    .await
}

pub async fn rate_name_taken(
    conn: &mut PgConnection,
    name: &str,
    except: Option<Uuid>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM mileage_rates WHERE lower(name) = lower($1) AND archived_at IS NULL AND ($2::uuid IS NULL OR id <> $2))",
    )
    .bind(name)
    .bind(except)
    .fetch_one(conn)
    .await
}

pub async fn version_date_taken(
    conn: &mut PgConnection,
    rate_id: Uuid,
    effective_from: NaiveDate,
    except: Option<Uuid>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM mileage_rate_versions WHERE rate_id = $1 AND effective_from = $2 AND ($3::uuid IS NULL OR id <> $3))",
    )
    .bind(rate_id)
    .bind(effective_from)
    .bind(except)
    .fetch_one(conn)
    .await
}

pub async fn insert_rate(
    conn: &mut PgConnection,
    business_id: Uuid,
    name: &str,
    notes: &str,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO mileage_rates (business_id, name, notes) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(business_id)
    .bind(name)
    .bind(notes)
    .fetch_one(conn)
    .await
}

pub async fn update_rate(
    conn: &mut PgConnection,
    id: Uuid,
    name: &str,
    notes: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE mileage_rates SET name = $2, notes = $3 WHERE id = $1")
        .bind(id)
        .bind(name)
        .bind(notes)
        .execute(conn)
        .await
        .map(|_| ())
}

pub async fn set_rate_archived(
    conn: &mut PgConnection,
    id: Uuid,
    archived: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE mileage_rates SET archived_at = CASE WHEN $2 THEN coalesce(archived_at, now()) END WHERE id = $1",
    )
    .bind(id)
    .bind(archived)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn insert_version(
    conn: &mut PgConnection,
    business_id: Uuid,
    rate_id: Uuid,
    effective_from: NaiveDate,
    rate_per_km: Decimal,
    tiers: Option<Tiers>,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO mileage_rate_versions (business_id, rate_id, effective_from, rate_per_km, tier1_limit_km, tier2_rate_per_km)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(business_id)
    .bind(rate_id)
    .bind(effective_from)
    .bind(rate_per_km)
    .bind(tiers.map(|t| t.tier1_limit_km))
    .bind(tiers.map(|t| t.tier2_rate_per_km))
    .fetch_one(conn)
    .await
}

pub async fn update_version(
    conn: &mut PgConnection,
    id: Uuid,
    effective_from: NaiveDate,
    rate_per_km: Decimal,
    tiers: Option<Tiers>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE mileage_rate_versions SET effective_from = $2, rate_per_km = $3, tier1_limit_km = $4, tier2_rate_per_km = $5
         WHERE id = $1",
    )
    .bind(id)
    .bind(effective_from)
    .bind(rate_per_km)
    .bind(tiers.map(|t| t.tier1_limit_km))
    .bind(tiers.map(|t| t.tier2_rate_per_km))
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn delete_version(conn: &mut PgConnection, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM mileage_rate_versions WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await
        .map(|_| ())
}

// ─── Vehicles ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct VehicleRow {
    pub id: Uuid,
    pub name: String,
    pub registration: String,
    pub make: String,
    pub model: String,
    pub year: Option<i16>,
    /// car, van, ute, motorcycle, truck or other.
    pub vehicle_type: String,
    /// petrol, diesel, hybrid, plug_in_hybrid, electric or other.
    pub fuel_type: String,
    pub default_rate_id: Option<Uuid>,
    pub default_rate_name: Option<String>,
    /// The vehicle new trips start with.
    pub is_default: bool,
    pub notes: String,
    pub archived: bool,
    /// Trips recorded with this vehicle (not deleted).
    pub trip_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

macro_rules! vehicle_columns {
    () => {
        "v.id, v.name, v.registration, v.make, v.model, v.year, v.vehicle_type, v.fuel_type, v.default_rate_id,
         r.name AS default_rate_name, v.is_default, v.notes, v.archived_at IS NOT NULL AS archived,
         (SELECT count(*) FROM mileage_trips t WHERE t.vehicle_id = v.id AND t.voided_at IS NULL) AS trip_count,
         v.created_at, v.updated_at
         FROM vehicles v LEFT JOIN mileage_rates r ON r.id = v.default_rate_id"
    };
}

pub struct VehicleFields {
    pub name: String,
    pub registration: String,
    pub make: String,
    pub model: String,
    pub year: Option<i16>,
    pub vehicle_type: String,
    pub fuel_type: String,
    pub default_rate_id: Option<Uuid>,
    pub is_default: bool,
    pub notes: String,
}

pub async fn list_vehicles(conn: &mut PgConnection) -> Result<Vec<VehicleRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        vehicle_columns!(),
        " ORDER BY v.archived_at IS NOT NULL, NOT v.is_default, lower(v.name)"
    ))
    .fetch_all(conn)
    .await
}

pub async fn find_vehicle(
    conn: &mut PgConnection,
    id: Uuid,
) -> Result<Option<VehicleRow>, sqlx::Error> {
    sqlx::query_as(concat!("SELECT ", vehicle_columns!(), " WHERE v.id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub async fn vehicle_name_taken(
    conn: &mut PgConnection,
    name: &str,
    except: Option<Uuid>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM vehicles WHERE lower(name) = lower($1) AND archived_at IS NULL AND ($2::uuid IS NULL OR id <> $2))",
    )
    .bind(name)
    .bind(except)
    .fetch_one(conn)
    .await
}

pub async fn active_vehicle_count(conn: &mut PgConnection) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM vehicles WHERE archived_at IS NULL")
        .fetch_one(conn)
        .await
}

/// Makes `id` the only default vehicle (or clears the default when `id` is None).
pub async fn clear_default_vehicle(
    conn: &mut PgConnection,
    except: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE vehicles SET is_default = false WHERE is_default AND ($1::uuid IS NULL OR id <> $1)",
    )
    .bind(except)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn insert_vehicle(
    conn: &mut PgConnection,
    business_id: Uuid,
    f: &VehicleFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO vehicles (business_id, name, registration, make, model, year, vehicle_type, fuel_type, default_rate_id, is_default, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING id",
    )
    .bind(business_id)
    .bind(&f.name)
    .bind(&f.registration)
    .bind(&f.make)
    .bind(&f.model)
    .bind(f.year)
    .bind(&f.vehicle_type)
    .bind(&f.fuel_type)
    .bind(f.default_rate_id)
    .bind(f.is_default)
    .bind(&f.notes)
    .fetch_one(conn)
    .await
}

pub async fn update_vehicle(
    conn: &mut PgConnection,
    id: Uuid,
    f: &VehicleFields,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE vehicles SET name = $2, registration = $3, make = $4, model = $5, year = $6, vehicle_type = $7, fuel_type = $8,
            default_rate_id = $9, is_default = $10, notes = $11
         WHERE id = $1",
    )
    .bind(id)
    .bind(&f.name)
    .bind(&f.registration)
    .bind(&f.make)
    .bind(&f.model)
    .bind(f.year)
    .bind(&f.vehicle_type)
    .bind(&f.fuel_type)
    .bind(f.default_rate_id)
    .bind(f.is_default)
    .bind(&f.notes)
    .execute(conn)
    .await
    .map(|_| ())
}

/// Archives or restores a vehicle; an archived vehicle is never the default.
pub async fn set_vehicle_archived(
    conn: &mut PgConnection,
    id: Uuid,
    archived: bool,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE vehicles SET archived_at = CASE WHEN $2 THEN coalesce(archived_at, now()) END,
            is_default = is_default AND NOT $2
         WHERE id = $1",
    )
    .bind(id)
    .bind(archived)
    .execute(conn)
    .await
    .map(|_| ())
}

// ─── Trips ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TripRow {
    pub id: Uuid,
    pub vehicle_id: Uuid,
    pub vehicle_name: String,
    pub trip_date: NaiveDate,
    pub start_location: String,
    pub destination: String,
    pub purpose: String,
    pub start_odometer: Option<Decimal>,
    pub end_odometer: Option<Decimal>,
    pub distance_km: Decimal,
    pub business_percentage: Decimal,
    pub business_distance_km: Decimal,
    pub personal_distance_km: Decimal,
    pub rate_id: Option<Uuid>,
    pub rate_name: Option<String>,
    pub rate_per_km: Decimal,
    pub rate_overridden: bool,
    pub tier1_limit_km: Option<Decimal>,
    pub tier2_rate_per_km: Option<Decimal>,
    pub tier2_distance_km: Decimal,
    pub return_trip: bool,
    pub currency: String,
    pub claim_amount: i64,
    pub notes: String,
    pub voided_at: Option<DateTime<Utc>>,
    pub void_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

macro_rules! trip_columns {
    () => {
        "t.id, t.vehicle_id, v.name AS vehicle_name, t.trip_date, t.start_location, t.destination, t.purpose,
         t.start_odometer, t.end_odometer, t.distance_km, t.business_percentage, t.business_distance_km,
         t.distance_km - t.business_distance_km AS personal_distance_km, t.rate_id, r.name AS rate_name,
         t.rate_per_km, t.rate_overridden, t.tier1_limit_km, t.tier2_rate_per_km, t.tier2_distance_km, t.return_trip, t.currency, t.claim_amount, t.notes, t.voided_at, t.void_reason,
         t.created_at, t.updated_at
         FROM mileage_trips t JOIN vehicles v ON v.id = t.vehicle_id LEFT JOIN mileage_rates r ON r.id = t.rate_id"
    };
}

/// Conditions shared by the trip list, totals, export and summary ($1 from, $2 to, $3 vehicle,
/// $4 search pattern, $5 include deleted).
macro_rules! trip_filter {
    () => {
        " WHERE ($5 OR t.voided_at IS NULL)
            AND ($1::date IS NULL OR t.trip_date >= $1) AND ($2::date IS NULL OR t.trip_date <= $2)
            AND ($3::uuid IS NULL OR t.vehicle_id = $3)
            AND ($4::text IS NULL OR t.purpose ILIKE $4 OR t.start_location ILIKE $4 OR t.destination ILIKE $4 OR t.notes ILIKE $4)"
    };
}

pub struct TripFields {
    pub vehicle_id: Uuid,
    pub trip_date: NaiveDate,
    pub start_location: String,
    pub destination: String,
    pub purpose: String,
    pub start_odometer: Option<Decimal>,
    pub end_odometer: Option<Decimal>,
    pub distance_km: Decimal,
    pub business_percentage: Decimal,
    pub business_distance_km: Decimal,
    pub rate_id: Option<Uuid>,
    pub rate_per_km: Decimal,
    pub rate_overridden: bool,
    pub tiers: Option<Tiers>,
    pub tier2_distance_km: Decimal,
    pub return_trip: bool,
    pub currency: String,
    pub claim_amount: i64,
    pub notes: String,
}

#[derive(Debug, Clone, Default)]
pub struct TripFilter {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub vehicle_id: Option<Uuid>,
    pub search: Option<String>,
    pub include_deleted: bool,
}

impl TripFilter {
    fn pattern(&self) -> Option<String> {
        self.search.as_ref().map(|s| {
            format!(
                "%{}%",
                s.replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            )
        })
    }
}

pub async fn insert_trip(
    conn: &mut PgConnection,
    business_id: Uuid,
    user_id: Uuid,
    f: &TripFields,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO mileage_trips (business_id, vehicle_id, trip_date, start_location, destination, purpose, start_odometer, end_odometer,
            distance_km, business_percentage, business_distance_km, rate_id, rate_per_km, rate_overridden, currency, claim_amount, notes, created_by,
            tier1_limit_km, tier2_rate_per_km, tier2_distance_km, return_trip)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22) RETURNING id",
    )
    .bind(business_id)
    .bind(f.vehicle_id)
    .bind(f.trip_date)
    .bind(&f.start_location)
    .bind(&f.destination)
    .bind(&f.purpose)
    .bind(f.start_odometer)
    .bind(f.end_odometer)
    .bind(f.distance_km)
    .bind(f.business_percentage)
    .bind(f.business_distance_km)
    .bind(f.rate_id)
    .bind(f.rate_per_km)
    .bind(f.rate_overridden)
    .bind(&f.currency)
    .bind(f.claim_amount)
    .bind(&f.notes)
    .bind(user_id)
    .bind(f.tiers.map(|t| t.tier1_limit_km))
    .bind(f.tiers.map(|t| t.tier2_rate_per_km))
    .bind(f.tier2_distance_km)
    .bind(f.return_trip)
    .fetch_one(conn)
    .await
}

pub async fn update_trip(
    conn: &mut PgConnection,
    id: Uuid,
    f: &TripFields,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE mileage_trips SET vehicle_id = $2, trip_date = $3, start_location = $4, destination = $5, purpose = $6,
            start_odometer = $7, end_odometer = $8, distance_km = $9, business_percentage = $10, business_distance_km = $11,
            rate_id = $12, rate_per_km = $13, rate_overridden = $14, claim_amount = $15, notes = $16,
            tier1_limit_km = $17, tier2_rate_per_km = $18, tier2_distance_km = $19, return_trip = $20
         WHERE id = $1 AND voided_at IS NULL",
    )
    .bind(id)
    .bind(f.vehicle_id)
    .bind(f.trip_date)
    .bind(&f.start_location)
    .bind(&f.destination)
    .bind(&f.purpose)
    .bind(f.start_odometer)
    .bind(f.end_odometer)
    .bind(f.distance_km)
    .bind(f.business_percentage)
    .bind(f.business_distance_km)
    .bind(f.rate_id)
    .bind(f.rate_per_km)
    .bind(f.rate_overridden)
    .bind(f.claim_amount)
    .bind(&f.notes)
    .bind(f.tiers.map(|t| t.tier1_limit_km))
    .bind(f.tiers.map(|t| t.tier2_rate_per_km))
    .bind(f.tier2_distance_km)
    .bind(f.return_trip)
    .execute(conn)
    .await
    .map(|_| ())
}

pub async fn void_trip(conn: &mut PgConnection, id: Uuid, reason: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE mileage_trips SET voided_at = now(), void_reason = $2 WHERE id = $1 AND voided_at IS NULL",
    )
    .bind(id)
    .bind(reason)
    .execute(conn)
    .await
    .map(|_| ())
}

/// Kilometres a vehicle travelled (business and private, trips not deleted) from `from` up to a
/// trip on `date`: trips on earlier dates, and on the same date those recorded before `except`
/// (every one when `except` is None, a trip not yet saved).
pub async fn travelled_before(
    conn: &mut PgConnection,
    vehicle_id: Uuid,
    from: NaiveDate,
    date: NaiveDate,
    except: Option<Uuid>,
) -> Result<Decimal, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT coalesce(sum(t.distance_km), 0) FROM mileage_trips t
         LEFT JOIN mileage_trips e ON e.id = $4
         WHERE t.vehicle_id = $1 AND t.voided_at IS NULL AND t.trip_date >= $2 AND t.trip_date <= $3
           AND ($4::uuid IS NULL OR (t.id <> $4 AND (t.trip_date < $3 OR (t.created_at, t.id) < (e.created_at, e.id))))",
    )
    .bind(vehicle_id)
    .bind(from)
    .bind(date)
    .bind(except)
    .fetch_one(conn)
    .await
}

/// A trip's figures for repricing its year's tiers.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct TierTrip {
    pub id: Uuid,
    pub distance_km: Decimal,
    pub business_distance_km: Decimal,
    pub rate_per_km: Decimal,
    pub tier1_limit_km: Option<Decimal>,
    pub tier2_rate_per_km: Option<Decimal>,
    pub tier2_distance_km: Decimal,
    pub claim_amount: i64,
}

/// A vehicle's trips (not deleted) from `from` to `to`, in the order they count toward the tiers.
pub async fn tier_trips(
    conn: &mut PgConnection,
    vehicle_id: Uuid,
    from: NaiveDate,
    to: NaiveDate,
) -> Result<Vec<TierTrip>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, distance_km, business_distance_km, rate_per_km, tier1_limit_km, tier2_rate_per_km, tier2_distance_km, claim_amount
         FROM mileage_trips WHERE vehicle_id = $1 AND voided_at IS NULL AND trip_date BETWEEN $2 AND $3
         ORDER BY trip_date, created_at, id",
    )
    .bind(vehicle_id)
    .bind(from)
    .bind(to)
    .fetch_all(conn)
    .await
}

pub async fn set_trip_claim(
    conn: &mut PgConnection,
    id: Uuid,
    tier2_distance_km: Decimal,
    claim_amount: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE mileage_trips SET tier2_distance_km = $2, claim_amount = $3 WHERE id = $1 AND voided_at IS NULL",
    )
    .bind(id)
    .bind(tier2_distance_km)
    .bind(claim_amount)
    .execute(conn)
    .await
    .map(|_| ())
}

/// A trip made often: its route and details from the latest time, with how often it was made.
#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct FrequentTrip {
    pub vehicle_id: Uuid,
    pub vehicle_name: String,
    pub start_location: String,
    pub destination: String,
    pub purpose: String,
    /// The distance as entered: one way for a return trip.
    #[schema(value_type = String)]
    pub distance_km: Decimal,
    pub return_trip: bool,
    #[schema(value_type = String)]
    pub business_percentage: Decimal,
    /// How many times it was recorded since `since`.
    pub trips: i64,
    pub last_date: NaiveDate,
}

/// The routes recorded at least twice since `since` (trips not deleted, active vehicles), most
/// often first; trips by odometer readings are left out (their distance changes every time).
pub async fn frequent_trips(
    conn: &mut PgConnection,
    since: NaiveDate,
    limit: i64,
) -> Result<Vec<FrequentTrip>, sqlx::Error> {
    sqlx::query_as(
        "SELECT t.vehicle_id, v.name AS vehicle_name, t.start_location, t.destination, t.purpose,
                CASE WHEN t.return_trip THEN t.distance_km / 2 ELSE t.distance_km END AS distance_km,
                t.return_trip, t.business_percentage, count(*) AS trips, max(t.trip_date) AS last_date
         FROM mileage_trips t JOIN vehicles v ON v.id = t.vehicle_id
         WHERE t.voided_at IS NULL AND v.archived_at IS NULL AND t.start_odometer IS NULL AND t.trip_date >= $1
         GROUP BY t.vehicle_id, v.name, t.start_location, t.destination, t.purpose, t.distance_km, t.return_trip,
                  t.business_percentage
         HAVING count(*) >= 2
         ORDER BY count(*) DESC, max(t.trip_date) DESC
         LIMIT $2",
    )
    .bind(since)
    .bind(limit)
    .fetch_all(conn)
    .await
}

pub async fn find_trip(conn: &mut PgConnection, id: Uuid) -> Result<Option<TripRow>, sqlx::Error> {
    sqlx::query_as(concat!("SELECT ", trip_columns!(), " WHERE t.id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

/// Trips matching the filter, newest first. `limit = None` returns all of them (for exports).
pub async fn list_trips(
    conn: &mut PgConnection,
    filter: &TripFilter,
    limit: Option<i64>,
    offset: i64,
) -> Result<Vec<TripRow>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        trip_columns!(),
        trip_filter!(),
        " ORDER BY t.trip_date DESC, t.created_at DESC LIMIT $6 OFFSET $7"
    ))
    .bind(filter.from)
    .bind(filter.to)
    .bind(filter.vehicle_id)
    .bind(filter.pattern())
    .bind(filter.include_deleted)
    .bind(limit)
    .bind(offset)
    .fetch_all(conn)
    .await
}

/// Number of trips and their totals.
#[derive(Debug, Clone, Default, Serialize, ToSchema, sqlx::FromRow)]
pub struct TripTotals {
    pub trips: i64,
    #[schema(value_type = String)]
    pub distance_km: Decimal,
    #[schema(value_type = String)]
    pub business_distance_km: Decimal,
    #[schema(value_type = String)]
    pub personal_distance_km: Decimal,
    /// Minor units.
    pub claim_amount: i64,
}

impl TripTotals {
    /// Kilometres with one decimal place (sums of zero come back without a scale).
    pub fn scaled(mut self) -> Self {
        use crate::domain::mileage::km_display;
        self.distance_km = km_display(self.distance_km);
        self.business_distance_km = km_display(self.business_distance_km);
        self.personal_distance_km = km_display(self.personal_distance_km);
        self
    }
}

macro_rules! trip_aggregates {
    () => {
        "count(t.id) AS trips, coalesce(sum(t.distance_km), 0) AS distance_km,
         coalesce(sum(t.business_distance_km), 0) AS business_distance_km,
         coalesce(sum(t.distance_km - t.business_distance_km), 0) AS personal_distance_km,
         coalesce(sum(t.claim_amount), 0)::bigint AS claim_amount"
    };
}

/// Totals of the trips matching the filter (deleted trips never count).
pub async fn trip_totals(
    conn: &mut PgConnection,
    filter: &TripFilter,
) -> Result<TripTotals, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        trip_aggregates!(),
        " FROM mileage_trips t",
        trip_filter!()
    ))
    .bind(filter.from)
    .bind(filter.to)
    .bind(filter.vehicle_id)
    .bind(filter.pattern())
    .bind(false)
    .fetch_one(conn)
    .await
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct MonthMileage {
    /// First day of the month.
    pub month: NaiveDate,
    #[serde(flatten)]
    #[sqlx(flatten)]
    pub totals: TripTotals,
}

/// Totals per calendar month from `from` to `to`, including months without trips.
pub async fn monthly_mileage(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    vehicle_id: Option<Uuid>,
) -> Result<Vec<MonthMileage>, sqlx::Error> {
    sqlx::query_as(concat!(
        "WITH months AS (
           SELECT generate_series(date_trunc('month', $1::date), date_trunc('month', $2::date), interval '1 month')::date AS month
         )
         SELECT m.month, ",
        trip_aggregates!(),
        " FROM months m
         LEFT JOIN mileage_trips t ON date_trunc('month', t.trip_date)::date = m.month AND t.trip_date BETWEEN $1 AND $2
           AND t.voided_at IS NULL AND ($3::uuid IS NULL OR t.vehicle_id = $3)
         GROUP BY m.month ORDER BY m.month"
    ))
    .bind(from)
    .bind(to)
    .bind(vehicle_id)
    .fetch_all(conn)
    .await
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct VehicleMileage {
    pub vehicle_id: Uuid,
    pub vehicle_name: String,
    #[serde(flatten)]
    #[sqlx(flatten)]
    pub totals: TripTotals,
}

/// Totals per vehicle with trips in the period, most business kilometres first.
pub async fn mileage_by_vehicle(
    conn: &mut PgConnection,
    from: NaiveDate,
    to: NaiveDate,
    vehicle_id: Option<Uuid>,
) -> Result<Vec<VehicleMileage>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT v.id AS vehicle_id, v.name AS vehicle_name, ",
        trip_aggregates!(),
        " FROM mileage_trips t JOIN vehicles v ON v.id = t.vehicle_id
         WHERE t.voided_at IS NULL AND t.trip_date BETWEEN $1 AND $2 AND ($3::uuid IS NULL OR t.vehicle_id = $3)
         GROUP BY v.id, v.name ORDER BY sum(t.business_distance_km) DESC, lower(v.name)"
    ))
    .bind(from)
    .bind(to)
    .bind(vehicle_id)
    .fetch_all(conn)
    .await
}
