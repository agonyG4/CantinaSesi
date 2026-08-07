//! Product endpoints.

use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use cantina_application::catalog::{
    CreateProductCommand, ListProductsQuery, SetProductAvailabilityCommand, UpdateProductCommand,
};
use cantina_core::catalog::{CategoryId, ProductId};

use super::auth::{AuthenticatedSession, RequestId};
use crate::error::{ApiError, ErrorResponse, extract_request_id};
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

/// Body for `POST /api/v1/products`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateProductRequest {
    name: String,
    description: String,
    /// Price in minor units.
    price: u64,
    category_id: String,
    /// Availability; defaults to `true` when omitted.
    #[serde(default = "default_available")]
    available: bool,
}

/// Body for `PATCH /api/v1/products/{product_id}`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProductRequest {
    name: Option<String>,
    description: Option<String>,
    /// Price in minor units.
    price: Option<u64>,
    category_id: Option<String>,
}

/// Body for `PATCH /api/v1/products/{product_id}/availability`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct SetProductAvailabilityRequest {
    available: bool,
}

fn default_available() -> bool {
    true
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
        (status = 400, description = "Invalid query parameters", body = ErrorResponse),
    ),
)]
async fn list_products(
    State(state): State<AppState>,
    params: Result<Query<ListProductsParams>, QueryRejection>,
    request: Request,
) -> Result<Json<ProductListResponse>, ApiError> {
    let params = params
        .map_err(|err| {
            tracing::debug!(detail = %err, "invalid catalog query rejected");
            ApiError::invalid_query("the catalog query is invalid")
        })
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

/// `GET /api/v1/products/{product_id}`
#[utoipa::path(
    get,
    path = "/api/v1/products/{product_id}",
    params(
        ("product_id" = String, Path, description = "Product identifier"),
    ),
    responses(
        (status = 200, description = "The requested product", body = ProductResponse),
        (status = 404, description = "Product not found", body = ErrorResponse),
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

/// `POST /api/v1/products`
#[utoipa::path(
    post,
    path = "/api/v1/products",
    security(("auth_bearer" = [])),
    request_body = CreateProductRequest,
    responses(
        (status = 201, description = "Product created", body = ProductResponse, headers(
            ("Location" = String, description = "Path of the created product"),
        )),
        (status = 400, description = "Invalid request body", body = ErrorResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "Account inactive or forbidden", body = ErrorResponse),
        (status = 404, description = "Referenced category not found", body = ErrorResponse),
        (status = 422, description = "Validation error", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    ),
)]
async fn create_product(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
    payload: Result<Json<CreateProductRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(payload) = payload
        .map_err(|err| {
            tracing::debug!(detail = %err, "create product body rejected");
            ApiError::invalid_request_body()
        })
        .map_err(|err| err.with_request_id(request_id.clone()))?;

    let created = state
        .catalog
        .create_product(
            &context.principal,
            CreateProductCommand {
                name: payload.name,
                description: payload.description,
                price: payload.price,
                category_id: CategoryId::new(payload.category_id),
                available: payload.available,
            },
        )
        .await
        .map_err(|err| ApiError::from(err).with_request_id(request_id))?;

    let id = created.id.to_string();
    let location = format!("/api/v1/products/{id}");
    let body = Json(ProductResponse::from(created));
    Ok((StatusCode::CREATED, [(header::LOCATION, location)], body).into_response())
}

/// `PATCH /api/v1/products/{product_id}`
#[utoipa::path(
    patch,
    path = "/api/v1/products/{product_id}",
    security(("auth_bearer" = [])),
    params(
        ("product_id" = String, Path, description = "Product identifier"),
    ),
    request_body = UpdateProductRequest,
    responses(
        (status = 200, description = "Product updated", body = ProductResponse),
        (status = 400, description = "Invalid request body", body = ErrorResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "Account inactive or forbidden", body = ErrorResponse),
        (status = 404, description = "Product or referenced category not found", body = ErrorResponse),
        (status = 422, description = "Empty update or validation error", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    ),
)]
async fn update_product(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
    Path(id): Path<String>,
    payload: Result<Json<UpdateProductRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<ProductResponse>, ApiError> {
    let Json(payload) = payload
        .map_err(|err| {
            tracing::debug!(detail = %err, "update product body rejected");
            ApiError::invalid_request_body()
        })
        .map_err(|err| err.with_request_id(request_id.clone()))?;

    let updated = state
        .catalog
        .update_product(
            &context.principal,
            UpdateProductCommand {
                product_id: ProductId::new(id),
                name: payload.name,
                description: payload.description,
                price: payload.price,
                category_id: payload.category_id.map(CategoryId::new),
            },
        )
        .await
        .map_err(|err| ApiError::from(err).with_request_id(request_id))?;

    Ok(Json(updated.into()))
}

/// `PATCH /api/v1/products/{product_id}/availability`
#[utoipa::path(
    patch,
    path = "/api/v1/products/{product_id}/availability",
    security(("auth_bearer" = [])),
    params(
        ("product_id" = String, Path, description = "Product identifier"),
    ),
    request_body = SetProductAvailabilityRequest,
    responses(
        (status = 200, description = "Product availability updated", body = ProductResponse),
        (status = 400, description = "Invalid request body", body = ErrorResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "Account inactive or forbidden", body = ErrorResponse),
        (status = 404, description = "Product not found", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    ),
)]
async fn set_product_availability(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
    Path(id): Path<String>,
    payload: Result<Json<SetProductAvailabilityRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<ProductResponse>, ApiError> {
    let Json(payload) = payload
        .map_err(|err| {
            tracing::debug!(detail = %err, "availability body rejected");
            ApiError::invalid_request_body()
        })
        .map_err(|err| err.with_request_id(request_id.clone()))?;

    let updated = state
        .catalog
        .set_product_availability(
            &context.principal,
            SetProductAvailabilityCommand {
                product_id: ProductId::new(id),
                available: payload.available,
            },
        )
        .await
        .map_err(|err| ApiError::from(err).with_request_id(request_id))?;

    Ok(Json(updated.into()))
}

/// Prepares the products router.
///
/// Public reads require no authentication; write routes require a valid bearer
/// token and enforce role policy in the application layer. Bodies are bounded
/// to 32 KiB; oversized payloads receive the stable `INVALID_REQUEST_BODY`.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_products).post(create_product))
        .route("/{product_id}", get(get_product).patch(update_product))
        .route(
            "/{product_id}/availability",
            patch(set_product_availability),
        )
        .layer(axum::extract::DefaultBodyLimit::max(32 * 1024))
}
