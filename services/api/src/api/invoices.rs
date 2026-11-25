//! `/api/v1/businesses/{business_id}/invoices` and `/quotes`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use super::extract::{ApiJson, ApiQuery, ItemPath, Page, PageParams, SubItemPath};
use crate::context::BusinessContext;
use crate::error::{AppResult, ErrorBody};
use crate::repositories::{invoices::ListFilter, quotes::ListFilter as QuoteFilter};
use crate::services::documents::{self, DocumentType, SendDocumentInput};
use crate::services::invoices::{
    self, CalculateInput, Calculation, Invoice, InvoiceInput, InvoiceSummary, PaymentInput,
    ReasonInput,
};
use crate::services::quotes::{self, Quote, QuoteInput, QuoteStatusInput, QuoteSummary};
use crate::state::AppState;

fn split_list(value: Option<String>) -> Option<Vec<String>> {
    value
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty())
}

fn trimmed(value: Option<String>) -> Option<String> {
    value.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct InvoiceQuery {
    /// Comma-separated: draft, issued, sent, partially_paid, paid, overdue, cancelled.
    pub status: Option<String>,
    pub customer_id: Option<Uuid>,
    /// Issue date from (inclusive).
    pub from: Option<NaiveDate>,
    /// Issue date to (inclusive).
    pub to: Option<NaiveDate>,
    /// Matches number, reference or customer name.
    pub search: Option<String>,
    /// issue_date, due_date, number, customer, status, total, balance (prefix `-` for descending).
    /// Default: newest first.
    pub sort: Option<String>,
}

#[utoipa::path(get, path = "/{business_id}/invoices", tag = "invoices", params(("business_id" = Uuid, Path), InvoiceQuery, PageParams),
    responses((status = OK, body = Page<InvoiceSummary>)))]
async fn list(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<InvoiceQuery>,
    ApiQuery(page): ApiQuery<PageParams>,
) -> AppResult<Json<Page<InvoiceSummary>>> {
    let filter = ListFilter {
        statuses: split_list(q.status),
        customer_id: q.customer_id,
        from: q.from,
        to: q.to,
        search: trimmed(q.search),
        sort: q.sort.unwrap_or_default(),
        limit: page.limit(),
        offset: page.offset(),
    };
    let (items, total) = invoices::list(&state, &ctx, filter).await?;
    Ok(Json(Page::new(items, page, total)))
}

