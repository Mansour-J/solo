//! Annual-accounts records under `/api/v1/businesses/{business_id}`: money in and out
//! (`/cash-movements`), fixed assets and depreciation (`/assets`), and opening balances
//! (`/opening-balances`).

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::NaiveDate;
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiQuery, ItemPath, PageParams};
use crate::context::BusinessContext;
use crate::domain::ledger::MovementKind;
use crate::error::{AppResult, ErrorBody};
use crate::repositories::annual::DisposalRow;
use crate::repositories::annual::{AssetRow, MovementRow};
use crate::services::annual::{
    self, ArchiveInput, AssetInput, DepreciationInput, DepreciationYear, MovementInput,
    MovementPage, OpeningInput, OpeningView,
};
use crate::services::change_of_use::{self, AssetUse, UseReviewInput};
use crate::services::disposals::{self, DisposalInput};
use crate::services::home_office::{self, HomeOfficeInput, HomeOfficeYear};
use crate::services::invoices::ReasonInput;
use crate::state::AppState;

// ─── Money in and out ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MovementQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    #[param(inline)]
    pub kind: Option<MovementKind>,
}

#[utoipa::path(get, path = "/{business_id}/cash-movements", tag = "annual", params(("business_id" = Uuid, Path), MovementQuery, PageParams),
    responses((status = OK, body = MovementPage)))]
async fn list_movements(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<MovementQuery>,
    ApiQuery(page): ApiQuery<PageParams>,
) -> AppResult<Json<MovementPage>> {
    Ok(Json(
        annual::list_movements(&state, &ctx, q.from, q.to, q.kind, page.page, page.limit()).await?,
    ))
}

