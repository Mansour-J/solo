//! HTTP layer: routing, OpenAPI document, and the middleware stack. Handlers are thin: they extract
//! input, call a service and shape the response. Business rules never live here.

pub mod allocations;
pub mod annual;
pub mod business_health;
pub mod businesses;
pub mod customers;
pub mod expenses;
pub mod extract;
pub mod gst;
pub mod health;
pub mod insights;
pub mod invoices;
pub mod meta;
pub mod mileage;
pub mod recurring;
pub mod reminders;
pub mod reports;
pub mod users;

use axum::extract::DefaultBodyLimit;
use axum::http::{HeaderName, HeaderValue, Method, StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Json, Router, middleware};
use tower::ServiceBuilder;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::TraceLayer;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;
use utoipa_scalar::{Scalar, Servable};
use uuid::Uuid;

use crate::context::OwnerId;
use crate::error::AppError;
use crate::middleware::{
    API_KEY_HEADER, CLIENT_KIND_HEADER, require_api_key, require_client_header,
};
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    info(description = "REST API for bookkeeping, for one person: businesses, invoicing, expenses, mileage, allocations and reports. \
        Money amounts are integers in the currency's minor unit (cents). Dates are ISO 8601. \
        There is no sign-in: every request acts as the owner. Every state-changing request must send         `x-client-kind: web|desktop|mobile`, and the desktop app's local server also requires its `x-api-key`."),
    modifiers(&SecurityAddon),
    tags(
        (name = "health", description = "Liveness and readiness"),
        (name = "meta", description = "Branding and reference data"),
        (name = "users", description = "The owner's profile and the full data export"),
        (name = "businesses", description = "Businesses, settings and activity"),
        (name = "dashboard", description = "Financial overview from the ledger"),
        (name = "customers", description = "Customers and their balances"),
        (name = "invoices", description = "Invoices, payments, PDFs and email"),
        (name = "quotes", description = "Quotes and conversion to invoices"),
        (name = "expenses", description = "Expenses, categories and receipts"),
        (name = "income", description = "Income not tied to an invoice"),
        (name = "mileage", description = "Vehicles, mileage rates, trips and mileage reports"),
        (name = "allocations", description = "Allocation destinations, rules, allocations of money received and balances (planning only)"),
        (name = "gst", description = "Tax: GST returns, income tax and provisional tax, IRD due dates and what is owed"),
        (name = "reports", description = "Reports from the journal and receivables, as JSON or CSV, Excel and PDF"),
        (name = "insights", description = "Deterministic, explainable insights and dismissible notices"),
        (name = "annual", description = "Money in and out, fixed assets and depreciation, opening balances"),
    )
)]
struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "local_key",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new(API_KEY_HEADER))),
        );
    }
}

/// All routes with their OpenAPI descriptions.
pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest("/health", health::router())
        .nest("/api/v1/meta", meta::router())
        .nest("/api/v1/users/me", users::router())
        .nest("/api/v1/businesses", businesses::router())
        .nest("/api/v1/businesses", business_health::router())
        .nest("/api/v1/businesses", customers::router())
        .nest("/api/v1/businesses", invoices::router())
        .nest("/api/v1/businesses", expenses::router())
        .nest("/api/v1/businesses", mileage::router())
        .nest("/api/v1/businesses", allocations::router())
        .nest("/api/v1/businesses", reports::router())
        .nest("/api/v1/businesses", insights::router())
        .nest("/api/v1/businesses", annual::router())
        .nest("/api/v1/businesses", gst::router())
        .nest("/api/v1/businesses", recurring::router())
        .nest("/api/v1/businesses", reminders::router())
}

/// The OpenAPI document, titled with the configured product name.
pub fn openapi(state: &AppState) -> utoipa::openapi::OpenApi {
    let (_, doc) = routes().split_for_parts();
    finish_document(doc, state)
}

/// Titles the document with the product name and makes operation IDs unique (`<tag>_<handler>`),
/// which the generated TypeScript client relies on.
fn finish_document(
    mut doc: utoipa::openapi::OpenApi,
    state: &AppState,
) -> utoipa::openapi::OpenApi {
    doc.info.title = format!("{} API", state.config.brand.name);
    doc.info.version = env!("CARGO_PKG_VERSION").to_owned();
    for item in doc.paths.paths.values_mut() {
        let operations = [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
        ];
        for operation in operations.into_iter().flatten() {
            let tag = operation
                .tags
                .as_ref()
                .and_then(|t| t.first())
                .cloned()
                .unwrap_or_default();
            if let Some(id) = &operation.operation_id
                && !tag.is_empty()
                && !id.starts_with(&format!("{tag}_"))
            {
                operation.operation_id = Some(format!("{tag}_{id}"));
            }
        }
    }
    doc
}

async fn not_found() -> impl IntoResponse {
    AppError::NotFound("Endpoint")
}

/// The complete application, acting as `owner` (`services::users::ensure_owner`).
pub fn app(state: AppState, owner: Uuid) -> Router {
    router(state).layer(Extension(OwnerId(owner)))
}

/// The application with middleware but no owner: a layer must add an [`OwnerId`] to each request
/// (`app` does; the integration tests pick one per test).
pub fn router(state: AppState) -> Router {
    let (router, doc) = routes().split_for_parts();
    let doc = finish_document(doc, &state);
    let spec = doc.clone();
    let http = state.config.http.clone();

    let router = router
        .route("/api/openapi.json", get(move || async move { Json(spec) }))
        .merge(Scalar::with_url("/api/docs", doc))
        .fallback(not_found)
        .layer(middleware::from_fn(require_client_header))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_api_key,
        ))
        .layer(DefaultBodyLimit::max(http.max_body_bytes));

    let cors = (!http.cors_allowed_origins.is_empty()).then(|| {
        let origins: Vec<HeaderValue> = http
            .cors_allowed_origins
            .iter()
            .filter_map(|o| HeaderValue::from_str(o).ok())
            .collect();
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
            ])
            .allow_headers([
                header::CONTENT_TYPE,
                HeaderName::from_static(CLIENT_KIND_HEADER),
                HeaderName::from_static(API_KEY_HEADER),
            ])
            .expose_headers([
                HeaderName::from_static("x-request-id"),
                header::RETRY_AFTER,
                header::CONTENT_DISPOSITION,
            ])
    });

    let request_id = HeaderName::from_static("x-request-id");
    router
        .layer(
            ServiceBuilder::new()
                .layer(SetSensitiveRequestHeadersLayer::new([HeaderName::from_static(API_KEY_HEADER)]))
                .layer(SetRequestIdLayer::new(request_id.clone(), MakeRequestUuid))
                .layer(TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<_>| {
                    let id = request.headers().get("x-request-id").and_then(|v| v.to_str().ok()).unwrap_or("-");
                    tracing::info_span!("request", method = %request.method(), path = %request.uri().path(), request_id = %id)
                }))
                .layer(PropagateRequestIdLayer::new(request_id))
                .layer(TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, http.request_timeout))
                .option_layer(cors),
        )
        .with_state(state)
}
