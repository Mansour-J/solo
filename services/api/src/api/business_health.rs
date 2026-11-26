//! `/api/v1/businesses/{business_id}/health`: whether the business is set up to keep correct books,
//! issue valid tax invoices and get paid.

use axum::Json;
use axum::extract::State;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::context::BusinessContext;
use crate::error::AppResult;
use crate::services::business_health::{self, BusinessHealth};
use crate::state::AppState;

/// Every business health check with what it found, why it matters and where to fix it. Calculated on request.
#[utoipa::path(get, path = "/{business_id}/health", tag = "businesses", params(("business_id" = uuid::Uuid, Path)),
    responses((status = OK, body = BusinessHealth)))]
async fn get_health(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<BusinessHealth>> {
    Ok(Json(business_health::build(&state, &ctx).await?))
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(get_health))
}
