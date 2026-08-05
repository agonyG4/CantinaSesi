//! Category endpoints.

use axum::extract::{Path, Request, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use utoipa::ToSchema;

use cantina_core::catalog::CategoryId;

use crate::error::{ApiError, ErrorResponse, extract_request_id};
use crate::state::AppState;

/// HTTP representation of a category.
#[derive(Debug, Serialize, ToSchema)]
pub struct CategoryResponse {
    id: String,
    name: String,
}

impl From<cantina_application::catalog::CategoryDto> for CategoryResponse {
    fn from(dto: cantina_application::catalog::CategoryDto) -> Self {
        Self {
            id: dto.id.to_string(),
            name: dto.name,
        }
    }
}

/// Envelope for category collection responses.
#[derive(Debug, Serialize, ToSchema)]
pub struct CategoryListResponse {
    items: Vec<CategoryResponse>,
}

/// `GET /api/v1/categories`
#[utoipa::path(
    get,
    path = "/api/v1/categories",
    responses(
        (status = 200, description = "The category list", body = CategoryListResponse),
        (status = 500, description = "Internal storage failure", body = ErrorResponse),
    ),
)]
async fn list_categories(
    State(state): State<AppState>,
    request: Request,
) -> Result<Json<CategoryListResponse>, ApiError> {
    state
        .catalog
        .list_categories()
        .await
        .map(|categories| {
            Json(CategoryListResponse {
                items: categories.into_iter().map(Into::into).collect(),
            })
        })
        .map_err(|err| ApiError::from(err).with_request_id(extract_request_id(&request)))
}

/// `GET /api/v1/categories/{category_id}`
#[utoipa::path(
    get,
    path = "/api/v1/categories/{category_id}",
    params(
        ("category_id" = String, Path, description = "Category identifier"),
    ),
    responses(
        (status = 200, description = "The requested category", body = CategoryResponse),
        (status = 404, description = "Category not found", body = ErrorResponse),
    ),
)]
async fn get_category(
    State(state): State<AppState>,
    Path(id): Path<String>,
    request: Request,
) -> Result<Json<CategoryResponse>, ApiError> {
    state
        .catalog
        .get_category(CategoryId::new(id))
        .await
        .map(|category| Json(category.into()))
        .map_err(|err| ApiError::from(err).with_request_id(extract_request_id(&request)))
}

/// Prepares the categories router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_categories))
        .route("/{category_id}", get(get_category))
}
