//! OpenAPI description of the versioned API.
//!
//! Exposed as JSON at `GET /api/openapi.json`. No interactive UI is served;
//! the document is available for tooling and generated clients.

use axum::Json;
use utoipa::OpenApi;

use super::categories::{CategoryListResponse, CategoryResponse};
use super::products::{ListProductsParams, ProductListResponse, ProductResponse};

/// Combined OpenAPI description for the versioned API.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "CantinaSesi API",
        description = "HTTP API for the CantinaSesi school cafeteria backend.",
        version = "0.1.0"
    ),
    paths(
        super::products::list_products,
        super::products::get_product,
        super::categories::list_categories,
        super::categories::get_category,
    ),
    components(schemas(
        ProductResponse,
        ProductListResponse,
        ListProductsParams,
        CategoryResponse,
        CategoryListResponse,
    ))
)]
pub struct ApiDoc;

/// `GET /api/openapi.json`
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}
