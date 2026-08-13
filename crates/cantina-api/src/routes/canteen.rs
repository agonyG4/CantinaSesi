//! Cafeteria order queue and status endpoints.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::Deserialize;
use utoipa::ToSchema;

use cantina_core::orders::{OrderId, OrderStatus};

use super::auth::{AuthenticatedSession, RequestId};
use super::orders::{OrderListResponse, OrderResponse};
use crate::error::{ApiError, ErrorResponse};
use crate::state::AppState;

/// Body for `PATCH /api/v1/canteen/orders/{order_id}/status`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChangeOrderStatusRequest {
    /// Target status. Valid values are pending, confirmed, preparing, ready,
    /// delivered, and cancelled; the domain still validates transitions.
    status: String,
}

/// `GET /api/v1/canteen/orders`
#[utoipa::path(
    get,
    path = "/api/v1/canteen/orders",
    security(("auth_bearer" = [])),
    responses(
        (status = 200, description = "Active cafeteria order queue", body = OrderListResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "Canteen or admin role required", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    ),
)]
async fn list_orders(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
) -> Result<Json<OrderListResponse>, ApiError> {
    state
        .orders
        .list_canteen_orders(&context.principal)
        .await
        .map(|orders| {
            Json(OrderListResponse {
                items: orders.into_iter().map(Into::into).collect(),
            })
        })
        .map_err(|err| ApiError::from(err).with_request_id(request_id))
}

/// `GET /api/v1/canteen/orders/{order_id}`
#[utoipa::path(
    get,
    path = "/api/v1/canteen/orders/{order_id}",
    security(("auth_bearer" = [])),
    params(("order_id" = String, Path, description = "Order identifier")),
    responses(
        (status = 200, description = "The requested cafeteria order", body = OrderResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "Canteen or admin role required", body = ErrorResponse),
        (status = 404, description = "Order not found", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    ),
)]
async fn get_order(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
    Path(id): Path<String>,
) -> Result<Json<OrderResponse>, ApiError> {
    state
        .orders
        .get_canteen_order(&context.principal, OrderId::new(id))
        .await
        .map(|order| Json(order.into()))
        .map_err(|err| ApiError::from(err).with_request_id(request_id))
}

/// `PATCH /api/v1/canteen/orders/{order_id}/status`
#[utoipa::path(
    patch,
    path = "/api/v1/canteen/orders/{order_id}/status",
    security(("auth_bearer" = [])),
    params(("order_id" = String, Path, description = "Order identifier")),
    request_body = ChangeOrderStatusRequest,
    responses(
        (status = 200, description = "Order status updated", body = OrderResponse),
        (status = 400, description = "Invalid request body or status", body = ErrorResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "Canteen or admin role required", body = ErrorResponse),
        (status = 404, description = "Order not found", body = ErrorResponse),
        (status = 409, description = "Invalid domain transition", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    ),
)]
async fn update_status(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
    Path(id): Path<String>,
    payload: Result<Json<ChangeOrderStatusRequest>, JsonRejection>,
) -> Result<Json<OrderResponse>, ApiError> {
    let Json(payload) = payload
        .map_err(|err| {
            tracing::debug!(detail = %err, "order status body rejected");
            ApiError::invalid_request_body()
        })
        .map_err(|err| err.with_request_id(request_id.clone()))?;
    let target = payload
        .status
        .parse::<OrderStatus>()
        .map_err(|_| ApiError::invalid_request_body())
        .map_err(|err| err.with_request_id(request_id.clone()))?;

    state
        .orders
        .change_status(&context.principal, OrderId::new(id), target)
        .await
        .map(|order| Json(order.into()))
        .map_err(|err| ApiError::from(err).with_request_id(request_id))
}

/// Prepares the cafeteria operations router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orders", get(list_orders))
        .route("/orders/{order_id}", get(get_order))
        .route("/orders/{order_id}/status", patch(update_status))
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024))
}