#[utoipa::path(post, path = "/{business_id}/cash-movements", tag = "annual", params(("business_id" = Uuid, Path)), request_body = MovementInput,
    responses((status = CREATED, body = MovementRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create_movement(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<MovementInput>,
) -> AppResult<(StatusCode, Json<MovementRow>)> {
    Ok((
        StatusCode::CREATED,
        Json(annual::create_movement(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(put, path = "/{business_id}/cash-movements/{movement_id}", tag = "annual",
    params(("business_id" = Uuid, Path), ("movement_id" = Uuid, Path)), request_body = MovementInput,
    responses((status = OK, body = MovementRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update_movement(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<MovementInput>,
) -> AppResult<Json<MovementRow>> {
    Ok(Json(
        annual::update_movement(&state, &ctx, p.id, input).await?,
    ))
}

#[utoipa::path(delete, path = "/{business_id}/cash-movements/{movement_id}", tag = "annual",
    params(("business_id" = Uuid, Path), ("movement_id" = Uuid, Path)), request_body = ReasonInput,
    responses((status = NO_CONTENT), (status = CONFLICT, body = ErrorBody)))]
async fn delete_movement(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ReasonInput>,
) -> AppResult<StatusCode> {
    annual::void_movement(&state, &ctx, p.id, input.reason).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ─── Assets ─────────────────────────────────────────────────────────────────

#[utoipa::path(get, path = "/{business_id}/assets", tag = "annual", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = Vec<AssetRow>)))]
async fn list_assets(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<Vec<AssetRow>>> {
    Ok(Json(annual::list_assets(&state, &ctx).await?))
}

#[utoipa::path(post, path = "/{business_id}/assets", tag = "annual", params(("business_id" = Uuid, Path)), request_body = AssetInput,
    responses((status = CREATED, body = AssetRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create_asset(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<AssetInput>,
) -> AppResult<(StatusCode, Json<AssetRow>)> {
    Ok((
        StatusCode::CREATED,
        Json(annual::create_asset(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(put, path = "/{business_id}/assets/{asset_id}", tag = "annual",
    params(("business_id" = Uuid, Path), ("asset_id" = Uuid, Path)), request_body = AssetInput,
    responses((status = OK, body = AssetRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody), (status = CONFLICT, body = ErrorBody)))]
async fn update_asset(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<AssetInput>,
) -> AppResult<Json<AssetRow>> {
    Ok(Json(annual::update_asset(&state, &ctx, p.id, input).await?))
}

#[utoipa::path(post, path = "/{business_id}/assets/{asset_id}/archive", tag = "annual",
    params(("business_id" = Uuid, Path), ("asset_id" = Uuid, Path)), request_body = ArchiveInput,
    responses((status = OK, body = AssetRow)))]
async fn archive_asset(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ArchiveInput>,
) -> AppResult<Json<AssetRow>> {
    Ok(Json(
        annual::archive_asset(&state, &ctx, p.id, input.archived).await?,
    ))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct YearQuery {
    /// The last day of a financial year.
    pub year_end: NaiveDate,
}

#[utoipa::path(get, path = "/{business_id}/assets/depreciation", tag = "annual", params(("business_id" = Uuid, Path), YearQuery),
    responses((status = OK, body = DepreciationYear), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn get_depreciation(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<YearQuery>,
) -> AppResult<Json<DepreciationYear>> {
    Ok(Json(
        annual::get_depreciation(&state, &ctx, q.year_end).await?,
    ))
}

/// Works out and posts every active asset's depreciation for a financial year.
#[utoipa::path(post, path = "/{business_id}/assets/depreciation", tag = "annual", params(("business_id" = Uuid, Path)), request_body = DepreciationInput,
    responses((status = OK, body = DepreciationYear), (status = UNPROCESSABLE_ENTITY, body = ErrorBody), (status = CONFLICT, body = ErrorBody)))]
async fn post_depreciation(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<DepreciationInput>,
) -> AppResult<Json<DepreciationYear>> {
    Ok(Json(annual::post_depreciation(&state, &ctx, input).await?))
}

/// Removes a year's depreciation (and later years') and reverses it.
#[utoipa::path(delete, path = "/{business_id}/assets/depreciation", tag = "annual", params(("business_id" = Uuid, Path), YearQuery),
    responses((status = NO_CONTENT), (status = NOT_FOUND, body = ErrorBody)))]
async fn delete_depreciation(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<YearQuery>,
) -> AppResult<StatusCode> {
    annual::delete_depreciation(&state, &ctx, q.year_end).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ─── Opening balances ───────────────────────────────────────────────────────

#[utoipa::path(get, path = "/{business_id}/opening-balances", tag = "annual", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = OpeningView)))]
async fn get_opening(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<OpeningView>> {
    Ok(Json(annual::opening(&state, &ctx).await?))
}

#[utoipa::path(put, path = "/{business_id}/opening-balances", tag = "annual", params(("business_id" = Uuid, Path)), request_body = OpeningInput,
    responses((status = OK, body = OpeningView), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn save_opening(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<OpeningInput>,
) -> AppResult<Json<OpeningView>> {
    Ok(Json(annual::save_opening(&state, &ctx, input).await?))
}

#[utoipa::path(delete, path = "/{business_id}/opening-balances", tag = "annual", params(("business_id" = Uuid, Path)),
    responses((status = NO_CONTENT), (status = NOT_FOUND, body = ErrorBody)))]
async fn delete_opening(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<StatusCode> {
    annual::delete_opening(&state, &ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ─── Home office (square metre rate) ────────────────────────────────────────

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct YearEndQuery {
    /// The last day of a financial year.
    pub year_end: NaiveDate,
}

/// The home office claim for a financial year by IRD's square metre rate option, and IRD's rate.
#[utoipa::path(get, path = "/{business_id}/home-office", tag = "annual", params(("business_id" = Uuid, Path), YearEndQuery),
    responses((status = OK, body = HomeOfficeYear), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn get_home_office(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<YearEndQuery>,
) -> AppResult<Json<HomeOfficeYear>> {
    Ok(Json(home_office::year(&state, &ctx, q.year_end).await?))
}

/// Saves the year's claim: (mortgage interest, rates and rent × business share of the floor area) +
/// (business area × IRD's rate), posted as a home office expense paid personally.
#[utoipa::path(put, path = "/{business_id}/home-office", tag = "annual", params(("business_id" = Uuid, Path)), request_body = HomeOfficeInput,
    responses((status = OK, body = HomeOfficeYear), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn save_home_office(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<HomeOfficeInput>,
) -> AppResult<Json<HomeOfficeYear>> {
    Ok(Json(home_office::save(&state, &ctx, input).await?))
}

#[utoipa::path(delete, path = "/{business_id}/home-office", tag = "annual", params(("business_id" = Uuid, Path), YearEndQuery),
    responses((status = NO_CONTENT), (status = NOT_FOUND, body = ErrorBody)))]
async fn delete_home_office(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<YearEndQuery>,
) -> AppResult<StatusCode> {
    home_office::delete(&state, &ctx, q.year_end).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(get, path = "/{business_id}/assets/{asset_id}/use-reviews", tag = "annual",
    params(("business_id" = Uuid, Path), ("asset_id" = Uuid, Path)),
    responses((status = OK, body = AssetUse), (status = NOT_FOUND, body = ErrorBody)))]
async fn get_asset_use(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<AssetUse>> {
    Ok(Json(change_of_use::asset_use(&state, &ctx, p.id).await?))
}

/// Records (or, without `business_use`, removes) an adjustment period's business use; the GST
/// change-of-use adjustments follow.
#[utoipa::path(put, path = "/{business_id}/assets/{asset_id}/use-reviews", tag = "annual",
    params(("business_id" = Uuid, Path), ("asset_id" = Uuid, Path)), request_body = UseReviewInput,
    responses((status = OK, body = AssetUse), (status = UNPROCESSABLE_ENTITY, body = ErrorBody), (status = CONFLICT, body = ErrorBody)))]
async fn save_asset_use(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<UseReviewInput>,
) -> AppResult<Json<AssetUse>> {
    Ok(Json(
        change_of_use::save_review(&state, &ctx, p.id, input).await?,
    ))
}

/// Records the asset as sold or scrapped (or changes the record): no depreciation in that year, and
/// the sale against its value at the start of the year posts depreciation recovered or a loss.
#[utoipa::path(put, path = "/{business_id}/assets/{asset_id}/disposal", tag = "annual",
    params(("business_id" = Uuid, Path), ("asset_id" = Uuid, Path)), request_body = DisposalInput,
    responses((status = OK, body = DisposalRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody), (status = CONFLICT, body = ErrorBody)))]
async fn dispose_asset(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<DisposalInput>,
) -> AppResult<Json<DisposalRow>> {
    Ok(Json(disposals::dispose(&state, &ctx, p.id, input).await?))
}

#[utoipa::path(get, path = "/{business_id}/assets/{asset_id}/disposal", tag = "annual",
    params(("business_id" = Uuid, Path), ("asset_id" = Uuid, Path)),
    responses((status = OK, body = DisposalRow), (status = NOT_FOUND, body = ErrorBody)))]
async fn get_disposal(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<DisposalRow>> {
    Ok(Json(disposals::get(&state, &ctx, p.id).await?))
}

/// Undoes a sale: the asset is held again.
#[utoipa::path(delete, path = "/{business_id}/assets/{asset_id}/disposal", tag = "annual",
    params(("business_id" = Uuid, Path), ("asset_id" = Uuid, Path)),
    responses((status = NO_CONTENT), (status = NOT_FOUND, body = ErrorBody)))]
async fn undo_disposal(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<StatusCode> {
    disposals::undo(&state, &ctx, p.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(
            get_home_office,
            save_home_office,
            delete_home_office
        ))
        .routes(routes!(list_movements, create_movement))
        .routes(routes!(update_movement, delete_movement))
        .routes(routes!(list_assets, create_asset))
        .routes(routes!(
            get_depreciation,
            post_depreciation,
            delete_depreciation
        ))
        .routes(routes!(update_asset))
        .routes(routes!(archive_asset))
        .routes(routes!(get_asset_use, save_asset_use))
        .routes(routes!(get_disposal, dispose_asset, undo_disposal))
        .routes(routes!(get_opening, save_opening, delete_opening))
}
