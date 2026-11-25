//! `/api/v1/businesses/{business_id}/expenses`, `/expense-categories`, `/income` and `/dashboard`.

use axum::Json;
use axum::extract::{Multipart, Path, State};
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
use crate::error::{AppError, AppResult, ErrorBody};
use crate::repositories::expenses::{CategoryRow, ExpenseFilter, IncomeRow};
use crate::services::dashboard::{self, Dashboard, DashboardQuery};
use crate::services::expenses::{
    self, CategoryArchiveInput, CategoryInput, ExpenseCalculation, ExpenseCalculationInput,
    ExpenseDetail, ExpenseInput, ExpensePage,
};
use crate::services::income::{self, IncomeInput, IncomePage};
use crate::services::invoices::ReasonInput;
use crate::services::key_dates::{self, KeyDates};
use crate::state::AppState;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ExpenseQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub category_id: Option<Uuid>,
    /// Matches supplier, description or payment reference.
    pub search: Option<String>,
    /// Minimum gross amount (minor units).
    pub min_amount: Option<i64>,
    pub max_amount: Option<i64>,
    /// standard, zero_rated, exempt or no_gst.
    pub gst_treatment: Option<String>,
    pub has_receipt: Option<bool>,
    /// business, personal, partial or rechargeable.
    pub classification: Option<String>,
    /// Include deleted (voided) expenses.
    #[serde(default)]
    pub include_deleted: bool,
}

#[utoipa::path(get, path = "/{business_id}/expenses", tag = "expenses", params(("business_id" = Uuid, Path), ExpenseQuery, PageParams),
    responses((status = OK, body = ExpensePage)))]
async fn list(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<ExpenseQuery>,
    ApiQuery(page): ApiQuery<PageParams>,
) -> AppResult<Json<ExpensePage>> {
    let filter = ExpenseFilter {
        from: q.from,
        to: q.to,
        category_id: q.category_id,
        search: q
            .search
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty()),
        min_amount: q.min_amount,
        max_amount: q.max_amount,
        gst_treatment: q.gst_treatment.filter(|s| !s.is_empty()),
        has_receipt: q.has_receipt,
        classification: q.classification.filter(|s| !s.is_empty()),
        include_voided: q.include_deleted,
        limit: page.limit(),
        offset: page.offset(),
    };
    Ok(Json(
        expenses::list(&state, &ctx, filter, page.page.max(1)).await?,
    ))
}

#[utoipa::path(post, path = "/{business_id}/expenses", tag = "expenses", params(("business_id" = Uuid, Path)), request_body = ExpenseInput,
    responses((status = CREATED, body = ExpenseDetail), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<ExpenseInput>,
) -> AppResult<(StatusCode, Json<ExpenseDetail>)> {
    Ok((
        StatusCode::CREATED,
        Json(expenses::create(&state, &ctx, input).await?),
    ))
}

/// Works out how an expense would be booked (GST, business and private shares) without saving it.
#[utoipa::path(post, path = "/{business_id}/expenses/calculate", tag = "expenses", params(("business_id" = Uuid, Path)), request_body = ExpenseCalculationInput,
    responses((status = OK, body = ExpenseCalculation), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn calculate(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<ExpenseCalculationInput>,
) -> AppResult<Json<ExpenseCalculation>> {
    Ok(Json(expenses::calculate(&state, &ctx, input).await?))
}

#[utoipa::path(get, path = "/{business_id}/expenses/{expense_id}", tag = "expenses", params(("business_id" = Uuid, Path), ("expense_id" = Uuid, Path)),
    responses((status = OK, body = ExpenseDetail), (status = NOT_FOUND, body = ErrorBody)))]
async fn get(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<ExpenseDetail>> {
    let id = p.id;
    Ok(Json(expenses::get(&state, &ctx, id).await?))
}

#[utoipa::path(put, path = "/{business_id}/expenses/{expense_id}", tag = "expenses", params(("business_id" = Uuid, Path), ("expense_id" = Uuid, Path)),
    request_body = ExpenseInput, responses((status = OK, body = ExpenseDetail), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ExpenseInput>,
) -> AppResult<Json<ExpenseDetail>> {
    let id = p.id;
    Ok(Json(expenses::update(&state, &ctx, id, input).await?))
}

/// Deletes an expense from the books (kept as voided in the history; its ledger entry is reversed).
#[utoipa::path(delete, path = "/{business_id}/expenses/{expense_id}", tag = "expenses", params(("business_id" = Uuid, Path), ("expense_id" = Uuid, Path)),
    request_body = ReasonInput, responses((status = NO_CONTENT)))]
