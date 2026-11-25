//! Invoice reminders: settings, the per-customer switch and the run that sends what's due.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ItemPath};
use crate::context::BusinessContext;
use crate::error::{AppResult, ErrorBody};
use crate::services::reminders::{self, CustomerRemindersInput, ReminderRun, ReminderSettings};
use crate::state::AppState;

#[utoipa::path(get, path = "/{business_id}/reminders/settings", tag = "invoices", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = ReminderSettings)))]
async fn get_reminder_settings(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<ReminderSettings>> {
    Ok(Json(reminders::get_settings(&state, &ctx).await?))
}

#[utoipa::path(put, path = "/{business_id}/reminders/settings", tag = "invoices", params(("business_id" = Uuid, Path)),
    request_body = ReminderSettings,
    responses((status = OK, body = ReminderSettings), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn save_reminder_settings(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<ReminderSettings>,
) -> AppResult<Json<ReminderSettings>> {
    Ok(Json(reminders::save_settings(&state, &ctx, input).await?))
}

/// Sends the reminders due today. The app calls this when it opens; each reminder goes once.
#[utoipa::path(post, path = "/{business_id}/reminders/run", tag = "invoices", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = ReminderRun)))]
async fn run_reminders(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<ReminderRun>> {
    Ok(Json(reminders::run(&state, &ctx).await?))
}

/// Turns invoice reminders on or off for one customer.
#[utoipa::path(put, path = "/{business_id}/customers/{customer_id}/reminders", tag = "invoices",
    params(("business_id" = Uuid, Path), ("customer_id" = Uuid, Path)), request_body = CustomerRemindersInput,
    responses((status = NO_CONTENT), (status = NOT_FOUND, body = ErrorBody)))]
async fn set_customer_reminders(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<CustomerRemindersInput>,
) -> AppResult<StatusCode> {
    reminders::set_customer(&state, &ctx, p.id, input).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_reminder_settings, save_reminder_settings))
        .routes(routes!(run_reminders))
        .routes(routes!(set_customer_reminders))
}
