//! Student-facing order endpoints.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use cantina_application::orders::{CreateOrderCommand, OrderItemInput};
use cantina_core::catalog::ProductId;
use cantina_core::orders::OrderId;

use super::auth::{AuthenticatedSession, RequestId};
use crate::error::{ApiError, ErrorResponse};
use crate::state::AppState;

/// HTTP representation of an order line.
#[derive(Debug, Serialize, ToSchema)]
pub struct OrderItemResponse {
    product_id: String,
    product_name: String,
    /// Unit price in minor currency units.
    unit_price: u64,
    quantity: u32,
}

impl From<cantina_application::orders::OrderItemDto> for OrderItemResponse {
    fn from(dto: cantina_application::orders::OrderItemDto) -> Self {
        Self {
            product_id: dto.product_id.to_string(),
            product_name: dto.product_name,
            unit_price: dto.unit_price,
            quantity: dto.quantity,
        }
    }
}

/// HTTP representation of an order.
#[derive(Debug, Serialize, ToSchema)]
pub struct OrderResponse {
    id: String,
    status: String,
    /// Total in minor currency units.
    total: u64,
    pickup_code: String,
    student_id: String,
    items: Vec<OrderItemResponse>,
}

impl From<cantina_application::orders::OrderDto> for OrderResponse {
    fn from(dto: cantina_application::orders::OrderDto) -> Self {
        Self {
            id: dto.id.to_string(),
            status: dto.status.to_string(),
            total: dto.total,
            pickup_code: dto.pickup_code.to_string(),
            student_id: dto.student_id.to_string(),
            items: dto.items.into_iter().map(OrderItemResponse::from).collect(),
        }
    }
}

/// Envelope for order collection responses.
#[derive(Debug, Serialize, ToSchema)]
pub struct OrderListResponse {
    pub(crate) items: Vec<OrderResponse>,
}

/// Body for `POST /api/v1/orders`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateOrderRequest {
    items: Vec<OrderItemRequest>,
}

/// A client-provided product/quantity pair.
#[derive(Debug, Deserialize, ToSchema)]
pub struct OrderItemRequest {
    product_id: String,
    quantity: u32,
}

/// `POST /api/v1/orders`
#[utoipa::path(
    post,
    path = "/api/v1/orders",
    security(("auth_bearer" = [])),
    request_body = CreateOrderRequest,
    responses(
        (status = 201, description = "Order created", body = OrderResponse, headers(
            ("Location" = String, description = "Path of the created order"),
        )),
        (status = 400, description = "Invalid request body", body = ErrorResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "The caller is not a student", body = ErrorResponse),
        (status = 409, description = "Product unavailable or pickup-code conflict", body = ErrorResponse),
        (status = 422, description = "Order validation error", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    ),
)]
async fn create_order(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
    payload: Result<Json<CreateOrderRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(payload) = payload
        .map_err(|err| {
            tracing::debug!(detail = %err, "create order body rejected");
            ApiError::invalid_request_body()
        })
        .map_err(|err| err.with_request_id(request_id.clone()))?;

    let created = state
        .orders
        .create_order(
            &context.principal,
            CreateOrderCommand {
                items: payload
                    .items
                    .into_iter()
                    .map(|item| OrderItemInput {
                        product_id: ProductId::new(item.product_id),
                        quantity: item.quantity,
                    })
                    .collect(),
            },
        )
        .await
        .map_err(|err| ApiError::from(err).with_request_id(request_id))?;

    let id = created.id.to_string();
    let location = format!("/api/v1/orders/{id}");
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(OrderResponse::from(created)),
    )
        .into_response())
}

/// `GET /api/v1/orders`
#[utoipa::path(
    get,
    path = "/api/v1/orders",
    security(("auth_bearer" = [])),
    responses(
        (status = 200, description = "The authenticated student's orders", body = OrderListResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "The caller is not a student", body = ErrorResponse),
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
        .list_student_orders(&context.principal)
        .await
        .map(|orders| {
            Json(OrderListResponse {
                items: orders.into_iter().map(Into::into).collect(),
            })
        })
        .map_err(|err| ApiError::from(err).with_request_id(request_id))
}

/// `GET /api/v1/orders/{order_id}`
#[utoipa::path(
    get,
    path = "/api/v1/orders/{order_id}",
    security(("auth_bearer" = [])),
    params(("order_id" = String, Path, description = "Order identifier")),
    responses(
        (status = 200, description = "The requested order", body = OrderResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "The caller is not allowed to read the order", body = ErrorResponse),
        (status = 404, description = "Order not found", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    ),
)]
async fn get_order(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
) -> Result<Json<OrderResponse>, ApiError> {
    state
        .orders
        .get_student_order(&context.principal, OrderId::new(id))
        .await
        .map(|order| Json(order.into()))
        .map_err(|err| ApiError::from(err).with_request_id(request_id))
}

/// Prepares the student-facing orders router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_orders).post(create_order))
        .route("/{order_id}", get(get_order))
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024))
}
