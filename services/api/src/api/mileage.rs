//! `/api/v1/businesses/{business_id}/mileage`: vehicles, rates, trips, the mileage report and exports.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use chrono::NaiveDate;
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use super::extract::{ApiJson, ApiQuery, ItemPath, PageParams, SubItemPath};
use crate::context::BusinessContext;
use crate::error::{AppResult, ErrorBody};
use crate::repositories::mileage::{FrequentTrip, TripFilter, VehicleRow};
use crate::services::invoices::ReasonInput;
use crate::services::mileage::{
    self, ArchiveInput, MileageRate, MileageSummary, NewRateInput, RateInput, RateVersionInput,
    TripCalculation, TripDetail, TripInput, TripPage, VehicleInput,
};
use crate::state::AppState;

// ─── Vehicles ───────────────────────────────────────────────────────────────

#[utoipa::path(get, path = "/{business_id}/mileage/vehicles", tag = "mileage", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = Vec<VehicleRow>)))]
async fn list_vehicles(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<Vec<VehicleRow>>> {
    Ok(Json(mileage::list_vehicles(&state, &ctx).await?))
}

#[utoipa::path(post, path = "/{business_id}/mileage/vehicles", tag = "mileage", params(("business_id" = Uuid, Path)), request_body = VehicleInput,
    responses((status = CREATED, body = VehicleRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create_vehicle(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<VehicleInput>,
) -> AppResult<(StatusCode, Json<VehicleRow>)> {
    Ok((
        StatusCode::CREATED,
        Json(mileage::create_vehicle(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(put, path = "/{business_id}/mileage/vehicles/{vehicle_id}", tag = "mileage",
    params(("business_id" = Uuid, Path), ("vehicle_id" = Uuid, Path)), request_body = VehicleInput,
    responses((status = OK, body = VehicleRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update_vehicle(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<VehicleInput>,
) -> AppResult<Json<VehicleRow>> {
    Ok(Json(
        mileage::update_vehicle(&state, &ctx, p.id, input).await?,
    ))
}

#[utoipa::path(post, path = "/{business_id}/mileage/vehicles/{vehicle_id}/archive", tag = "mileage",
    params(("business_id" = Uuid, Path), ("vehicle_id" = Uuid, Path)), request_body = ArchiveInput,
    responses((status = OK, body = VehicleRow)))]
async fn archive_vehicle(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ArchiveInput>,
) -> AppResult<Json<VehicleRow>> {
    Ok(Json(
        mileage::archive_vehicle(&state, &ctx, p.id, input.archived).await?,
    ))
}

// ─── Rates ──────────────────────────────────────────────────────────────────

#[utoipa::path(get, path = "/{business_id}/mileage/rates", tag = "mileage", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = Vec<MileageRate>)))]
async fn list_rates(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<Vec<MileageRate>>> {
    Ok(Json(mileage::list_rates(&state, &ctx).await?))
}

#[utoipa::path(post, path = "/{business_id}/mileage/rates", tag = "mileage", params(("business_id" = Uuid, Path)), request_body = NewRateInput,
    responses((status = CREATED, body = MileageRate), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create_rate(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<NewRateInput>,
) -> AppResult<(StatusCode, Json<MileageRate>)> {
    Ok((
        StatusCode::CREATED,
        Json(mileage::create_rate(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(put, path = "/{business_id}/mileage/rates/{rate_id}", tag = "mileage",
    params(("business_id" = Uuid, Path), ("rate_id" = Uuid, Path)), request_body = RateInput,
    responses((status = OK, body = MileageRate), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update_rate(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<RateInput>,
) -> AppResult<Json<MileageRate>> {
    Ok(Json(mileage::update_rate(&state, &ctx, p.id, input).await?))
}

#[utoipa::path(post, path = "/{business_id}/mileage/rates/{rate_id}/archive", tag = "mileage",
    params(("business_id" = Uuid, Path), ("rate_id" = Uuid, Path)), request_body = ArchiveInput,
    responses((status = OK, body = MileageRate), (status = CONFLICT, body = ErrorBody)))]
async fn archive_rate(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ArchiveInput>,
) -> AppResult<Json<MileageRate>> {
    Ok(Json(
        mileage::archive_rate(&state, &ctx, p.id, input.archived).await?,
    ))
}

#[utoipa::path(post, path = "/{business_id}/mileage/rates/{rate_id}/versions", tag = "mileage",
    params(("business_id" = Uuid, Path), ("rate_id" = Uuid, Path)), request_body = RateVersionInput,
    responses((status = CREATED, body = MileageRate), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn add_version(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<RateVersionInput>,
) -> AppResult<(StatusCode, Json<MileageRate>)> {
    Ok((
        StatusCode::CREATED,
        Json(mileage::add_version(&state, &ctx, p.id, input).await?),
    ))
}

#[utoipa::path(put, path = "/{business_id}/mileage/rates/{rate_id}/versions/{version_id}", tag = "mileage",
    params(("business_id" = Uuid, Path), ("rate_id" = Uuid, Path), ("version_id" = Uuid, Path)), request_body = RateVersionInput,
    responses((status = OK, body = MileageRate), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update_version(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<SubItemPath>,
    ApiJson(input): ApiJson<RateVersionInput>,
) -> AppResult<Json<MileageRate>> {
    Ok(Json(
        mileage::update_version(&state, &ctx, p.id, p.sub_id, input).await?,
    ))
}

#[utoipa::path(delete, path = "/{business_id}/mileage/rates/{rate_id}/versions/{version_id}", tag = "mileage",
    params(("business_id" = Uuid, Path), ("rate_id" = Uuid, Path), ("version_id" = Uuid, Path)),
    responses((status = OK, body = MileageRate), (status = CONFLICT, body = ErrorBody)))]
async fn delete_version(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<SubItemPath>,
) -> AppResult<Json<MileageRate>> {
    Ok(Json(
        mileage::delete_version(&state, &ctx, p.id, p.sub_id).await?,
    ))
}

// ─── Trips ──────────────────────────────────────────────────────────────────

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TripQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub vehicle_id: Option<Uuid>,
    /// Matches purpose, start, destination or notes.
    pub search: Option<String>,
}

/// Routes recorded at least twice in the last year, most often first (to fill in a new trip).
#[utoipa::path(get, path = "/{business_id}/mileage/frequent-trips", tag = "mileage", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = Vec<FrequentTrip>)))]
async fn frequent_trips(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<Vec<FrequentTrip>>> {
    Ok(Json(mileage::frequent_trips(&state, &ctx).await?))
}

#[utoipa::path(get, path = "/{business_id}/mileage/trips", tag = "mileage", params(("business_id" = Uuid, Path), TripQuery, PageParams),
    responses((status = OK, body = TripPage)))]
async fn list_trips(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<TripQuery>,
    ApiQuery(page): ApiQuery<PageParams>,
) -> AppResult<Json<TripPage>> {
    let filter = TripFilter {
        from: q.from,
        to: q.to,
        vehicle_id: q.vehicle_id,
        search: q
            .search
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty()),
        include_deleted: false,
    };
    Ok(Json(
        mileage::list_trips(&state, &ctx, filter, page.page, page.limit()).await?,
    ))
}

#[utoipa::path(post, path = "/{business_id}/mileage/trips", tag = "mileage", params(("business_id" = Uuid, Path)), request_body = TripInput,
    responses((status = CREATED, body = TripDetail), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create_trip(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<TripInput>,
) -> AppResult<(StatusCode, Json<TripDetail>)> {
    Ok((
        StatusCode::CREATED,
        Json(mileage::create_trip(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(get, path = "/{business_id}/mileage/trips/{trip_id}", tag = "mileage",
    params(("business_id" = Uuid, Path), ("trip_id" = Uuid, Path)),
    responses((status = OK, body = TripDetail)))]
async fn get_trip(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<TripDetail>> {
    Ok(Json(mileage::get_trip(&state, &ctx, p.id).await?))
}

#[utoipa::path(put, path = "/{business_id}/mileage/trips/{trip_id}", tag = "mileage",
    params(("business_id" = Uuid, Path), ("trip_id" = Uuid, Path)), request_body = TripInput,
    responses((status = OK, body = TripDetail), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update_trip(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<TripInput>,
) -> AppResult<Json<TripDetail>> {
    Ok(Json(mileage::update_trip(&state, &ctx, p.id, input).await?))
}

#[utoipa::path(delete, path = "/{business_id}/mileage/trips/{trip_id}", tag = "mileage",
    params(("business_id" = Uuid, Path), ("trip_id" = Uuid, Path)), request_body = ReasonInput,
    responses((status = NO_CONTENT)))]
async fn delete_trip(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ReasonInput>,
) -> AppResult<StatusCode> {
    mileage::void_trip(&state, &ctx, p.id, input.reason).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CalculateQuery {
    /// The trip being edited: its vehicle and rate stay usable even if since archived.
    pub trip_id: Option<Uuid>,
}

/// Works out a trip's distance, rate and claim without saving it (for the trip form).
#[utoipa::path(post, path = "/{business_id}/mileage/calculate", tag = "mileage", params(("business_id" = Uuid, Path), CalculateQuery), request_body = TripInput,
    responses((status = OK, body = TripCalculation), (status = NOT_FOUND, body = ErrorBody), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn calculate(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<CalculateQuery>,
    ApiJson(input): ApiJson<TripInput>,
) -> AppResult<Json<TripCalculation>> {
    Ok(Json(
        mileage::calculate(&state, &ctx, q.trip_id, input).await?,
    ))
}

// ─── Report and export ──────────────────────────────────────────────────────

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MileageReportQuery {
    /// Defaults to the start of the current financial year.
    pub from: Option<NaiveDate>,
    /// Defaults to today.
    pub to: Option<NaiveDate>,
    pub vehicle_id: Option<Uuid>,
}

#[utoipa::path(get, path = "/{business_id}/mileage/summary", tag = "mileage", params(("business_id" = Uuid, Path), MileageReportQuery),
    responses((status = OK, body = MileageSummary)))]
async fn summary(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<MileageReportQuery>,
) -> AppResult<Json<MileageSummary>> {
    Ok(Json(
        mileage::summary(&state, &ctx, q.from, q.to, q.vehicle_id).await?,
    ))
}

/// Trips in the period as CSV (UTF-8 with BOM, opens in Excel).
#[utoipa::path(get, path = "/{business_id}/mileage/export", tag = "mileage", params(("business_id" = Uuid, Path), MileageReportQuery),
    responses((status = OK, content_type = "text/csv", body = String)))]
async fn export(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<MileageReportQuery>,
) -> AppResult<impl IntoResponse> {
    let (filename, bytes) = mileage::export_csv(&state, &ctx, q.from, q.to, q.vehicle_id).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        bytes,
    ))
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_vehicles, create_vehicle))
        .routes(routes!(update_vehicle))
        .routes(routes!(archive_vehicle))
        .routes(routes!(list_rates, create_rate))
        .routes(routes!(update_rate))
        .routes(routes!(archive_rate))
        .routes(routes!(add_version))
        .routes(routes!(update_version, delete_version))
        .routes(routes!(list_trips, create_trip))
        .routes(routes!(frequent_trips))
        .routes(routes!(get_trip, update_trip, delete_trip))
        .routes(routes!(calculate))
        .routes(routes!(summary))
        .routes(routes!(export))
}