/// Calculates lines and totals without saving (live preview while editing invoices and quotes).
#[utoipa::path(post, path = "/{business_id}/invoices/calculate", tag = "invoices", params(("business_id" = Uuid, Path)), request_body = CalculateInput,
    responses((status = OK, body = Calculation), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn calculate(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<CalculateInput>,
) -> AppResult<Json<Calculation>> {
    Ok(Json(invoices::calculate(&state, &ctx, input).await?))
}

/// Creates a draft invoice.
#[utoipa::path(post, path = "/{business_id}/invoices", tag = "invoices", params(("business_id" = Uuid, Path)), request_body = InvoiceInput,
    responses((status = CREATED, body = Invoice), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<InvoiceInput>,
) -> AppResult<(StatusCode, Json<Invoice>)> {
    Ok((
        StatusCode::CREATED,
        Json(invoices::create(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(get, path = "/{business_id}/invoices/{invoice_id}", tag = "invoices", params(("business_id" = Uuid, Path), ("invoice_id" = Uuid, Path)),
    responses((status = OK, body = Invoice), (status = NOT_FOUND, body = ErrorBody)))]
async fn get(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<Invoice>> {
    let id = p.id;
    Ok(Json(invoices::get(&state, &ctx, id).await?))
}

/// Updates a draft or issued invoice (issued invoices keep their number; changes are re-posted).
#[utoipa::path(put, path = "/{business_id}/invoices/{invoice_id}", tag = "invoices", params(("business_id" = Uuid, Path), ("invoice_id" = Uuid, Path)),
    request_body = InvoiceInput, responses((status = OK, body = Invoice), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<InvoiceInput>,
) -> AppResult<Json<Invoice>> {
    let id = p.id;
    Ok(Json(invoices::update(&state, &ctx, id, input).await?))
}

/// Deletes a draft. Issued invoices are cancelled instead.
#[utoipa::path(delete, path = "/{business_id}/invoices/{invoice_id}", tag = "invoices", params(("business_id" = Uuid, Path), ("invoice_id" = Uuid, Path)),
    responses((status = NO_CONTENT), (status = CONFLICT, body = ErrorBody)))]
async fn delete(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<StatusCode> {
    let id = p.id;
    invoices::delete_draft(&state, &ctx, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Issues a draft: assigns the next number and posts it to the ledger.
#[utoipa::path(post, path = "/{business_id}/invoices/{invoice_id}/issue", tag = "invoices", params(("business_id" = Uuid, Path), ("invoice_id" = Uuid, Path)),
    responses((status = OK, body = Invoice), (status = CONFLICT, body = ErrorBody)))]
async fn issue(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<Invoice>> {
    let id = p.id;
    Ok(Json(invoices::issue(&state, &ctx, id).await?))
}

/// Records that the invoice was sent outside the app.
#[utoipa::path(post, path = "/{business_id}/invoices/{invoice_id}/mark-sent", tag = "invoices", params(("business_id" = Uuid, Path), ("invoice_id" = Uuid, Path)),
    responses((status = OK, body = Invoice)))]
async fn mark_sent(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<Invoice>> {
    let id = p.id;
    Ok(Json(invoices::mark_sent(&state, &ctx, id).await?))
}

#[utoipa::path(post, path = "/{business_id}/invoices/{invoice_id}/cancel", tag = "invoices", params(("business_id" = Uuid, Path), ("invoice_id" = Uuid, Path)),
    request_body = ReasonInput, responses((status = OK, body = Invoice), (status = CONFLICT, body = ErrorBody)))]
async fn cancel(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<ReasonInput>,
) -> AppResult<Json<Invoice>> {
    let id = p.id;
    Ok(Json(invoices::cancel(&state, &ctx, id, input).await?))
}

#[utoipa::path(post, path = "/{business_id}/invoices/{invoice_id}/payments", tag = "invoices", params(("business_id" = Uuid, Path), ("invoice_id" = Uuid, Path)),
    request_body = PaymentInput, responses((status = CREATED, body = Invoice), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn add_payment(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<PaymentInput>,
) -> AppResult<(StatusCode, Json<Invoice>)> {
    let id = p.id;
    Ok((
        StatusCode::CREATED,
        Json(invoices::add_payment(&state, &ctx, id, input).await?),
    ))
}

#[utoipa::path(put, path = "/{business_id}/invoices/{invoice_id}/payments/{payment_id}", tag = "invoices",
    params(("business_id" = Uuid, Path), ("invoice_id" = Uuid, Path), ("payment_id" = Uuid, Path)), request_body = PaymentInput,
    responses((status = OK, body = Invoice), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update_payment(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<SubItemPath>,
    ApiJson(input): ApiJson<PaymentInput>,
) -> AppResult<Json<Invoice>> {
    let (id, payment_id) = (p.id, p.sub_id);
    Ok(Json(
        invoices::update_payment(&state, &ctx, id, payment_id, input).await?,
    ))
}

/// Removes a payment (kept in the history as voided; its ledger entries are reversed).
#[utoipa::path(delete, path = "/{business_id}/invoices/{invoice_id}/payments/{payment_id}", tag = "invoices",
    params(("business_id" = Uuid, Path), ("invoice_id" = Uuid, Path), ("payment_id" = Uuid, Path)), request_body = ReasonInput,
    responses((status = OK, body = Invoice)))]
async fn void_payment(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<SubItemPath>,
    ApiJson(input): ApiJson<ReasonInput>,
) -> AppResult<Json<Invoice>> {
    let (id, payment_id) = (p.id, p.sub_id);
    Ok(Json(
        invoices::void_payment(&state, &ctx, id, payment_id, input).await?,
    ))
}

fn pdf_response(doc: documents::RenderedDocument) -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "application/pdf".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{}\"", doc.filename),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        doc.bytes,
    )
}