async fn delete(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ReasonInput>,
) -> AppResult<StatusCode> {
    let id = p.id;
    expenses::void(&state, &ctx, id, input.reason).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Attaches a receipt (multipart field `file`: PDF, JPEG, PNG or WebP up to 10 MB).
#[utoipa::path(post, path = "/{business_id}/expenses/{expense_id}/attachments", tag = "expenses",
    params(("business_id" = Uuid, Path), ("expense_id" = Uuid, Path)), request_body(content_type = "multipart/form-data", content = Object),
    responses((status = CREATED, body = ExpenseDetail), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn add_attachment(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<ExpenseDetail>)> {
    let id = p.id;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.body_text()))?
    {
        if field.name() == Some("file") {
            let filename = field.file_name().unwrap_or("receipt").to_owned();
            let bytes = field
                .bytes()
                .await
                .map_err(|e| AppError::BadRequest(e.body_text()))?;
            return Ok((
                StatusCode::CREATED,
                Json(expenses::add_attachment(&state, &ctx, id, filename, bytes.to_vec()).await?),
            ));
        }
    }
    Err(AppError::validation("file", "Choose a file to attach."))
}

#[utoipa::path(get, path = "/{business_id}/expenses/{expense_id}/attachments/{attachment_id}", tag = "expenses",
    params(("business_id" = Uuid, Path), ("expense_id" = Uuid, Path), ("attachment_id" = Uuid, Path)),
    responses((status = OK, content_type = "application/octet-stream", body = Vec<u8>)))]
async fn get_attachment(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<SubItemPath>,
) -> AppResult<impl IntoResponse> {
    let (id, attachment_id) = (p.id, p.sub_id);
    let (row, bytes) = expenses::attachment(&state, &ctx, id, attachment_id).await?;
    let filename: String = row
        .filename
        .chars()
        .map(|c| if c == '"' || c == '\\' { '_' } else { c })
        .collect();
    Ok((
        [
            (header::CONTENT_TYPE, row.content_type),
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{filename}\""),
            ),
            (header::CACHE_CONTROL, "private, max-age=300".to_owned()),
            (
                header::HeaderName::from_static("x-content-type-options"),
                "nosniff".to_owned(),
            ),
        ],
        bytes,
    ))
}

#[utoipa::path(delete, path = "/{business_id}/expenses/{expense_id}/attachments/{attachment_id}", tag = "expenses",
    params(("business_id" = Uuid, Path), ("expense_id" = Uuid, Path), ("attachment_id" = Uuid, Path)), responses((status = NO_CONTENT)))]
