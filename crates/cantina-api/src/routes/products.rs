//! Product endpoints.

use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, Request, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use cantina_application::catalog::ListProductsQuery;
use cantina_core::catalog::{CategoryId, ProductId};

use crate::error::{ApiError, extract_request_id};
use crate::state::AppState;

/// HTTP representation of a product.
#[derive(Debug, Serialize, ToSchema)]
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

/// Envelope for product collection responses.
#[derive(Debug, Serialize, ToSchema)]
pub struct ProductListResponse {
    items: Vec<ProductResponse>,
}

/// Query parameters accepted by `GET /api/v1/products`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ListProductsParams {
    /// Restricts the results to a single category.
    #[serde(default)]
    category_id: Option<String>,
    /// Case-insensitive substring match against the name or description.
    #[serde(default)]
    search: Option<String>,
    /// When `true` unavailable products are included.
    #[serde(default)]
    include_unavailable: bool,
}

/// `GET /api/v1/products`
#[utoipa::path(
    get,
    path = "/api/v1/products",
    params(
        ("category_id" = Option<String>, Query, description = "Restrict results to a single category"),
        ("search" = Option<String>, Query, description = "Case-insensitive substring match against the name or description"),
        ("include_unavailable" = Option<bool>, Query, description = "Include unavailable products when true"),
    ),
    responses(
        (status = 200, description = "The product list", body = ProductListResponse),
        (status = 400, description = "Invalid query parameters"),
    ),
)]
async fn list_products(
    State(state): State<AppState>,
    params: Result<Query<ListProductsParams>, QueryRejection>,
    request: Request,
) -> Result<Json<ProductListResponse>, ApiError> {
    let params = params
        .map_err(|err| ApiError::invalid_query(err.body_text()))
        .map_err(|err| err.with_request_id(extract_request_id(&request)))?;
    let Query(params) = params;

    let category_id = match params.category_id {
        Some(value) if !value.trim().is_empty() => Some(CategoryId::new(value)),
        _ => None,
    };

    let products = state
        .catalog
        .list_products(&ListProductsQuery {
            category_id,
            search: params.search,
            include_unavailable: params.include_unavailable,
        })
        .await
        .map_err(|err| ApiError::from(err).with_request_id(extract_request_id(&request)))?;

    Ok(Json(ProductListResponse {
        items: products.into_iter().map(Into::into).collect(),
    }))
}

/// `GET /api/v1/products/{id}`
#[utoipa::path(
    get,
    path = "/api/v1/products/{id}",
    params(
        ("id" = String, Path, description = "Product identifier"),
    ),
    responses(
        (status = 200, description = "The requested product", body = ProductResponse),
        (status = 404, description = "Product not found"),
    ),
)]
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
    Router::new()
        .route("/", get(list_products))
        .route("/{id}", get(get_product))
}
