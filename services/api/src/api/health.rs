//! Liveness and readiness probes (Docker health checks, load balancers).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::state::AppState;

#[derive(Serialize, ToSchema)]
pub struct Health {
    pub status: &'static str,
    pub version: &'static str,
}

/// The process is running.
#[utoipa::path(get, path = "/live", tag = "health", responses((status = OK, body = Health)))]
async fn live() -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// The API can serve requests (the database answers).
#[utoipa::path(get, path = "/ready", tag = "health", responses((status = OK, body = Health), (status = SERVICE_UNAVAILABLE, body = Health)))]
async fn ready(State(state): State<AppState>) -> (StatusCode, Json<Health>) {
    match state.db.ping().await {
        Ok(()) => (
            StatusCode::OK,
            Json(Health {
                status: "ok",
                version: env!("CARGO_PKG_VERSION"),
            }),
        ),
        Err(error) => {
            tracing::warn!(%error, "readiness check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(Health {
                    status: "database_unavailable",
                    version: env!("CARGO_PKG_VERSION"),
                }),
            )
        }
    }
}

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(live))
        .routes(routes!(ready))
}
