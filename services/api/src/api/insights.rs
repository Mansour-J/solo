//! `/api/v1/businesses/{business_id}/insights`: what the figures mean, in plain language, and the
//! notices a user can dismiss.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::ApiJson;
use crate::context::BusinessContext;
use crate::error::{AppResult, ErrorBody};
use crate::services::insights::{self, DismissInput, Insights};
use crate::state::AppState;

/// Insights in four sections and the notices this user hasn't dismissed. Calculated on request.
#[utoipa::path(get, path = "/{business_id}/insights", tag = "insights", params(("business_id" = uuid::Uuid, Path)),
    responses((status = OK, body = Insights)))]
async fn get_insights(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<Insights>> {
    Ok(Json(insights::build(&state, &ctx).await?))
}

/// Hides a notice for this user until what it says changes.
#[utoipa::path(post, path = "/{business_id}/insights/dismiss", tag = "insights", params(("business_id" = uuid::Uuid, Path)),
    request_body = DismissInput,
    responses((status = NO_CONTENT), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn dismiss(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<DismissInput>,
) -> AppResult<StatusCode> {
    insights::dismiss(&state, &ctx, input).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Shows every notice this user dismissed again.
#[utoipa::path(post, path = "/{business_id}/insights/restore", tag = "insights", params(("business_id" = uuid::Uuid, Path)),
    responses((status = NO_CONTENT)))]
async fn restore(State(state): State<AppState>, ctx: BusinessContext) -> AppResult<StatusCode> {
    insights::restore(&state, &ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_insights))
        .routes(routes!(dismiss))
        .routes(routes!(restore))
}
