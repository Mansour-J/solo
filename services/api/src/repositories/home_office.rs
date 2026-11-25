//! Home office claims (one per business and financial year) and IRD's square metre rates.

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgConnection;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct HomeOfficeClaim {
    pub id: Uuid,
    pub year_end: NaiveDate,
    #[schema(value_type = String)]
    pub business_area: Decimal,
    #[schema(value_type = String)]
    pub total_area: Decimal,
    /// Mortgage interest, rates and rent for the year (minor units).
    pub premises_costs: i64,
    /// IRD's rate used, in cents per square metre.
    pub rate: i64,
    /// The amount claimed (minor units).
    pub claim: i64,
    pub notes: String,
    pub updated_at: DateTime<Utc>,
}

macro_rules! columns {
    () => {
        "id, year_end, business_area, total_area, premises_costs, rate, claim, notes, updated_at"
    };
}

/// IRD's rate for the income year ending on `year_end`, if published.
pub async fn rate(
    conn: &mut PgConnection,
    year_end: NaiveDate,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT rate FROM home_office_rates WHERE jurisdiction = 'NZ' AND year_end = $1",
    )
    .bind(year_end)
    .fetch_optional(conn)
    .await
}

pub async fn find(
    conn: &mut PgConnection,
    year_end: NaiveDate,
) -> Result<Option<HomeOfficeClaim>, sqlx::Error> {
    sqlx::query_as(concat!(
        "SELECT ",
        columns!(),
        " FROM home_office_claims WHERE year_end = $1"
    ))
    .bind(year_end)
    .fetch_optional(conn)
    .await
}

pub struct ClaimFields {
    pub year_end: NaiveDate,
    pub business_area: Decimal,
    pub total_area: Decimal,
    pub premises_costs: i64,
    pub rate: i64,
    pub claim: i64,
    pub notes: String,
}

pub async fn save(
    conn: &mut PgConnection,
    business_id: Uuid,
    f: &ClaimFields,
) -> Result<HomeOfficeClaim, sqlx::Error> {
    sqlx::query_as(concat!(
        "INSERT INTO home_office_claims
           (business_id, year_end, business_area, total_area, premises_costs, rate, claim, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         ON CONFLICT (business_id, year_end) DO UPDATE SET business_area = EXCLUDED.business_area,
           total_area = EXCLUDED.total_area, premises_costs = EXCLUDED.premises_costs,
           rate = EXCLUDED.rate, claim = EXCLUDED.claim, notes = EXCLUDED.notes, updated_at = now()
         RETURNING ",
        columns!()
    ))
    .bind(business_id)
    .bind(f.year_end)
    .bind(f.business_area)
    .bind(f.total_area)
    .bind(f.premises_costs)
    .bind(f.rate)
    .bind(f.claim)
    .bind(&f.notes)
    .fetch_one(conn)
    .await
}

pub async fn delete(
    conn: &mut PgConnection,
    year_end: NaiveDate,
) -> Result<Option<Uuid>, sqlx::Error> {
    sqlx::query_scalar("DELETE FROM home_office_claims WHERE year_end = $1 RETURNING id")
        .bind(year_end)
        .fetch_optional(conn)
        .await
}
