//! Integration tests over the HTTP adapter.

use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::util::ServiceExt;

use cantina_api::app;
use cantina_api::bootstrap;
use cantina_api::state::AppState;
use cantina_application::auth::AuthService;
use cantina_application::catalog::CatalogService;
use cantina_application::orders::OrderService;
use cantina_application::ports::{
    Clock, IdGenerator, PickupCodeGenerator, ProductRepository, RepositoryError,
};
use cantina_core::catalog::{Product, ProductId};
use cantina_infrastructure::config::Config;
use cantina_infrastructure::identifiers::{DefaultPickupCodeGenerator, Generator};
use cantina_infrastructure::persistence::InMemoryRepositories;
use cantina_infrastructure::time::SystemClock;

/// Product repository that always fails, simulating a storage outage.
struct FailingProductRepository;

#[async_trait]
impl ProductRepository for FailingProductRepository {
    async fn find_product_by_id(
        &self,
        _id: &ProductId,
    ) -> Result<Option<Product>, RepositoryError> {
        Err(RepositoryError::new("injected storage failure"))
    }

    async fn list_products(&self) -> Result<Vec<Product>, RepositoryError> {
        Err(RepositoryError::new("injected storage failure"))
    }
}

/// Router backed by the seeded in-memory repositories.
async fn router() -> axum::Router<()> {
    let repositories = InMemoryRepositories::with_seed_data().expect("seed repositories");
    let state = bootstrap::build_state(repositories);
    app(state, &Config::development())
}

/// Router whose catalog is backed by a failing product repository.
async fn failing_router() -> axum::Router<()> {
    let repositories = InMemoryRepositories::with_seed_data().expect("seed repositories");
    let ids: Arc<dyn IdGenerator> = Arc::new(Generator::new());
    let pickup_codes: Arc<dyn PickupCodeGenerator> = Arc::new(DefaultPickupCodeGenerator::new());
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);

    let state = AppState::new(
        Arc::new(CatalogService::new(
            Arc::new(FailingProductRepository),
            repositories.categories.clone(),
        )),
        Arc::new(OrderService::new(
            repositories.orders.clone(),
            Arc::new(FailingProductRepository),
            ids,
            pickup_codes,
            clock,
        )),
        Arc::new(AuthService::new(repositories.users.clone())),
    );
    app(state, &Config::development())
}

async fn raw_get(uri: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = router()
        .await
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, body.to_vec())
}

async fn get(uri: &str) -> (StatusCode, axum::http::HeaderMap, Value) {
    let (status, headers, body) = raw_get(uri).await;
    let json: Value = serde_json::from_slice(&body).unwrap();
    (status, headers, json)
}

async fn get_from(
    router: &axum::Router<()>,
    uri: &str,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = router
        .clone()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    (status, headers, json)
}

#[tokio::test]
async fn root_health_returns_ok_json() {
    let (status, headers, json) = get("/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers["content-type"], "application/json",
        "response must use JSON content type"
    );
    assert_eq!(json["status"], "healthy");
    assert_eq!(json["service"], "cantina-api");
    assert!(json["version"].is_string());
    assert!(!json["version"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn versioned_health_returns_ok_json() {
    let (status, _, json) = get("/api/v1/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["status"], "healthy");
    assert_eq!(json["service"], "cantina-api");
    assert!(json["version"].is_string());
}

#[tokio::test]
async fn unknown_route_returns_not_found() {
    let (status, _, _) = raw_get("/api/v1/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn seeded_product_is_returned() {
    let (status, _, json) = get("/api/v1/products/product_brigadeiro").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["id"], "product_brigadeiro");
    assert_eq!(json["name"], "Brigadeiro");
    assert_eq!(json["price"], 500);
    assert_eq!(json["available"], true);
}

#[tokio::test]
async fn missing_product_returns_centralized_error() {
    let (status, _, json) = get("/api/v1/products/product_missing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "NOT_FOUND");
    assert_eq!(
        json["error"]["message"],
        "the product with id 'product_missing' was not found"
    );
    assert!(
        json["error"]["request_id"].is_string(),
        "request id must be attached"
    );
}

#[tokio::test]
async fn missing_order_returns_centralized_error() {
    let (status, _, json) = get("/api/v1/orders/order_missing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn seeded_category_is_returned() {
    let (status, _, json) = get("/api/v1/categories/category_snacks").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["id"], "category_snacks");
    assert_eq!(json["name"], "Snacks");
}

#[tokio::test]
async fn empty_prepared_routers_answer_not_found() {
    let (status, _, _) = raw_get("/api/v1/auth").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = raw_get("/api/v1/users").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn product_list_returns_only_available_products() {
    let (status, _, json) = get("/api/v1/products").await;
    assert_eq!(status, StatusCode::OK);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 4, "seed has 4 available products");
    for item in items {
        assert_eq!(item["available"], true);
    }
}

#[tokio::test]
async fn product_list_can_include_unavailable_products() {
    let (status, _, json) = get("/api/v1/products?include_unavailable=true").await;
    assert_eq!(status, StatusCode::OK);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 5, "seed has 5 products in total");
    assert!(items.iter().any(|item| item["available"] == false));
}

#[tokio::test]
async fn product_list_filters_by_category() {
    let (status, _, json) = get("/api/v1/products?category_id=category_drinks").await;
    assert_eq!(status, StatusCode::OK);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "product_refresco");
}

#[tokio::test]
async fn product_list_searches_by_name() {
    let (status, _, json) = get("/api/v1/products?search=coxinha").await;
    assert_eq!(status, StatusCode::OK);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["id"], "product_coxinha");
}

#[tokio::test]
async fn product_list_combines_filters() {
    let (status, _, json) =
        get("/api/v1/products?category_id=category_snacks&search=queijo&include_unavailable=true")
            .await;
    assert_eq!(status, StatusCode::OK);
    let items = json["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        1,
        "category, search and availability filters must all apply"
    );
    assert_eq!(items[0]["id"], "product_pastel");
}

#[tokio::test]
async fn product_list_returns_empty_items_for_no_match() {
    let (status, headers, json) = get("/api/v1/products?search=does-not-exist").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers["content-type"], "application/json",
        "even empty results must use JSON content type"
    );
    assert_eq!(
        json["items"],
        serde_json::json!([]),
        "no match must be an empty 200 envelope, not an error"
    );
}

#[tokio::test]
async fn product_list_is_sorted_by_name() {
    let (status, _, json) = get("/api/v1/products?include_unavailable=true").await;
    assert_eq!(status, StatusCode::OK);
    let items = json["items"].as_array().unwrap();
    let names: Vec<&str> = items
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec![
            "Brigadeiro",
            "Coxinha",
            "Pastel de queijo",
            "Pão de queijo",
            "Refresco de limão"
        ]
    );
}

