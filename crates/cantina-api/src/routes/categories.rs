//! Category endpoints.

use axum::extract::{Path, Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use cantina_application::catalog::{CreateCategoryCommand, UpdateCategoryCommand};
use cantina_core::catalog::CategoryId;

use super::auth::{AuthenticatedSession, RequestId};
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

/// Body for `POST /api/v1/categories`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCategoryRequest {
    name: String,
}

/// Body for `PATCH /api/v1/categories/{category_id}`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCategoryRequest {
    name: String,
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

/// `POST /api/v1/categories`
#[utoipa::path(
    post,
    path = "/api/v1/categories",
    security(("auth_bearer" = [])),
    request_body = CreateCategoryRequest,
    responses(
        (status = 201, description = "Category created", body = CategoryResponse, headers(
            ("Location" = String, description = "Path of the created category"),
        )),
        (status = 400, description = "Invalid request body", body = ErrorResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "Account inactive or forbidden", body = ErrorResponse),
        (status = 409, description = "Category name conflict", body = ErrorResponse),
        (status = 422, description = "Validation error", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    ),
)]
async fn create_category(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
    payload: Result<Json<CreateCategoryRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(payload) = payload
        .map_err(|err| {
            tracing::debug!(detail = %err, "create category body rejected");
            ApiError::invalid_request_body()
        })
        .map_err(|err| err.with_request_id(request_id.clone()))?;

    let created = state
        .catalog
        .create_category(
            &context.principal,
            CreateCategoryCommand { name: payload.name },
        )
        .await
        .map_err(|err| ApiError::from(err).with_request_id(request_id))?;

    let id = created.id.to_string();
    let location = format!("/api/v1/categories/{id}");
    let body = Json(CategoryResponse::from(created));
    Ok((StatusCode::CREATED, [(header::LOCATION, location)], body).into_response())
}

/// `PATCH /api/v1/categories/{category_id}`
#[utoipa::path(
    patch,
    path = "/api/v1/categories/{category_id}",
    security(("auth_bearer" = [])),
    params(
        ("category_id" = String, Path, description = "Category identifier"),
    ),
    request_body = UpdateCategoryRequest,
    responses(
        (status = 200, description = "Category updated", body = CategoryResponse),
        (status = 400, description = "Invalid request body", body = ErrorResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "Account inactive or forbidden", body = ErrorResponse),
        (status = 404, description = "Category not found", body = ErrorResponse),
        (status = 409, description = "Category name conflict", body = ErrorResponse),
        (status = 422, description = "Validation error", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse),
    ),
)]
async fn update_category(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
    Path(id): Path<String>,
    payload: Result<Json<UpdateCategoryRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<CategoryResponse>, ApiError> {
    let Json(payload) = payload
        .map_err(|err| {
            tracing::debug!(detail = %err, "update category body rejected");
            ApiError::invalid_request_body()
        })
        .map_err(|err| err.with_request_id(request_id.clone()))?;

    let updated = state
        .catalog
        .update_category(
            &context.principal,
            UpdateCategoryCommand {
                category_id: CategoryId::new(id),
                name: payload.name,
            },
        )
        .await
        .map_err(|err| ApiError::from(err).with_request_id(request_id))?;

    Ok(Json(updated.into()))
}

/// Prepares the categories router.
///
/// Public reads require no authentication; write routes require a valid bearer
/// token and enforce role policy in the application layer. Bodies are bounded
/// to 32 KiB; oversized payloads receive the stable `INVALID_REQUEST_BODY`.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_categories).post(create_category))
        .route("/{category_id}", get(get_category).patch(update_category))
        .layer(axum::extract::DefaultBodyLimit::max(32 * 1024))
}
