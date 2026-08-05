//! Product endpoints.

use axum::extract::{Path, Request, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use cantina_core::catalog::ProductId;

use crate::error::{ApiError, extract_request_id};
use crate::state::AppState;

/// HTTP representation of a product.
#[derive(Debug, Serialize)]
pub struct ProductResponse {
    id: String,
    name: String,
    description: String,
    /// Price in minor units.
    price: u64,
    category_id: String,
    available: bool,
}

impl From<cantina_application::catalog::ProductDto> for ProductResponse {
    fn from(dto: cantina_application::catalog::ProductDto) -> Self {
        Self {
            id: dto.id.to_string(),
            name: dto.name,
            description: dto.description,
            price: dto.price,
            category_id: dto.category_id.to_string(),
            available: dto.available,
        }
    }
}

/// `GET /api/v1/products/{id}`
async fn get_product(
    State(state): State<AppState>,
    Path(id): Path<String>,
    request: Request,
) -> Result<Json<ProductResponse>, ApiError> {
    state
        .catalog
        .get_product(ProductId::new(id))
        .await
        .map(|product| Json(product.into()))
        .map_err(|err| ApiError::from(err).with_request_id(extract_request_id(&request)))
}

/// Prepares the products router.
pub fn router() -> Router<AppState> {
    Router::new().route("/{id}", get(get_product))
}
