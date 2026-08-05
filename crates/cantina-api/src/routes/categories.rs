//! Category endpoints.

use axum::extract::{Path, Request, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use cantina_core::catalog::CategoryId;

use crate::error::{ApiError, extract_request_id};
use crate::state::AppState;

/// HTTP representation of a category.
#[derive(Debug, Serialize)]
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

/// `GET /api/v1/categories/{id}`
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
    Router::new().route("/{id}", get(get_category))
}
