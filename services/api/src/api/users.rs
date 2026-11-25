//! `/api/v1/users/me`: the owner's profile and the full data export.

use axum::Json;
use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::ApiJson;
use crate::context::Owner;
use crate::error::{AppResult, ErrorBody};
use crate::services::email_settings::{self, EmailSettings, EmailSettingsInput, TestEmailInput};
use crate::services::notices_email::{
    self, NoticesEmail, NoticesEmailInput, NoticesRun, NoticesRunInput,
};
use crate::services::users::{self, UpdateProfileInput, UserProfile};
use crate::state::AppState;

/// The owner's profile.
#[utoipa::path(get, path = "", tag = "users", responses((status = OK, body = UserProfile)))]
async fn me(State(state): State<AppState>, owner: Owner) -> AppResult<Json<UserProfile>> {
    Ok(Json(users::profile(&state, &owner).await?))
}

/// Update name and preferences.
#[utoipa::path(put, path = "", tag = "users", request_body = UpdateProfileInput,
    responses((status = OK, body = UserProfile), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update_me(
    State(state): State<AppState>,
    owner: Owner,
    ApiJson(input): ApiJson<UpdateProfileInput>,
) -> AppResult<Json<UserProfile>> {
    Ok(Json(users::update_profile(&state, &owner, input).await?))
}

/// Download all data (the profile and every business) as JSON.
#[utoipa::path(get, path = "/export", tag = "users", responses((status = OK, body = Object, content_type = "application/json")))]
async fn export(State(state): State<AppState>, owner: Owner) -> AppResult<impl IntoResponse> {
    let data = users::export(&state, &owner).await?;
    let filename = format!("solo-export-{}.json", chrono::Utc::now().format("%Y-%m-%d"));
    Ok((
        [
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        Json(data),
    ))
}

/// How email is sent (the password is never returned).
#[utoipa::path(get, path = "/email-settings", tag = "users", responses((status = OK, body = EmailSettings)))]
async fn get_email(State(state): State<AppState>, owner: Owner) -> AppResult<Json<EmailSettings>> {
    Ok(Json(email_settings::get(&state, &owner).await?))
}

/// Saves the owner's mail account (an empty password keeps the saved one).
#[utoipa::path(put, path = "/email-settings", tag = "users", request_body = EmailSettingsInput,
    responses((status = OK, body = EmailSettings), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn save_email(
    State(state): State<AppState>,
    owner: Owner,
    ApiJson(input): ApiJson<EmailSettingsInput>,
) -> AppResult<Json<EmailSettings>> {
    Ok(Json(email_settings::save(&state, &owner, input).await?))
}

#[utoipa::path(delete, path = "/email-settings", tag = "users", responses((status = NO_CONTENT)))]
async fn delete_email(
    State(state): State<AppState>,
    owner: Owner,
) -> AppResult<axum::http::StatusCode> {
    email_settings::delete(&state, &owner).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// Sends a test email with the saved settings; a failure is a conflict with the mail server's words.
#[utoipa::path(post, path = "/email-settings/test", tag = "users", request_body = TestEmailInput,
    responses((status = OK, body = EmailSettings), (status = CONFLICT, body = ErrorBody)))]
async fn test_email(
    State(state): State<AppState>,
    owner: Owner,
    ApiJson(input): ApiJson<TestEmailInput>,
) -> AppResult<Json<EmailSettings>> {
    Ok(Json(email_settings::test(&state, &owner, input).await?))
}

/// What's working on this installation: database, file storage, PDF documents and email, with what to do
/// about anything that isn't.
#[utoipa::path(get, path = "/system-status", tag = "users", responses((status = OK, body = crate::services::system_status::SystemStatus)))]
async fn system_status(
    State(state): State<AppState>,
    owner: Owner,
) -> Json<crate::services::system_status::SystemStatus> {
    Json(crate::services::system_status::status(&state, &owner).await)
}

/// The weekly notices email setting.
#[utoipa::path(get, path = "/notices-email", tag = "users", responses((status = OK, body = NoticesEmail)))]
async fn get_notices_email(
    State(state): State<AppState>,
    owner: Owner,
) -> AppResult<Json<NoticesEmail>> {
    Ok(Json(notices_email::get(&state, &owner).await?))
}

#[utoipa::path(put, path = "/notices-email", tag = "users", request_body = NoticesEmailInput,
    responses((status = OK, body = NoticesEmail), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn save_notices_email(
    State(state): State<AppState>,
    owner: Owner,
    ApiJson(input): ApiJson<NoticesEmailInput>,
) -> AppResult<Json<NoticesEmail>> {
    Ok(Json(notices_email::save(&state, &owner, input).await?))
}

/// Sends the weekly notices email if it's due (the app calls this when it opens), or now.
#[utoipa::path(post, path = "/notices-email/run", tag = "users", request_body = NoticesRunInput,
    responses((status = OK, body = NoticesRun), (status = CONFLICT, body = ErrorBody)))]
async fn run_notices_email(
    State(state): State<AppState>,
    owner: Owner,
    ApiJson(input): ApiJson<NoticesRunInput>,
) -> AppResult<Json<NoticesRun>> {
    Ok(Json(notices_email::run(&state, &owner, input).await?))
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(me, update_me))
        .routes(routes!(export))
        .routes(routes!(get_email, save_email, delete_email))
        .routes(routes!(test_email))
        .routes(routes!(system_status))
        .routes(routes!(get_notices_email, save_notices_email))
        .routes(routes!(run_notices_email))
}