async fn delete_attachment(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<SubItemPath>,
) -> AppResult<StatusCode> {
    let (id, attachment_id) = (p.id, p.sub_id);
    expenses::delete_attachment(&state, &ctx, id, attachment_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ─── Categories ─────────────────────────────────────────────────────────────

#[utoipa::path(get, path = "/{business_id}/expense-categories", tag = "expenses", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = Vec<CategoryRow>)))]
async fn categories(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<Vec<CategoryRow>>> {
    Ok(Json(expenses::categories(&state, &ctx).await?))
}

#[utoipa::path(post, path = "/{business_id}/expense-categories", tag = "expenses", params(("business_id" = Uuid, Path)), request_body = CategoryInput,
    responses((status = CREATED, body = CategoryRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create_category(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<CategoryInput>,
) -> AppResult<(StatusCode, Json<CategoryRow>)> {
    Ok((
        StatusCode::CREATED,
        Json(expenses::create_category(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(put, path = "/{business_id}/expense-categories/{category_id}", tag = "expenses", params(("business_id" = Uuid, Path), ("category_id" = Uuid, Path)),
    request_body = CategoryInput, responses((status = OK, body = CategoryRow)))]
async fn update_category(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<CategoryInput>,
) -> AppResult<Json<CategoryRow>> {
    let id = p.id;
    Ok(Json(
        expenses::update_category(&state, &ctx, id, input).await?,
    ))
}

/// Archives (hides from new expenses) or restores a category. Categories are never deleted.
#[utoipa::path(post, path = "/{business_id}/expense-categories/{category_id}/archive", tag = "expenses",
    params(("business_id" = Uuid, Path), ("category_id" = Uuid, Path)), request_body = CategoryArchiveInput,
    responses((status = OK, body = CategoryRow)))]
async fn archive_category(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<CategoryArchiveInput>,
) -> AppResult<Json<CategoryRow>> {
    let id = p.id;
    Ok(Json(
        expenses::archive_category(&state, &ctx, id, input.archived).await?,
    ))
}

// ─── Income ─────────────────────────────────────────────────────────────────

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct IncomeQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub search: Option<String>,
}

#[utoipa::path(get, path = "/{business_id}/income", tag = "income", params(("business_id" = Uuid, Path), IncomeQuery, PageParams),
    responses((status = OK, body = IncomePage)))]
async fn list_income(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<IncomeQuery>,
    ApiQuery(page): ApiQuery<PageParams>,
) -> AppResult<Json<IncomePage>> {
    let search = q
        .search
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    Ok(Json(
        income::list(&state, &ctx, q.from, q.to, search, page.page, page.limit()).await?,
    ))
}

#[utoipa::path(post, path = "/{business_id}/income", tag = "income", params(("business_id" = Uuid, Path)), request_body = IncomeInput,
    responses((status = CREATED, body = IncomeRow), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create_income(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<IncomeInput>,
) -> AppResult<(StatusCode, Json<IncomeRow>)> {
    Ok((
        StatusCode::CREATED,
        Json(income::create(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(get, path = "/{business_id}/income/{income_id}", tag = "income", params(("business_id" = Uuid, Path), ("income_id" = Uuid, Path)),
    responses((status = OK, body = IncomeRow)))]
async fn get_income(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<IncomeRow>> {
    let id = p.id;
    Ok(Json(income::get(&state, &ctx, id).await?))
}

#[utoipa::path(put, path = "/{business_id}/income/{income_id}", tag = "income", params(("business_id" = Uuid, Path), ("income_id" = Uuid, Path)),
    request_body = IncomeInput, responses((status = OK, body = IncomeRow)))]
async fn update_income(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<IncomeInput>,
) -> AppResult<Json<IncomeRow>> {
    let id = p.id;
    Ok(Json(income::update(&state, &ctx, id, input).await?))
}

#[utoipa::path(delete, path = "/{business_id}/income/{income_id}", tag = "income", params(("business_id" = Uuid, Path), ("income_id" = Uuid, Path)),
    request_body = ReasonInput, responses((status = NO_CONTENT)))]
async fn delete_income(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ReasonInput>,
) -> AppResult<StatusCode> {
    let id = p.id;
    income::void(&state, &ctx, id, input.reason).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ─── Dashboard ──────────────────────────────────────────────────────────────

#[utoipa::path(get, path = "/{business_id}/dashboard", tag = "dashboard", params(("business_id" = Uuid, Path), DashboardQuery),
    responses((status = OK, body = Dashboard)))]
async fn get_dashboard(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<DashboardQuery>,
) -> AppResult<Json<Dashboard>> {
    Ok(Json(dashboard::dashboard(&state, &ctx, q).await?))
}

/// Upcoming key dates (GST returns, provisional and terminal tax, the IR3 return and the end of the
/// financial year) with the days left, for the dashboard.
#[utoipa::path(get, path = "/{business_id}/key-dates", tag = "dashboard", params(("business_id" = Uuid, Path)),
    responses((status = OK, body = KeyDates)))]
async fn get_key_dates(
    State(state): State<AppState>,
    ctx: BusinessContext,
) -> AppResult<Json<KeyDates>> {
    Ok(Json(key_dates::upcoming(&state, &ctx).await?))
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(calculate))
        .routes(routes!(get, update, delete))
        .routes(routes!(add_attachment))
        .routes(routes!(get_attachment, delete_attachment))
        .routes(routes!(categories, create_category))
        .routes(routes!(update_category))
        .routes(routes!(archive_category))
        .routes(routes!(list_income, create_income))
        .routes(routes!(get_income, update_income, delete_income))
        .routes(routes!(get_dashboard))
        .routes(routes!(get_key_dates))
}
