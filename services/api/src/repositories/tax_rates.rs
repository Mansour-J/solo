//! Versioned tax rates (reference data).

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgConnection;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct TaxRateRow {
    pub jurisdiction: String,
    pub tax_type: String,
    /// Rate as a fraction, e.g. `0.15` for 15%.
    #[schema(value_type = String)]
    pub rate: Decimal,
    pub effective_from: NaiveDate,
    /// Exclusive end date; `null` while the rate is current.
    pub effective_to: Option<NaiveDate>,
    pub description: String,
}

pub async fn list(
    conn: &mut PgConnection,
    jurisdiction: &str,
) -> Result<Vec<TaxRateRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT jurisdiction::text AS jurisdiction, tax_type, rate, effective_from, effective_to, description
         FROM tax_rates WHERE jurisdiction = $1 ORDER BY tax_type, effective_from",
    )
    .bind(jurisdiction)
    .fetch_all(conn)
    .await
}

/// The rate in effect on `date`, if any.
pub async fn rate_on(
    conn: &mut PgConnection,
    jurisdiction: &str,
    tax_type: &str,
    date: NaiveDate,
) -> Result<Option<Decimal>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT rate FROM tax_rates
         WHERE jurisdiction = $1 AND tax_type = $2 AND effective_from <= $3 AND (effective_to IS NULL OR effective_to > $3)",
    )
    .bind(jurisdiction)
    .bind(tax_type)
    .bind(date)
    .fetch_optional(conn)
    .await
}