#[tokio::test]
async fn invalid_product_query_returns_stable_invalid_query_error() {
    let (status, _, json) = get("/api/v1/products?include_unavailable=not-a-bool").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "INVALID_QUERY");
    assert_eq!(
        json["error"]["message"], "the catalog query is invalid",
        "the message must be stable and must not leak parser internals"
    );
    assert!(
        json["error"]["request_id"].is_string(),
        "request id must be attached"
    );
}

#[tokio::test]
async fn category_list_returns_envelope() {
    let (status, _, json) = get("/api/v1/categories").await;
    assert_eq!(status, StatusCode::OK);
    let items = json["items"].as_array().unwrap();
    assert_eq!(items.len(), 3, "seed has 3 categories");
    let names: Vec<&str> = items
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["Drinks", "Padaria", "Snacks"]);
}

#[tokio::test]
async fn openapi_document_is_served_as_json() {
    let (status, headers, json) = get("/api/openapi.json").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        headers["content-type"], "application/json",
        "OpenAPI must use JSON content type"
    );
    assert_eq!(json["openapi"], "3.1.0");
    assert_eq!(json["info"]["title"], "CantinaSesi API");
    let paths = json["paths"].as_object().unwrap();
    for path in [
        "/health",
        "/api/v1/health",
        "/api/v1/products",
        "/api/v1/products/{product_id}",
        "/api/v1/categories",
        "/api/v1/categories/{category_id}",
    ] {
        assert!(paths.contains_key(path), "missing OpenAPI path: {path}");
    }
}

#[tokio::test]
async fn openapi_document_describes_expected_schemas() {
    let (_, _, json) = get("/api/openapi.json").await;
    let schemas = json["components"]["schemas"].as_object().unwrap();
    for schema in [
        "HealthResponse",
        "ProductResponse",
        "ProductListResponse",
        "ListProductsParams",
        "CategoryResponse",
        "CategoryListResponse",
        "ErrorResponse",
        "ErrorBody",
    ] {
        assert!(
            schemas.contains_key(schema),
            "missing OpenAPI schema: {schema}"
        );
    }
}

#[tokio::test]
async fn openapi_product_list_documents_query_and_error_response() {
    let (_, _, json) = get("/api/openapi.json").await;
    let operation = &json["paths"]["/api/v1/products"]["get"];
    let parameters = operation["parameters"].as_array().unwrap();
    let names: Vec<&str> = parameters
        .iter()
        .map(|parameter| parameter["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"category_id"));
    assert!(names.contains(&"search"));
    assert!(names.contains(&"include_unavailable"));
    let error_response = &operation["responses"]["400"]["content"]["application/json"]["schema"];
    assert_eq!(
        error_response["$ref"].as_str().unwrap(),
        "#/components/schemas/ErrorResponse"
    );
}

#[tokio::test]
async fn storage_failure_returns_centralized_internal_error() {
    let router = failing_router().await;
    let (status, _, json) = get_from(&router, "/api/v1/products").await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json["error"]["code"], "INTERNAL_ERROR");
    assert_eq!(json["error"]["message"], "internal server error");
    assert!(
        json["error"]["request_id"].is_string(),
        "request id must be attached"
    );
    let body = serde_json::to_string(&json).unwrap();
    assert!(
        !body.contains("injected storage failure"),
        "internal storage details must never leak to clients"
    );
}
