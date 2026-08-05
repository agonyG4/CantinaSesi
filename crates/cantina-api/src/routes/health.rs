//! Health endpoints.

use axum::Json;
use serde::Serialize;
use utoipa::ToSchema;

/// Health payload returned by `GET /health` and `GET /api/v1/health`.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

fn payload() -> HealthResponse {
    HealthResponse {
        status: "healthy",
        service: "cantina-api",
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// `GET /health`
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "The service is healthy", body = HealthResponse),
    ),
)]
pub async fn health() -> Json<HealthResponse> {
    Json(payload())
}

/// `GET /api/v1/health`
#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses(
        (status = 200, description = "The service is healthy", body = HealthResponse),
    ),
)]
pub async fn versioned_health() -> Json<HealthResponse> {
    Json(payload())
}
