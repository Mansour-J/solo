//! Request extractors and shared response types.

use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::error::AppError;

/// `axum::Json` whose rejections use the standard error body.
#[derive(Debug, FromRequest)]
#[from_request(via(axum::Json), rejection(AppError))]
pub struct ApiJson<T>(pub T);

impl From<JsonRejection> for AppError {
    fn from(rejection: JsonRejection) -> Self {
        match rejection {
            JsonRejection::JsonDataError(error) => {
                // serde reports the path of the offending field, e.g. "address.city: invalid type".
                let detail = error.body_text();
                let field = detail
                    .split(':')
                    .next()
                    .map(str::trim)
                    .filter(|f| !f.is_empty() && !f.contains(' '))
                    .unwrap_or("body");
                AppError::validation(field, "This value has the wrong type or format.")
            }
            JsonRejection::MissingJsonContentType(_) => {
                AppError::BadRequest("Send the request body as JSON.".into())
            }
            other => AppError::BadRequest(format!(
                "The request body could not be read: {}",
                other.body_text()
            )),
        }
    }
}

/// `axum::extract::Query` whose rejections use the standard error body.
#[derive(Debug, FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(AppError))]
pub struct ApiQuery<T>(pub T);

impl From<QueryRejection> for AppError {
    fn from(rejection: QueryRejection) -> Self {
        AppError::BadRequest(format!(
            "Invalid query parameters: {}",
            rejection.body_text()
        ))
    }
}

/// Page-based pagination parameters (`?page=2&per_page=50`).
#[derive(Debug, Clone, Copy, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PageParams {
    /// 1-based page number.
    #[serde(default = "default_page")]
    pub page: u32,
    /// Items per page (1–100).
    #[serde(default = "default_per_page")]
    pub per_page: u32,
}

fn default_page() -> u32 {
    1
}
fn default_per_page() -> u32 {
    25
}

impl PageParams {
    pub fn limit(&self) -> i64 {
        i64::from(self.per_page.clamp(1, 100))
    }

    pub fn offset(&self) -> i64 {
        i64::from(self.page.max(1) - 1) * self.limit()
    }
}

/// A page of results with the total count, for list endpoints.
#[derive(Debug, Serialize, ToSchema)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, params: PageParams, total: i64) -> Self {
        Self {
            items,
            page: params.page.max(1),
            per_page: params.limit() as u32,
            total,
        }
    }
}

/// Path of a record inside a business: `/{business_id}/<collection>/{<record>_id}`.
#[derive(Debug, Deserialize)]
pub struct ItemPath {
    #[allow(dead_code)]
    pub business_id: uuid::Uuid,
    #[serde(
        alias = "customer_id",
        alias = "invoice_id",
        alias = "quote_id",
        alias = "expense_id",
        alias = "income_id",
        alias = "category_id",
        alias = "vehicle_id",
        alias = "rate_id",
        alias = "trip_id",
        alias = "destination_id",
        alias = "rule_id",
        alias = "allocation_id",
        alias = "movement_id",
        alias = "asset_id",
        alias = "spending_id",
        alias = "recurring_id"
    )]
    pub id: uuid::Uuid,
}

/// Path of a child record: `/{business_id}/<collection>/{<record>_id}/<children>/{<child>_id}`.
#[derive(Debug, Deserialize)]
pub struct SubItemPath {
    #[allow(dead_code)]
    pub business_id: uuid::Uuid,
    #[serde(alias = "invoice_id", alias = "expense_id", alias = "rate_id")]
    pub id: uuid::Uuid,
    #[serde(alias = "payment_id", alias = "attachment_id", alias = "version_id")]
    pub sub_id: uuid::Uuid,
}
