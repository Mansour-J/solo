//! Centralised error handling. Every failure leaves the API as
//! `{"error": {"code": "...", "message": "...", "details": {...}}}` with a matching HTTP status.
//! Internal errors are logged with their cause and returned without it.

use std::collections::BTreeMap;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

/// Field-level validation messages, keyed by the request field name (`line_items.0.quantity`).
#[derive(Debug, Default, Clone, Serialize, ToSchema, PartialEq, Eq)]
pub struct FieldErrors(pub BTreeMap<String, String>);

impl FieldErrors {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a message for a field; the first message for a field wins.
    pub fn add(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.0.entry(field.into()).or_insert_with(|| message.into());
    }

    pub fn check(&mut self, ok: bool, field: &str, message: &str) {
        if !ok {
            self.add(field, message);
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Converts into `Err(AppError::Validation)` when any field failed.
    pub fn into_result(self) -> Result<(), AppError> {
        if self.is_empty() {
            Ok(())
        } else {
            Err(AppError::Validation(self))
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("validation failed")]
    Validation(FieldErrors),
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    PayloadTooLarge(String),
    #[error("{0}")]
    ServiceUnavailable(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn validation(field: &str, message: &str) -> Self {
        let mut errors = FieldErrors::new();
        errors.add(field, message);
        Self::Validation(errors)
    }

    pub fn internal(error: impl Into<anyhow::Error>) -> Self {
        Self::Internal(error.into())
    }

    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "VALIDATION"),
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            Self::Forbidden(_) => (StatusCode::FORBIDDEN, "FORBIDDEN"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            Self::PayloadTooLarge(_) => (StatusCode::PAYLOAD_TOO_LARGE, "PAYLOAD_TOO_LARGE"),
            Self::ServiceUnavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, "SERVICE_UNAVAILABLE"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL"),
        }
    }

    /// A message safe to show the user (an internal error's cause is never included).
    pub fn public_message(&self) -> String {
        match self {
            Self::Validation(_) => "Some fields need attention.".into(),
            Self::NotFound(what) => format!("{what} not found."),
            Self::Internal(_) => "Something went wrong on our side. Please try again.".into(),
            other => other.to_string(),
        }
    }
}

/// Error body (documented in OpenAPI).
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorDetail {
    /// Stable machine-readable code, e.g. `VALIDATION`, `NOT_FOUND`, `CONFLICT`.
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<FieldErrors>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        if let Self::Internal(error) = &self {
            tracing::error!(error = ?error, "internal error");
        }
        let fields = match &self {
            Self::Validation(fields) => Some(fields.clone()),
            _ => None,
        };
        let body = ErrorBody {
            error: ErrorDetail {
                code: code.to_owned(),
                message: self.public_message(),
                fields,
            },
        };
        (status, axum::Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &error {
            // Constraint violations that slip past service validation become client errors, not 500s.
            match db.kind() {
                sqlx::error::ErrorKind::UniqueViolation => {
                    return Self::Conflict("That value is already in use.".into());
                }
                sqlx::error::ErrorKind::ForeignKeyViolation => {
                    return Self::BadRequest("A referenced record does not exist.".into());
                }
                sqlx::error::ErrorKind::CheckViolation => {
                    return Self::BadRequest(format!(
                        "A value is out of range ({}).",
                        db.constraint().unwrap_or("check")
                    ));
                }
                _ => {}
            }
        }
        Self::Internal(error.into())
    }
}

pub type AppResult<T> = Result<T, AppError>;
