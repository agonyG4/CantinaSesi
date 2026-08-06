//! OpenAPI description of the versioned API.
//!
//! Exposed as JSON at `GET /api/openapi.json`. No interactive UI is served;
//! the document is available for tooling and generated clients.

use axum::Json;
use utoipa::OpenApi;

use super::auth::{AuthenticationResponse, LoginRequest, UserResponse};
use super::categories::{CategoryListResponse, CategoryResponse};
use super::health::HealthResponse;
use super::products::{ListProductsParams, ProductListResponse, ProductResponse};
use crate::error::{ErrorBody, ErrorResponse};

/// Combined OpenAPI description for the versioned API.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "CantinaSesi API",
        description = "HTTP API for the CantinaSesi school cafeteria backend.",
        version = "0.1.0"
    ),
    paths(
        super::health::health,
        super::health::versioned_health,
        super::auth::login,
        super::auth::refresh,
        super::auth::logout,
        super::auth::me,
        super::products::list_products,
        super::products::get_product,
        super::categories::list_categories,
        super::categories::get_category,
    ),
    components(schemas(
        HealthResponse,
        LoginRequest,
        AuthenticationResponse,
        UserResponse,
        ProductResponse,
        ProductListResponse,
        ListProductsParams,
        CategoryResponse,
        CategoryListResponse,
        ErrorResponse,
        ErrorBody,
    )),
    tags(
        (name = "auth", description = "Authentication and session management")
    ),
    modifiers(&SecuritySchemeAddon)
)]
pub struct ApiDoc;

/// Addon injecting the bearer-token security scheme.
struct SecuritySchemeAddon;

impl utoipa::Modify for SecuritySchemeAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.as_mut().expect("components exist");
        let scheme = utoipa::openapi::security::HttpBuilder::new()
            .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
            .description(Some(
                "An opaque session access token issued by the login endpoint.",
            ))
            .build();
        components.add_security_scheme(
            "auth_bearer",
            utoipa::openapi::security::SecurityScheme::Http(scheme),
        );
    }
}

/// `GET /api/openapi.json`
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}
