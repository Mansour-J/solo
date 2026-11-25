//! `/api/v1/businesses`: the user's businesses, profile, settings, logo and activity log.

use axum::Json;
use axum::extract::{Multipart, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use super::extract::{ApiJson, ApiQuery, Page, PageParams};
use crate::context::{BusinessContext, Owner};
use crate::error::{AppError, AppResult, ErrorBody};
use crate::repositories::audit::{AuditFilter, AuditLogRow};
use crate::services::businesses::{
    self, Business, BusinessInput, BusinessSettings, BusinessSummary,
};
use crate::state::AppState;

#[derive(Deserialize, ToSchema)]
pub struct DeleteBusinessInput {
    /// The business name, retyped by the user.
    pub confirmation_name: String,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ActivityQuery {
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
}

/// Businesses the user belongs to.
#[utoipa::path(get, path = "", tag = "businesses", responses((status = OK, body = Vec<BusinessSummary>)))]
async fn list(
    State(state): State<AppState>,
    session: Owner,
) -> AppResult<Json<Vec<BusinessSummary>>> {
    Ok(Json(businesses::list(&state, &session).await?))
}

/// Create a business (onboarding). The creator becomes its owner.
#[utoipa::path(post, path = "", tag = "businesses", request_body = BusinessInput,
    responses((status = CREATED, body = Business), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create(
    State(state): State<AppState>,
    session: Owner,
    ApiJson(input): ApiJson<BusinessInput>,
) -> AppResult<(StatusCode, Json<Business>)> {
    Ok((
        StatusCode::CREATED,
        Json(businesses::create(&state, &session, input).await?),
    ))
}

/// One business's profile.
#[utoipa::path(get, path = "/{business_id}", tag = "businesses", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = Business), (status = NOT_FOUND, body = ErrorBody)))]
async fn get(State(state): State<AppState>, ctx: BusinessContext) -> AppResult<Json<Business>> {
    Ok(Json(businesses::get(&state, &ctx).await?))
}

/// Replace the business profile.
#[utoipa::path(put, path = "/{business_id}", tag = "businesses", params(("business_id" = Uuid, Path)), request_body = BusinessInput,
    responses((status = OK, body = Business), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<BusinessInput>,
) -> AppResult<Json<Business>> {
    Ok(Json(businesses::update(&state, &ctx, input).await?))
}

/// Delete the business and all of its records (owner only).
#[utoipa::path(delete, path = "/{business_id}", tag = "businesses", params(("business_id" = Uuid, Path)), request_body = DeleteBusinessInput,
    responses((status = NO_CONTENT), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn delete(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<DeleteBusinessInput>,
) -> AppResult<StatusCode> {
    businesses::delete(&state, &ctx, &input.confirmation_name).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Invoice, quote and expense defaults.
#[utoipa::path(get, path = "/{business_id}/settings", tag = "businesses", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = BusinessSettings)))]
async fn get_settings(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<BusinessSettings>> {
    Ok(Json(businesses::settings(&state, &ctx).await?))
}

#[utoipa::path(put, path = "/{business_id}/settings", tag = "businesses", params(("business_id" = Uuid, Path)), request_body = BusinessSettings,
    responses((status = OK, body = BusinessSettings), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update_settings(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<BusinessSettings>,
) -> AppResult<Json<BusinessSettings>> {
    Ok(Json(
        businesses::update_settings(&state, &ctx, input).await?,
    ))
}

/// The business logo image.
#[utoipa::path(get, path = "/{business_id}/logo", tag = "businesses", params(("business_id" = Uuid, Path)),
    responses((status = OK, content_type = "image/*", body = Vec<u8>), (status = NOT_FOUND, body = ErrorBody)))]
async fn get_logo(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<impl IntoResponse> {
    let (bytes, content_type) = businesses::logo(&state, &ctx).await?;
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "private, max-age=300"),
        ],
        bytes,
    ))
}

/// Upload a logo (multipart field `file`; PNG, JPEG or WebP up to 2 MB).
#[utoipa::path(put, path = "/{business_id}/logo", tag = "businesses", params(("business_id" = Uuid, Path)),
    request_body(content_type = "multipart/form-data", content = Object),
    responses((status = OK, body = Business), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn upload_logo(
    State(state): State<AppState>,
    ctx: BusinessContext,
    mut multipart: Multipart,
) -> AppResult<Json<Business>> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.body_text()))?
    {
        if field.name() == Some("file") {
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(e.body_text()))?;
            return Ok(Json(
                businesses::upload_logo(&state, &ctx, bytes.to_vec()).await?,
            ));
        }
    }
    Err(AppError::validation("file", "Choose an image to upload."))
}

#[utoipa::path(delete, path = "/{business_id}/logo", tag = "businesses", params(("business_id" = Uuid, Path)),
    responses((status = NO_CONTENT)))]
async fn delete_logo(State(state): State<AppState>, ctx: BusinessContext) -> AppResult<StatusCode> {
    businesses::remove_logo(&state, &ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Audit history of changes in the business, newest first.
#[utoipa::path(get, path = "/{business_id}/activity", tag = "businesses", params(("business_id" = Uuid, Path), PageParams, ActivityQuery),
    responses((status = OK, body = Page<AuditLogRow>)))]
async fn activity(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(page): ApiQuery<PageParams>,
    ApiQuery(query): ApiQuery<ActivityQuery>,
) -> AppResult<Json<Page<AuditLogRow>>> {
    let filter = AuditFilter {
        entity_type: query.entity_type,
        entity_id: query.entity_id,
    };
    let (items, total) =
        businesses::activity(&state, &ctx, filter, page.limit(), page.offset()).await?;
    Ok(Json(Page::new(items, page, total)))
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(get, update, delete))
        .routes(routes!(get_settings, update_settings))
        .routes(routes!(get_logo, upload_logo, delete_logo))
        .routes(routes!(activity))
}
