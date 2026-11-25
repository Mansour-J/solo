//! `/api/v1/businesses/{business_id}/recurring-invoices`: schedules made from invoices, and the run
//! that creates the invoices that have come due.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ItemPath};
use crate::context::BusinessContext;
use crate::error::{AppResult, ErrorBody};
use crate::repositories::recurring::RecurringRow;
use crate::services::recurring::{
    self, RecurringArchiveInput, RecurringFromInvoice, RecurringInput, RunResult,
};
use crate::state::AppState;

#[utoipa::path(get, path = "/{business_id}/recurring-invoices", tag = "invoices", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = Vec<RecurringRow>)))]
async fn list_recurring(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<Vec<RecurringRow>>> {
    Ok(Json(recurring::list(&state, &ctx).await?))
}

/// Makes a schedule that repeats an invoice.
#[utoipa::path(post, path = "/{business_id}/recurring-invoices", tag = "invoices", params(("business_id" = Uuid, Path)), request_body = RecurringFromInvoice,
    responses((status = CREATED, body = RecurringRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create_recurring(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<RecurringFromInvoice>,
) -> AppResult<(StatusCode, Json<RecurringRow>)> {
    Ok((
        StatusCode::CREATED,
        Json(recurring::create(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(put, path = "/{business_id}/recurring-invoices/{recurring_id}", tag = "invoices",
    params(("business_id" = Uuid, Path), ("recurring_id" = Uuid, Path)), request_body = RecurringInput,
    responses((status = OK, body = RecurringRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update_recurring(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<RecurringInput>,
) -> AppResult<Json<RecurringRow>> {
    Ok(Json(recurring::update(&state, &ctx, p.id, input).await?))
}

/// Stops or restarts a schedule.
#[utoipa::path(post, path = "/{business_id}/recurring-invoices/{recurring_id}/archive", tag = "invoices",
    params(("business_id" = Uuid, Path), ("recurring_id" = Uuid, Path)), request_body = RecurringArchiveInput,
    responses((status = OK, body = RecurringRow), (status = CONFLICT, body = ErrorBody)))]
async fn archive_recurring(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<RecurringArchiveInput>,
) -> AppResult<Json<RecurringRow>> {
    Ok(Json(
        recurring::archive(&state, &ctx, p.id, input.archived).await?,
    ))
}

/// Creates the invoices that have come due (called when the app opens).
#[utoipa::path(post, path = "/{business_id}/recurring-invoices/run", tag = "invoices", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = RunResult)))]
async fn run_recurring(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<RunResult>> {
    Ok(Json(recurring::run(&state, &ctx).await?))
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_recurring, create_recurring))
        .routes(routes!(run_recurring))
        .routes(routes!(update_recurring))
        .routes(routes!(archive_recurring))
}
