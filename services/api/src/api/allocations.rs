//! `/api/v1/businesses/{business_id}/allocations`: destinations, rules, allocations of money
//! received and balances. Planning only: Solo never moves money.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::NaiveDate;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use super::extract::{ApiJson, ApiQuery, ItemPath, PageParams};
use crate::context::BusinessContext;
use crate::error::{AppResult, ErrorBody};
use crate::repositories::allocations::{
    AllocationFilter, DestinationRow, RuleRow, SourceRef, SpendingRow,
};
use crate::services::allocations::{
    self, AllocationDetail, AllocationPage, ArchiveInput, BackfillInput, BackfillResult, Balances,
    DestinationInput, OrderInput, PlanView, PreviewInput, ReviewInput, RuleInput, SpendingInput,
    SpendingQuery,
};
use crate::services::invoices::ReasonInput;
use crate::state::AppState;

// ─── Destinations ───────────────────────────────────────────────────────────

#[utoipa::path(get, path = "/{business_id}/allocations/destinations", tag = "allocations", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = Vec<DestinationRow>)))]
async fn list_destinations(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<Vec<DestinationRow>>> {
    Ok(Json(allocations::list_destinations(&state, &ctx).await?))
}

#[utoipa::path(post, path = "/{business_id}/allocations/destinations", tag = "allocations", params(("business_id" = Uuid, Path)), request_body = DestinationInput,
    responses((status = CREATED, body = DestinationRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create_destination(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<DestinationInput>,
) -> AppResult<(StatusCode, Json<DestinationRow>)> {
    Ok((
        StatusCode::CREATED,
        Json(allocations::create_destination(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(put, path = "/{business_id}/allocations/destinations/{destination_id}", tag = "allocations",
    params(("business_id" = Uuid, Path), ("destination_id" = Uuid, Path)), request_body = DestinationInput,
    responses((status = OK, body = DestinationRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update_destination(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<DestinationInput>,
) -> AppResult<Json<DestinationRow>> {
    Ok(Json(
        allocations::update_destination(&state, &ctx, p.id, input).await?,
    ))
}

#[utoipa::path(post, path = "/{business_id}/allocations/destinations/{destination_id}/archive", tag = "allocations",
    params(("business_id" = Uuid, Path), ("destination_id" = Uuid, Path)), request_body = ArchiveInput,
    responses((status = OK, body = DestinationRow), (status = CONFLICT, body = ErrorBody)))]
async fn archive_destination(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ArchiveInput>,
) -> AppResult<Json<DestinationRow>> {
    Ok(Json(
        allocations::archive_destination(&state, &ctx, p.id, input.archived).await?,
    ))
}

#[utoipa::path(put, path = "/{business_id}/allocations/destinations/order", tag = "allocations", params(("business_id" = Uuid, Path)), request_body = OrderInput,
    responses((status = OK, body = Vec<DestinationRow>), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn order_destinations(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<OrderInput>,
) -> AppResult<Json<Vec<DestinationRow>>> {
    Ok(Json(
        allocations::order_destinations(&state, &ctx, input).await?,
    ))
}

// ─── Rules ──────────────────────────────────────────────────────────────────

#[utoipa::path(get, path = "/{business_id}/allocations/rules", tag = "allocations", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = Vec<RuleRow>)))]
async fn list_rules(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<Vec<RuleRow>>> {
    Ok(Json(allocations::list_rules(&state, &ctx).await?))
}

#[utoipa::path(post, path = "/{business_id}/allocations/rules", tag = "allocations", params(("business_id" = Uuid, Path)), request_body = RuleInput,
    responses((status = CREATED, body = RuleRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create_rule(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<RuleInput>,
) -> AppResult<(StatusCode, Json<RuleRow>)> {
    Ok((
        StatusCode::CREATED,
        Json(allocations::create_rule(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(put, path = "/{business_id}/allocations/rules/{rule_id}", tag = "allocations",
    params(("business_id" = Uuid, Path), ("rule_id" = Uuid, Path)), request_body = RuleInput,
    responses((status = OK, body = RuleRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update_rule(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<RuleInput>,
) -> AppResult<Json<RuleRow>> {
    Ok(Json(
        allocations::update_rule(&state, &ctx, p.id, input).await?,
    ))
}

#[utoipa::path(post, path = "/{business_id}/allocations/rules/{rule_id}/archive", tag = "allocations",
    params(("business_id" = Uuid, Path), ("rule_id" = Uuid, Path)), request_body = ArchiveInput,
    responses((status = OK, body = RuleRow), (status = CONFLICT, body = ErrorBody)))]
async fn archive_rule(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ArchiveInput>,
) -> AppResult<Json<RuleRow>> {
    Ok(Json(
        allocations::archive_rule(&state, &ctx, p.id, input.archived).await?,
    ))
}

/// Sets the order the active rules apply in.
#[utoipa::path(put, path = "/{business_id}/allocations/rules/order", tag = "allocations", params(("business_id" = Uuid, Path)), request_body = OrderInput,
    responses((status = OK, body = Vec<RuleRow>), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn order_rules(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<OrderInput>,
) -> AppResult<Json<Vec<RuleRow>>> {
    Ok(Json(allocations::order_rules(&state, &ctx, input).await?))
}

/// What the active rules would plan for a receipt, without recording anything.
#[utoipa::path(post, path = "/{business_id}/allocations/preview", tag = "allocations", params(("business_id" = Uuid, Path)), request_body = PreviewInput,
    responses((status = OK, body = PlanView), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn preview(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<PreviewInput>,
) -> AppResult<Json<PlanView>> {
    Ok(Json(allocations::preview(&state, &ctx, input).await?))
}

// ─── Allocations ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AllocationStatus {
    Pending,
    Confirmed,
    Skipped,
    Reversed,
    Cancelled,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct AllocationQuery {
    #[param(inline)]
    pub status: Option<AllocationStatus>,
    /// Allocations with an entry for this destination.
    pub destination_id: Option<Uuid>,
    /// Received on or after.
    pub from: Option<NaiveDate>,
    /// Received on or before.
    pub to: Option<NaiveDate>,
    /// Allocations of one invoice payment.
    pub invoice_payment_id: Option<Uuid>,
    /// Allocations of one income record.
    pub income_id: Option<Uuid>,
}

#[utoipa::path(get, path = "/{business_id}/allocations", tag = "allocations", params(("business_id" = Uuid, Path), AllocationQuery, PageParams),
    responses((status = OK, body = AllocationPage)))]
async fn list(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<AllocationQuery>,
    ApiQuery(page): ApiQuery<PageParams>,
) -> AppResult<Json<AllocationPage>> {
    let filter = AllocationFilter {
        status: q.status.map(|s| {
            match s {
                AllocationStatus::Pending => "pending",
                AllocationStatus::Confirmed => "confirmed",
                AllocationStatus::Skipped => "skipped",
                AllocationStatus::Reversed => "reversed",
                AllocationStatus::Cancelled => "cancelled",
            }
            .to_owned()
        }),
        destination_id: q.destination_id,
        from: q.from,
        to: q.to,
        source: q
            .invoice_payment_id
            .map(SourceRef::InvoicePayment)
            .or(q.income_id.map(SourceRef::Income)),
    };
    let (items, total) =
        allocations::list(&state, &ctx, filter, page.limit(), page.offset()).await?;
    Ok(Json(AllocationPage {
        items,
        page: page.page.max(1),
        per_page: page.limit() as u32,
        total,
    }))
}

#[utoipa::path(get, path = "/{business_id}/allocations/balances", tag = "allocations", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = Balances)))]
async fn balances(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<Balances>> {
    Ok(Json(allocations::balances(&state, &ctx).await?))
}

#[utoipa::path(get, path = "/{business_id}/allocations/{allocation_id}", tag = "allocations",
    params(("business_id" = Uuid, Path), ("allocation_id" = Uuid, Path)),
    responses((status = OK, body = AllocationDetail), (status = NOT_FOUND, body = ErrorBody)))]
async fn get_allocation(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<AllocationDetail>> {
    Ok(Json(allocations::get(&state, &ctx, p.id).await?))
}

/// Totals for reviewed amounts, without recording them (for the review form).
#[utoipa::path(post, path = "/{business_id}/allocations/{allocation_id}/calculate", tag = "allocations",
    params(("business_id" = Uuid, Path), ("allocation_id" = Uuid, Path)), request_body = ReviewInput,
    responses((status = OK, body = PlanView), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn calculate(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ReviewInput>,
) -> AppResult<Json<PlanView>> {
    Ok(Json(
        allocations::calculate(&state, &ctx, p.id, input).await?,
    ))
}

/// Records the reviewed amounts for a pending (or skipped) allocation.
#[utoipa::path(post, path = "/{business_id}/allocations/{allocation_id}/confirm", tag = "allocations",
    params(("business_id" = Uuid, Path), ("allocation_id" = Uuid, Path)), request_body = ReviewInput,
    responses((status = OK, body = AllocationDetail), (status = UNPROCESSABLE_ENTITY, body = ErrorBody), (status = CONFLICT, body = ErrorBody)))]
async fn confirm(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ReviewInput>,
) -> AppResult<Json<AllocationDetail>> {
    Ok(Json(allocations::confirm(&state, &ctx, p.id, input).await?))
}

#[utoipa::path(post, path = "/{business_id}/allocations/{allocation_id}/skip", tag = "allocations",
    params(("business_id" = Uuid, Path), ("allocation_id" = Uuid, Path)),
    responses((status = OK, body = AllocationDetail), (status = CONFLICT, body = ErrorBody)))]
async fn skip(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<AllocationDetail>> {
    Ok(Json(
        allocations::set_skipped(&state, &ctx, p.id, true).await?,
    ))
}

/// Puts a skipped allocation back up for review.
#[utoipa::path(post, path = "/{business_id}/allocations/{allocation_id}/reopen", tag = "allocations",
    params(("business_id" = Uuid, Path), ("allocation_id" = Uuid, Path)),
    responses((status = OK, body = AllocationDetail), (status = CONFLICT, body = ErrorBody)))]
async fn reopen(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<AllocationDetail>> {
    Ok(Json(
        allocations::set_skipped(&state, &ctx, p.id, false).await?,
    ))
}

/// Reverses a confirmed allocation and returns the new one opened for review.
#[utoipa::path(post, path = "/{business_id}/allocations/{allocation_id}/reverse", tag = "allocations",
    params(("business_id" = Uuid, Path), ("allocation_id" = Uuid, Path)), request_body = ReasonInput,
    responses((status = OK, body = AllocationDetail), (status = CONFLICT, body = ErrorBody)))]
async fn reverse(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ReasonInput>,
) -> AppResult<Json<AllocationDetail>> {
    Ok(Json(
        allocations::reverse(&state, &ctx, p.id, input.reason).await?,
    ))
}

#[utoipa::path(get, path = "/{business_id}/allocations/spending", tag = "allocations", params(("business_id" = Uuid, Path), SpendingQuery),
    responses((status = OK, body = Vec<SpendingRow>)))]
async fn list_spending(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<SpendingQuery>,
) -> AppResult<Json<Vec<SpendingRow>>> {
    Ok(Json(allocations::list_spending(&state, &ctx, q).await?))
}

/// Records money used from a destination (recorded only: Solo never moves money).
#[utoipa::path(post, path = "/{business_id}/allocations/spending", tag = "allocations", params(("business_id" = Uuid, Path)), request_body = SpendingInput,
    responses((status = CREATED, body = SpendingRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn record_spending(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<SpendingInput>,
) -> AppResult<(StatusCode, Json<SpendingRow>)> {
    Ok((
        StatusCode::CREATED,
        Json(allocations::record_spending(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(delete, path = "/{business_id}/allocations/spending/{spending_id}", tag = "allocations",
    params(("business_id" = Uuid, Path), ("spending_id" = Uuid, Path)), request_body = ReasonInput,
    responses((status = NO_CONTENT), (status = NOT_FOUND, body = ErrorBody)))]
async fn delete_spending(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ReasonInput>,
) -> AppResult<StatusCode> {
    allocations::delete_spending(&state, &ctx, p.id, input.reason).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Opens allocations for review for money received in a period before any rule was active.
#[utoipa::path(post, path = "/{business_id}/allocations/backfill", tag = "allocations", params(("business_id" = Uuid, Path)), request_body = BackfillInput,
    responses((status = OK, body = BackfillResult), (status = CONFLICT, body = ErrorBody), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn backfill(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<BackfillInput>,
) -> AppResult<Json<BackfillResult>> {
    Ok(Json(allocations::backfill(&state, &ctx, input).await?))
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_destinations, create_destination))
        .routes(routes!(order_destinations))
        .routes(routes!(update_destination))
        .routes(routes!(archive_destination))
        .routes(routes!(list_rules, create_rule))
        .routes(routes!(order_rules))
        .routes(routes!(update_rule))
        .routes(routes!(archive_rule))
        .routes(routes!(preview))
        .routes(routes!(list))
        .routes(routes!(balances))
        .routes(routes!(list_spending, record_spending))
        .routes(routes!(delete_spending))
        .routes(routes!(backfill))
        .routes(routes!(get_allocation))
        .routes(routes!(calculate))
        .routes(routes!(confirm))
        .routes(routes!(skip))
        .routes(routes!(reopen))
        .routes(routes!(reverse))
}