#[utoipa::path(get, path = "/{business_id}/invoices/{invoice_id}/pdf", tag = "invoices", params(("business_id" = Uuid, Path), ("invoice_id" = Uuid, Path)),
    responses((status = OK, content_type = "application/pdf", body = Vec<u8>)))]
async fn invoice_pdf(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<impl IntoResponse> {
    let id = p.id;
    Ok(pdf_response(
        documents::render_document(&state, &ctx, DocumentType::Invoice, id).await?,
    ))
}

/// Emails the invoice PDF to the customer and records the delivery.
#[utoipa::path(post, path = "/{business_id}/invoices/{invoice_id}/send", tag = "invoices", params(("business_id" = Uuid, Path), ("invoice_id" = Uuid, Path)),
    request_body = SendDocumentInput, responses((status = OK, body = Invoice), (status = SERVICE_UNAVAILABLE, body = ErrorBody)))]
async fn send_invoice(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<SendDocumentInput>,
) -> AppResult<Json<Invoice>> {
    let id = p.id;
    documents::send_document(&state, &ctx, DocumentType::Invoice, id, input).await?;
    Ok(Json(invoices::get(&state, &ctx, id).await?))
}

// ─── Quotes ─────────────────────────────────────────────────────────────────

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct QuoteQuery {
    /// Comma-separated: draft, sent, accepted, rejected, expired, converted.
    pub status: Option<String>,
    pub customer_id: Option<Uuid>,
    pub search: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct Converted {
    pub invoice_id: Uuid,
}

#[utoipa::path(get, path = "/{business_id}/quotes", tag = "quotes", params(("business_id" = Uuid, Path), QuoteQuery, PageParams),
    responses((status = OK, body = Page<QuoteSummary>)))]
async fn list_quotes(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiQuery(q): ApiQuery<QuoteQuery>,
    ApiQuery(page): ApiQuery<PageParams>,
) -> AppResult<Json<Page<QuoteSummary>>> {
    let filter = QuoteFilter {
        statuses: split_list(q.status),
        customer_id: q.customer_id,
        search: trimmed(q.search),
        limit: page.limit(),
        offset: page.offset(),
    };
    let (items, total) = quotes::list(&state, &ctx, filter).await?;
    Ok(Json(Page::new(items, page, total)))
}

#[utoipa::path(post, path = "/{business_id}/quotes", tag = "quotes", params(("business_id" = Uuid, Path)), request_body = QuoteInput,
    responses((status = CREATED, body = Quote), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn create_quote(
    State(state): State<AppState>,
    ctx: BusinessContext,
    ApiJson(input): ApiJson<QuoteInput>,
) -> AppResult<(StatusCode, Json<Quote>)> {
    Ok((
        StatusCode::CREATED,
        Json(quotes::create(&state, &ctx, input).await?),
    ))
}

#[utoipa::path(get, path = "/{business_id}/quotes/{quote_id}", tag = "quotes", params(("business_id" = Uuid, Path), ("quote_id" = Uuid, Path)),
    responses((status = OK, body = Quote), (status = NOT_FOUND, body = ErrorBody)))]
async fn get_quote(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<Json<Quote>> {
    let id = p.id;
    Ok(Json(quotes::get(&state, &ctx, id).await?))
}

#[utoipa::path(put, path = "/{business_id}/quotes/{quote_id}", tag = "quotes", params(("business_id" = Uuid, Path), ("quote_id" = Uuid, Path)),
    request_body = QuoteInput, responses((status = OK, body = Quote), (status = UNPROCESSABLE_ENTITY, body = ErrorBody)))]
