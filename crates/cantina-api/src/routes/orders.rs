//! Order endpoints.

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use cantina_application::auth::AuthorizationService;
use cantina_application::orders::{CreateOrderCommand, OrderItemInput};
use cantina_core::orders::OrderId;
use cantina_core::catalog::ProductId;
use cantina_core::users::UserRole;

use super::auth::{AuthenticatedSession, RequestId};
use crate::error::{ApiError, ErrorResponse, extract_request_id};
use crate::state::AppState;

/// HTTP representation of an order line.
#[derive(Debug, Serialize, ToSchema)]
pub struct OrderItemResponse {
    product_id: String,
    product_name: String,
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
    /// Total in minor units.
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

/// A product line submitted by a student at checkout.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateOrderItemRequest {
    pub product_id: String,
    pub quantity: u32,
}

/// Body for `POST /api/v1/orders`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateOrderRequest {
    pub items: Vec<CreateOrderItemRequest>,
}

/// `POST /api/v1/orders`
#[utoipa::path(
    post,
    path = "/api/v1/orders",
    security(("auth_bearer" = [])),
    request_body = CreateOrderRequest,
    responses(
        (status = 201, description = "Order sent to the cafeteria", body = OrderResponse, headers(
            ("Location" = String, description = "Path of the created order"),
        )),
        (status = 400, description = "Invalid request body", body = ErrorResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "Only student accounts may create orders", body = ErrorResponse),
        (status = 409, description = "A product is unavailable", body = ErrorResponse),
        (status = 422, description = "The order or quantity is invalid", body = ErrorResponse),
    ),
)]
async fn create_order(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
    payload: Result<Json<CreateOrderRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    AuthorizationService::require_any_role(&context.principal, &[UserRole::Student])
        .map_err(|err| ApiError::from(err).with_request_id(request_id.clone()))?;

    let Json(payload) = payload
        .map_err(|_| ApiError::invalid_request_body())
        .map_err(|err| err.with_request_id(request_id.clone()))?;

    let order = state
        .orders
        .create_order(CreateOrderCommand {
            student_id: context.principal.user_id().clone(),
            items: payload
                .items
                .into_iter()
                .map(|item| OrderItemInput {
                    product_id: ProductId::new(item.product_id),
                    quantity: item.quantity,
                })
                .collect(),
        })
        .await
        .map_err(|err| ApiError::from(err).with_request_id(request_id))?;

    let id = order.id.to_string();
    let location = format!("/api/v1/orders/{id}");
    Ok((
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(OrderResponse::from(order)),
    )
        .into_response())
}

/// `GET /api/v1/orders/{id}`
async fn get_order(
    State(state): State<AppState>,
    Path(id): Path<String>,
    request: Request,
) -> Result<Json<OrderResponse>, ApiError> {
    state
        .orders
        .get_order(OrderId::new(id))
        .await
        .map(|order| Json(order.into()))
        .map_err(|err| ApiError::from(err).with_request_id(extract_request_id(&request)))
}

/// Prepares the orders router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", post(create_order))
        .route("/{id}", get(get_order))
        .layer(axum::extract::DefaultBodyLimit::max(32 * 1024))
}
