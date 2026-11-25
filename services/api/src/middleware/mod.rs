//! Cross-cutting HTTP concerns: request metadata, the client header check and the local server's key.

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::error::AppError;
use crate::state::AppState;

/// Header every API client sends with its kind (`web`, `desktop`, `mobile`). Browsers can't add custom
/// headers to cross-site form posts without a CORS preflight, so requiring it on state-changing
/// requests stops other web pages from changing the books.
pub const CLIENT_KIND_HEADER: &str = "x-client-kind";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    Web,
    Desktop,
    Mobile,
}

impl ClientKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Desktop => "desktop",
            Self::Mobile => "mobile",
        }
    }

    fn from_headers(headers: &HeaderMap) -> Self {
        match headers
            .get(CLIENT_KIND_HEADER)
            .and_then(|v| v.to_str().ok())
        {
            Some("desktop") => Self::Desktop,
            Some("mobile") => Self::Mobile,
            _ => Self::Web,
        }
    }
}

/// Facts about the caller used for audit entries.
#[derive(Debug, Clone)]
pub struct RequestMeta {
    pub ip: Option<String>,
    pub user_agent: String,
    pub request_id: Option<String>,
    pub client: ClientKind,
}

impl FromRequestParts<AppState> for RequestMeta {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self {
            ip: client_ip(parts, state.config.http.trust_proxy),
            user_agent: parts
                .headers
                .get(axum::http::header::USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .map(|v| v.chars().take(512).collect())
                .unwrap_or_default(),
            request_id: parts
                .headers
                .get("x-request-id")
                .and_then(|v| v.to_str().ok())
                .map(|v| v.chars().take(64).collect()),
            client: ClientKind::from_headers(&parts.headers),
        })
    }
}

fn client_ip(parts: &Parts, trust_proxy: bool) -> Option<String> {
    if trust_proxy && let Some(ip) = parts.headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        return Some(ip.trim().chars().take(64).collect());
    }
    parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip().to_string())
}

/// Rejects state-changing API requests that lack the client-kind header (see [`CLIENT_KIND_HEADER`]).
pub async fn require_client_header(request: Request, next: Next) -> Response {
    let safe = matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    );
    if !safe && !request.headers().contains_key(CLIENT_KIND_HEADER) {
        return AppError::Forbidden(format!("Missing {CLIENT_KIND_HEADER} header."))
            .into_response();
    }
    next.run(request).await
}

/// Header carrying the local server's key (see `HttpConfig::api_key`).
pub const API_KEY_HEADER: &str = "x-api-key";

/// Rejects API requests without the configured key. Health checks stay open.
pub async fn require_api_key(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if let Some(expected) = &state.config.http.api_key
        && request.uri().path().starts_with("/api/")
        && request.method() != Method::OPTIONS
    {
        let presented = request.headers().get(API_KEY_HEADER).map(|v| v.as_bytes());
        if !presented.is_some_and(|p| constant_time_eq(p, expected.as_bytes())) {
            return AppError::Forbidden("This server only answers the app that started it.".into())
                .into_response();
        }
    }
    next.run(request).await
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
