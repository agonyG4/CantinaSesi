//! Health endpoint.

use axum::Json;
use serde::Serialize;

/// Health payload returned by `GET /health` and `GET /api/v1/health`.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

/// `GET /health` and `GET /api/v1/health`.
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy",
        service: "cantina-api",
        version: env!("CARGO_PKG_VERSION"),
    })
}
