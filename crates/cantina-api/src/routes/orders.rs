//! Order endpoints.

use axum::extract::{Path, Request, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use cantina_core::orders::OrderId;

use crate::error::{ApiError, extract_request_id};
use crate::state::AppState;

/// HTTP representation of an order line.
#[derive(Debug, Serialize)]
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
#[derive(Debug, Serialize)]
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
    Router::new().route("/{id}", get(get_order))
}
