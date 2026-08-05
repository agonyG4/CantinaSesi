//! HTTP adapter and application composition root for CantinaSesi.
//!
//! This crate translates between the outside world (HTTP) and the
//! application layer. It must not contain business rules, must not touch
//! storage directly and must not expose database models. It composes the
//! application services with the infrastructure adapters.
//!
//! Router construction is separated from process startup so the router can
//! be tested directly.

pub mod bootstrap;
pub mod error;
pub mod routes;
pub mod state;

use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;

use cantina_infrastructure::config::Config;

use state::AppState;

const REQUEST_ID_HEADER: &str = "x-request-id";

/// Builds the stateful router with all routes but no middleware.
pub fn build_router() -> Router<AppState> {
    Router::new()
        .route("/health", axum::routing::get(routes::health::health))
        .route(
            "/api/openapi.json",
            axum::routing::get(routes::openapi::openapi_json),
        )
        .nest("/api/v1", routes::api_v1())
}

/// Builds the complete application (routes + middleware) for a state.
pub fn app(state: AppState, config: &Config) -> Router<()> {
    build_router()
        .layer(PropagateRequestIdLayer::new(
            axum::http::header::HeaderName::from_static(REQUEST_ID_HEADER),
        ))
        .layer(SetRequestIdLayer::new(
            axum::http::header::HeaderName::from_static(REQUEST_ID_HEADER),
            MakeRequestUuid,
        ))
        .layer(TraceLayer::new_for_http())
        .layer(cors_layer(config.cors_allowed_origins()))
        .with_state(state)
}

/// Builds the CORS layer from the configured allow-list, falling back to a
/// permissive policy for development when no list is provided.
fn cors_layer(allowed_origins: Option<&[String]>) -> CorsLayer {
    match allowed_origins {
        Some(origins) => {
            let values: Vec<axum::http::header::HeaderValue> = origins
                .iter()
                .filter_map(|origin| origin.parse().ok())
                .collect();
            if values.is_empty() {
                CorsLayer::permissive()
            } else {
                CorsLayer::new()
                    .allow_origin(values)
                    .allow_methods([
                        axum::http::Method::GET,
                        axum::http::Method::POST,
                        axum::http::Method::PUT,
                        axum::http::Method::PATCH,
                        axum::http::Method::DELETE,
                        axum::http::Method::OPTIONS,
                    ])
                    .allow_headers([
                        axum::http::header::CONTENT_TYPE,
                        axum::http::header::AUTHORIZATION,
                        axum::http::header::HeaderName::from_static(REQUEST_ID_HEADER),
                    ])
            }
        }
        None => CorsLayer::permissive(),
    }
}