async fn update_quote(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<QuoteInput>,
) -> AppResult<Json<Quote>> {
    let id = p.id;
    Ok(Json(quotes::update(&state, &ctx, id, input).await?))
}

#[utoipa::path(delete, path = "/{business_id}/quotes/{quote_id}", tag = "quotes", params(("business_id" = Uuid, Path), ("quote_id" = Uuid, Path)),
    responses((status = NO_CONTENT), (status = CONFLICT, body = ErrorBody)))]
async fn delete_quote(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<StatusCode> {
    let id = p.id;
    quotes::delete(&state, &ctx, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Marks a quote as sent, accepted or rejected.
#[utoipa::path(post, path = "/{business_id}/quotes/{quote_id}/status", tag = "quotes", params(("business_id" = Uuid, Path), ("quote_id" = Uuid, Path)),
    request_body = QuoteStatusInput, responses((status = OK, body = Quote), (status = CONFLICT, body = ErrorBody)))]
async fn quote_status(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<QuoteStatusInput>,
) -> AppResult<Json<Quote>> {
    let id = p.id;
    Ok(Json(
        quotes::set_status(&state, &ctx, id, input.status).await?,
    ))
}

/// Creates a draft invoice from the quote, keeping its customer and lines.
#[utoipa::path(post, path = "/{business_id}/quotes/{quote_id}/convert", tag = "quotes", params(("business_id" = Uuid, Path), ("quote_id" = Uuid, Path)),
    responses((status = CREATED, body = Converted), (status = CONFLICT, body = ErrorBody)))]
async fn convert_quote(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<(StatusCode, Json<Converted>)> {
    let id = p.id;
    Ok((
        StatusCode::CREATED,
        Json(Converted {
            invoice_id: quotes::convert(&state, &ctx, id).await?,
        }),
    ))
}

#[utoipa::path(get, path = "/{business_id}/quotes/{quote_id}/pdf", tag = "quotes", params(("business_id" = Uuid, Path), ("quote_id" = Uuid, Path)),
    responses((status = OK, content_type = "application/pdf", body = Vec<u8>)))]
async fn quote_pdf(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
) -> AppResult<impl IntoResponse> {
    let id = p.id;
    Ok(pdf_response(
        documents::render_document(&state, &ctx, DocumentType::Quote, id).await?,
    ))
}

#[utoipa::path(post, path = "/{business_id}/quotes/{quote_id}/send", tag = "quotes", params(("business_id" = Uuid, Path), ("quote_id" = Uuid, Path)),
    request_body = SendDocumentInput, responses((status = OK, body = Quote), (status = SERVICE_UNAVAILABLE, body = ErrorBody)))]
async fn send_quote(
    State(state): State<AppState>,
    ctx: BusinessContext,
    Path(p): Path<ItemPath>,
    ApiJson(input): ApiJson<SendDocumentInput>,
) -> AppResult<Json<Quote>> {
    let id = p.id;
    documents::send_document(&state, &ctx, DocumentType::Quote, id, input).await?;
    Ok(Json(quotes::get(&state, &ctx, id).await?))
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, create))
        .routes(routes!(calculate))
        .routes(routes!(get, update, delete))
        .routes(routes!(issue))
        .routes(routes!(mark_sent))
        .routes(routes!(cancel))
        .routes(routes!(add_payment))
        .routes(routes!(update_payment, void_payment))
        .routes(routes!(invoice_pdf))
        .routes(routes!(send_invoice))
        .routes(routes!(list_quotes, create_quote))
        .routes(routes!(get_quote, update_quote, delete_quote))
        .routes(routes!(quote_status))
        .routes(routes!(convert_quote))
        .routes(routes!(quote_pdf))
        .routes(routes!(send_quote))
}
