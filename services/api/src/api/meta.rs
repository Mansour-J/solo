//! Public application metadata: branding, version, reference data.

use axum::Json;
use axum::extract::State;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::config::Brand;
use crate::domain::money::Currency;
use crate::error::AppResult;
use crate::repositories::tax_rates::{self, TaxRateRow};
use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct CurrencyInfo {
    pub code: String,
    pub minor_units: u32,
    pub symbol: String,
}

#[derive(Serialize, ToSchema)]
pub struct AppMeta {
    pub brand: Brand,
    /// Whether the server can send email (invoices and quotes). Off unless a mail server is set up.
    pub email_enabled: bool,
    pub version: &'static str,
    pub currencies: Vec<CurrencyInfo>,
}

/// Product identity and client configuration. Clients render the brand from here.
#[utoipa::path(get, path = "", tag = "meta", responses((status = OK, body = AppMeta)))]
async fn meta(State(state): State<AppState>) -> Json<AppMeta> {
    Json(AppMeta {
        brand: state.config.brand.clone(),
        email_enabled: crate::services::email_settings::enabled(&state)
            .await
            .unwrap_or(false),
        version: env!("CARGO_PKG_VERSION"),
        currencies: Currency::all()
            .iter()
            .map(|c| CurrencyInfo {
                code: c.code.into(),
                minor_units: c.minor_units,
                symbol: c.symbol.into(),
            })
            .collect(),
    })
}

/// Tax rates with their effective dates (New Zealand).
#[utoipa::path(get, path = "/tax-rates", tag = "meta", responses((status = OK, body = Vec<TaxRateRow>)))]
async fn list_tax_rates(State(state): State<AppState>) -> AppResult<Json<Vec<TaxRateRow>>> {
    let mut conn = state.db.pool().acquire().await?;
    Ok(Json(tax_rates::list(&mut conn, "NZ").await?))
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(meta))
        .routes(routes!(list_tax_rates))
}
