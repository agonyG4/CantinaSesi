//! Centralized HTTP error conversion.
//!
//! Application errors are translated into a standardized JSON error body.
//! Internal details (messages from storage backends, stack traces) are never
//! leaked to clients.

use axum::Json;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tower_http::request_id::RequestId;

use cantina_application::ApplicationError;

/// Standardized error payload returned to clients.
#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
}

/// Standardized error response envelope.
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: ErrorBody,
}

/// HTTP error with a stable machine-readable code.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: ErrorBody,
}

impl ApiError {
    /// Attaches the request identifier to the error body.
    pub fn with_request_id(mut self, request_id: Option<String>) -> Self {
        self.body.request_id = request_id;
        self
    }
}

impl From<ApplicationError> for ApiError {
    fn from(err: ApplicationError) -> Self {
        let (status, code, message) = match err {
            ApplicationError::NotFound { resource, id } => (
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                format!("the {resource} with id '{id}' was not found"),
            ),
            ApplicationError::ProductUnavailable { product_id } => (
                StatusCode::CONFLICT,
                "PRODUCT_UNAVAILABLE",
                format!("product '{product_id}' is not available"),
            ),
            ApplicationError::EmptyOrder => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "EMPTY_ORDER",
                "an order must contain at least one item".to_string(),
            ),
            ApplicationError::InvalidQuantity => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_QUANTITY",
                "item quantity must be greater than zero".to_string(),
            ),
            ApplicationError::InvalidStatusTransition { from, to } => (
                StatusCode::CONFLICT,
                "INVALID_STATUS_TRANSITION",
                format!("cannot transition order from '{from}' to '{to}'"),
            ),
            ApplicationError::NotImplemented { feature } => (
                StatusCode::NOT_IMPLEMENTED,
                "NOT_IMPLEMENTED",
                format!("{feature} is not implemented yet"),
            ),
            ApplicationError::Storage(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "internal server error".to_string(),
            ),
        };
        Self {
            status,
            body: ErrorBody {
                code,
                message,
                request_id: None,
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status;
        (status, Json(ErrorResponse { error: self.body })).into_response()
    }
}

/// Extracts the request identifier set by the request-id middleware.
pub fn extract_request_id(request: &Request) -> Option<String> {
    request
        .extensions()
        .get::<RequestId>()
        .and_then(|request_id| request_id.header_value().to_str().ok())
        .map(ToOwned::to_owned)
}
