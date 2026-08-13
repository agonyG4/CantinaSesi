//! Centralized HTTP error conversion.
//!
//! Application errors are translated into a standardized JSON error body.
//! Internal details (messages from storage backends, stack traces, password
//! hashes, tokens) are never leaked to clients.

use axum::Json;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use tower_http::request_id::RequestId;
use utoipa::ToSchema;

use cantina_application::ApplicationError;

/// Header returned on authentication failures.
pub(crate) const WWW_AUTHENTICATE_BEARER: &str = "Bearer";

/// Standardized error payload returned to clients.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ErrorBody {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    request_id: Option<String>,
}

/// Standardized error response envelope.
#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct ErrorResponse {
    error: ErrorBody,
}

/// HTTP error with a stable machine-readable code.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    body: ErrorBody,
    /// Whether the response must challenge the client with `WWW-Authenticate`.
    challenge: bool,
}

impl ApiError {
    /// Attaches the request identifier to the error body.
    pub fn with_request_id(mut self, request_id: Option<String>) -> Self {
        self.body.request_id = request_id;
        self
    }

    /// Marks the error as an authentication challenge that must return a
    /// `WWW-Authenticate: Bearer` header.
    pub fn with_bearer_challenge(mut self) -> Self {
        self.challenge = true;
        self
    }

    /// Builds a client error for an invalid query string.
    pub fn invalid_query(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            body: ErrorBody {
                code: "INVALID_QUERY",
                message: message.into(),
                request_id: None,
            },
            challenge: false,
        }
    }

    /// Builds the stable error for an invalid or oversized request body.
    pub fn invalid_request_body() -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            body: ErrorBody {
                code: "INVALID_REQUEST_BODY",
                message: "the request body is invalid".to_string(),
                request_id: None,
            },
            challenge: false,
        }
    }
}

impl From<ApplicationError> for ApiError {
    fn from(err: ApplicationError) -> Self {
        let (status, code, message, challenge) = match err {
            ApplicationError::NotFound { resource, id } => (
                StatusCode::NOT_FOUND,
                "NOT_FOUND",
                format!("the {resource} with id '{id}' was not found"),
                false,
            ),
            ApplicationError::ProductUnavailable { product_id } => (
                StatusCode::CONFLICT,
                "PRODUCT_UNAVAILABLE",
                format!("product '{product_id}' is not available"),
                false,
            ),
            ApplicationError::EmptyOrder => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "EMPTY_ORDER",
                "an order must contain at least one item".to_string(),
                false,
            ),
            ApplicationError::InvalidQuantity => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_QUANTITY",
                "item quantity must be greater than zero".to_string(),
                false,
            ),
            ApplicationError::TooManyOrderLines => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "TOO_MANY_ORDER_LINES",
                "an order contains too many distinct items".to_string(),
                false,
            ),
            ApplicationError::QuantityLimitExceeded => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "QUANTITY_LIMIT_EXCEEDED",
                "the quantity for a product exceeds the order limit".to_string(),
                false,
            ),
            ApplicationError::TotalQuantityLimitExceeded => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "TOTAL_QUANTITY_LIMIT_EXCEEDED",
                "the total order quantity exceeds the order limit".to_string(),
                false,
            ),
            ApplicationError::OrderTotalOverflow => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "ORDER_TOTAL_OVERFLOW",
                "the order total is too large".to_string(),
                false,
            ),
            ApplicationError::PickupCodeGenerationExhausted => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "PICKUP_CODE_GENERATION_FAILED",
                "internal server error".to_string(),
                false,
            ),
            ApplicationError::PickupCodeGenerationFailed => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "internal server error".to_string(),
                false,
            ),
            ApplicationError::InvalidStatusTransition { from, to } => (
                StatusCode::CONFLICT,
                "INVALID_STATUS_TRANSITION",
                format!("cannot transition order from '{from}' to '{to}'"),
                false,
            ),
            ApplicationError::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                "INVALID_CREDENTIALS",
                "invalid email or password".to_string(),
                true,
            ),
            ApplicationError::AccountInactive => (
                StatusCode::FORBIDDEN,
                "ACCOUNT_INACTIVE",
                "this account is inactive".to_string(),
                true,
            ),
            ApplicationError::AuthenticationRequired => (
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_REQUIRED",
                "authentication is required".to_string(),
                true,
            ),
            ApplicationError::InvalidAccessToken => (
                StatusCode::UNAUTHORIZED,
                "INVALID_ACCESS_TOKEN",
                "the access token is invalid".to_string(),
                true,
            ),
            ApplicationError::SessionExpired => (
                StatusCode::UNAUTHORIZED,
                "SESSION_EXPIRED",
                "the session has expired".to_string(),
                true,
            ),
            ApplicationError::Forbidden => (
                StatusCode::FORBIDDEN,
                "FORBIDDEN",
                "forbidden".to_string(),
                true,
            ),
            ApplicationError::InvalidProductName => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_PRODUCT_NAME",
                "a product name is required".to_string(),
                false,
            ),
            ApplicationError::ProductNameTooLong => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "PRODUCT_NAME_TOO_LONG",
                "the product name is too long".to_string(),
                false,
            ),
            ApplicationError::ProductDescriptionTooLong => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "PRODUCT_DESCRIPTION_TOO_LONG",
                "the product description is too long".to_string(),
                false,
            ),
            ApplicationError::InvalidProductPrice => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_PRODUCT_PRICE",
                "the product price must be greater than zero".to_string(),
                false,
            ),
            ApplicationError::InvalidCategoryName => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_CATEGORY_NAME",
                "a category name is required".to_string(),
                false,
            ),
            ApplicationError::CategoryNameTooLong => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "CATEGORY_NAME_TOO_LONG",
                "the category name is too long".to_string(),
                false,
            ),
            ApplicationError::EmptyUpdate => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "EMPTY_UPDATE",
                "the update is empty".to_string(),
                false,
            ),
            ApplicationError::CategoryNameConflict => (
                StatusCode::CONFLICT,
                "CATEGORY_NAME_CONFLICT",
                "a category with that name already exists".to_string(),
                false,
            ),
            ApplicationError::PasswordProcessing | ApplicationError::TokenGeneration => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "internal server error".to_string(),
                false,
            ),
            ApplicationError::NotImplemented { feature } => (
                StatusCode::NOT_IMPLEMENTED,
                "NOT_IMPLEMENTED",
                format!("{feature} is not implemented yet"),
                false,
            ),
            ApplicationError::Storage(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "INTERNAL_ERROR",
                "internal server error".to_string(),
                false,
            ),
        };
        Self {
            status,
            body: ErrorBody {
                code,
                message,
                request_id: None,
            },
            challenge,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(ErrorResponse { error: self.body })).into_response();
        if self.challenge
            && let Ok(value) = axum::http::HeaderValue::from_str(WWW_AUTHENTICATE_BEARER)
        {
            response
                .headers_mut()
                .insert(axum::http::header::WWW_AUTHENTICATE, value);
        }
        response
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

/// Extracts the request identifier from request parts.
pub fn extract_request_id_from_parts(parts: &axum::http::request::Parts) -> Option<String> {
    parts
        .extensions
        .get::<RequestId>()
        .and_then(|request_id| request_id.header_value().to_str().ok())
        .map(ToOwned::to_owned)
}
