//! `/api/v1/businesses/{business_id}/customers`

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use utoipa::IntoParams;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::extract::{ApiJson, ApiQuery, ItemPath, Page, PageParams};
use crate::context::BusinessContext;
use crate::error::{AppResult, ErrorBody};
use crate::repositories::customers::ListFilter;
use crate::repositories::invoices::EmailLogRow;
use crate::services::customers::{self, Customer, CustomerDetail, CustomerInput};
use crate::services::statements::{self, SendStatementInput};
use crate::state::AppState;

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CustomerQuery {
    /// Matches name, business name, contact person or email.
    pub search: Option<String>,
    /// `active` (default), `inactive` or `all`.
    pub status: Option<String>,
    /// `name` (default), `outstanding` or `created`.
    pub sort: Option<String>,
}

#[utoipa::path(get, path = "/{business_id}/customers", tag = "customers", params(("business_id" = Uuid, Path), CustomerQuery, PageParams),
    responses((status = OK, body = Page<Customer>)))]
async fn list(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<CustomerQuery>,
    ApiQuery(page): ApiQuery<PageParams>,
) -> AppResult<Json<Page<Customer>>> {
    let filter = ListFilter {
        search: q
            .search
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_owned()),
        active: match q.status.as_deref() {
            Some("all") => None,
            Some("inactive") => Some(false),
            _ => Some(true),
        },
        sort: q.sort.unwrap_or_else(|| "name".into()),
        limit: page.limit(),
        offset: page.offset(),
    };
    let (items, total) = customers::list(&state, &ctx, filter).await?;
    Ok(Json(Page::new(items, page, total)))
}

#[utoipa::path(post, path = "/{business_id}/customers", tag = "customers", params(("business_id" = Uuid, Path)), request_body = CustomerInput,
    responses((status = CREATED, body = CustomerDetail), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<CustomerInput>,
) -> AppResult<(StatusCode, Json<CustomerDetail>)> {
    Ok((
        StatusCode::CREATED,
        Json(customers::create(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(get, path = "/{business_id}/customers/{customer_id}", tag = "customers", params(("business_id" = Uuid, Path), ("customer_id" = Uuid, Path)),
    responses((status = OK, body = CustomerDetail), (status = NOT_FOUND, body = ErrorBody)))]
async fn get(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<CustomerDetail>> {
    let id = p.id;
    Ok(Json(customers::get(&state, &ctx, id).await?))
}

#[utoipa::path(put, path = "/{business_id}/customers/{customer_id}", tag = "customers", params(("business_id" = Uuid, Path), ("customer_id" = Uuid, Path)),
    request_body = CustomerInput, responses((status = OK, body = CustomerDetail), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<CustomerInput>,
) -> AppResult<Json<CustomerDetail>> {
    let id = p.id;
    Ok(Json(customers::update(&state, &ctx, id, input).await?))
}

/// Deletes a customer without records. Customers with invoices or other records can only be deactivated.
#[utoipa::path(delete, path = "/{business_id}/customers/{customer_id}", tag = "customers", params(("business_id" = Uuid, Path), ("customer_id" = Uuid, Path)),
    responses((status = NO_CONTENT), (status = CONFLICT, body = ErrorBody)))]
async fn delete(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<StatusCode> {
    let id = p.id;
    customers::delete(&state, &ctx, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The customer's statement emails, newest first.
#[utoipa::path(get, path = "/{business_id}/customers/{customer_id}/statement-emails", tag = "customers",
    params(("business_id" = Uuid, Path), ("customer_id" = Uuid, Path)),
    responses((status = OK, body = Vec<EmailLogRow>), (status = NOT_FOUND, body = ErrorBody)))]
async fn statement_emails(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<Vec<EmailLogRow>>> {
    Ok(Json(statements::history(&state, &ctx, p.id).await?))
}

/// Emails the customer their statement of account (PDF) for a period, through the owner's email.
#[utoipa::path(post, path = "/{business_id}/customers/{customer_id}/statement-emails", tag = "customers",
    params(("business_id" = Uuid, Path), ("customer_id" = Uuid, Path)), request_body = SendStatementInput,
    responses((status = OK, body = Vec<EmailLogRow>), (status = CONFLICT, body = ErrorBody), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn send_statement(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<SendStatementInput>,
) -> AppResult<Json<Vec<EmailLogRow>>> {
    Ok(Json(statements::send(&state, &ctx, p.id, input).await?))
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(get, update, delete))
        .routes(routes!(statement_emails, send_statement))
}
