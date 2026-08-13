//! Integration tests over the HTTP adapter.

use std::sync::Arc;

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::util::ServiceExt;

use cantina_api::app;
use cantina_api::app_with_test_routes;
use cantina_api::bootstrap;
use cantina_api::state::AppState;
use cantina_application::auth::AuthService;
use cantina_application::catalog::CatalogService;
use cantina_application::orders::OrderService;
use cantina_application::ports::{
    CatalogWriteError, Clock, IdGenerator, PickupCodeGenerator, ProductRepository, RepositoryError,
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

    async fn find_products_by_ids(
        &self,
        _ids: &[ProductId],
    ) -> Result<Vec<Product>, RepositoryError> {
        Err(RepositoryError::new("injected storage failure"))
    }

    async fn list_products(&self) -> Result<Vec<Product>, RepositoryError> {
        Err(RepositoryError::new("injected storage failure"))
    }

    async fn create_product(&self, _product: Product) -> Result<(), CatalogWriteError> {
        Err(CatalogWriteError::Internal(
            "injected storage failure".to_string(),
        ))
    }

    async fn update_product(&self, _product: Product) -> Result<(), CatalogWriteError> {
        Err(CatalogWriteError::Internal(
            "injected storage failure".to_string(),
        ))
    }
}

/// Router backed by the seeded in-memory repositories, including the
/// test-only RBAC guard routes.
async fn router() -> axum::Router<()> {
    let repositories = InMemoryRepositories::with_seed_data()
        .await
        .expect("seed repositories");
    let state = bootstrap::build_state(repositories, &Config::development());
    app_with_test_routes(state, &Config::development())
}

/// Router whose catalog is backed by a failing product repository.
async fn failing_router() -> axum::Router<()> {
    let repositories = InMemoryRepositories::with_seed_data()
        .await
        .expect("seed repositories");
    let ids: Arc<dyn IdGenerator> = Arc::new(Generator::new());
    let pickup_codes: Arc<dyn PickupCodeGenerator> = Arc::new(DefaultPickupCodeGenerator::new());
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);
    let config = Config::development();

    let auth = AuthService::new(
        repositories.users.clone(),
        repositories.credentials.clone(),
        repositories.sessions.clone(),
        Arc::new(cantina_infrastructure::security::passwords::Argon2PasswordService::new()),
        Arc::new(cantina_infrastructure::security::tokens::OsRngSessionTokenService),
        clock.clone(),
        config.session_config(),
    );

    let state = AppState::new(
        Arc::new(CatalogService::new(
            Arc::new(FailingProductRepository),
            repositories.categories.clone(),
            ids.clone(),
        )),
        Arc::new(OrderService::new(
            repositories.orders.clone(),
            Arc::new(FailingProductRepository),
            ids,
            pickup_codes,
            clock,
        )),
        Arc::new(auth),
    );
    app(state, &config)
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

async fn post(
    router: &axum::Router<()>,
    uri: &str,
    body: Value,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    if body.is_empty() {
        return (status, headers, Value::Null);
    }
    let json: Value = serde_json::from_slice(&body).unwrap();
    (status, headers, json)
}

/// Sends an authenticated JSON request and returns status, headers and body.
async fn authed_request(
    router: &axum::Router<()>,
    method: &str,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if !token.is_empty() {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let body = match body {
        Some(payload) => {
            builder = builder.header("content-type", "application/json");
            Body::from(payload.to_string())
        }
        None => Body::empty(),
    };
    let response = router
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    if bytes.is_empty() {
        return (status, headers, Value::Null);
    }
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    (status, headers, json)
}

/// Logs in as the seeded user with the given credentials.
async fn token_for(router: &axum::Router<()>, email: &str, password: &str) -> String {
    let (status, _, json) = post(
        router,
        "/api/v1/auth/login",
        serde_json::json!({
            "email": email,
            "password": password,
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "login for {email} must succeed");
    json["access_token"].as_str().unwrap().to_string()
}

/// Logs in as the seeded canteen operator and returns the access token.
async fn canteen_token(router: &axum::Router<()>) -> String {
    token_for(router, "cantina@example.com", "CanteenDemo!2026").await
}

/// Logs in as the seeded admin and returns the access token.
async fn admin_token(router: &axum::Router<()>) -> String {
    token_for(router, "admin@example.com", "AdminDemo!2026").await
}

/// `GET` via the given router (used by the authenticated helpers).
async fn authenticated_get(
    router: &axum::Router<()>,
    uri: &str,
    token: &str,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    (status, headers, json)
}

/// Logs in as the seeded student and returns the access token.
async fn student_token(router: &axum::Router<()>) -> String {
    let (status, _, json) = post(
        router,
        "/api/v1/auth/login",
        serde_json::json!({
            "email": "ana@example.com",
            "password": "StudentDemo!2026"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "student login must succeed");
    json["access_token"].as_str().unwrap().to_string()
}

async fn student_two_token(router: &axum::Router<()>) -> String {
    token_for(router, "bruno@example.com", "StudentTwoDemo!2026").await
}

async fn raw_authenticated_get(
    router: &axum::Router<()>,
    uri: &str,
    token: &str,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, headers, body.to_vec())
}

/// POSTs `/api/v1/auth/refresh` with the given bearer token.
async fn post_refresh(
    router: &axum::Router<()>,
    token: &str,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap()
    };
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
async fn student_can_create_list_and_read_own_order() {
    let app = router().await;
    let token = student_token(&app).await;
    let (status, headers, created) = authed_request(
        &app,
        "POST",
        "/api/v1/orders",
        &token,
        Some(serde_json::json!({
            "items": [{"product_id": "product_coxinha", "quantity": 2}]
        })),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(
        headers["location"]
            .to_str()
            .unwrap()
            .contains("/api/v1/orders/")
    );
    assert_eq!(created["status"], "pending");
    assert_eq!(created["items"][0]["quantity"], 2);
    assert!(created["pickup_code"].is_string());

    let order_id = created["id"].as_str().unwrap();
    let (status, _, history) = authed_request(&app, "GET", "/api/v1/orders", &token, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(history["items"].as_array().unwrap().len(), 1);

    let (status, _, detail) = authed_request(
        &app,
        "GET",
        &format!("/api/v1/orders/{order_id}"),
        &token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["id"], order_id);
    assert_eq!(detail["pickup_code"], created["pickup_code"]);
}

#[tokio::test]
async fn student_cannot_read_another_students_order_or_pickup_code() {
    let app = router().await;
    let owner = student_token(&app).await;
    let other_student = student_two_token(&app).await;
    let (_, _, created) = authed_request(
        &app,
        "POST",
        "/api/v1/orders",
        &owner,
        Some(serde_json::json!({
            "items": [{"product_id": "product_coxinha", "quantity": 1}]
        })),
    )
    .await;
    let order_id = created["id"].as_str().unwrap();
    let pickup_code = created["pickup_code"].as_str().unwrap().to_string();

    let (status, _, detail) = authed_request(
        &app,
        "GET",
        &format!("/api/v1/orders/{order_id}"),
        &other_student,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_ne!(detail["error"]["message"], pickup_code);
    assert!(detail.get("pickup_code").is_none());

    let (status, _, history) =
        authed_request(&app, "GET", "/api/v1/orders", &other_student, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(history["items"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn students_cannot_access_canteen_queue_or_change_status() {
    let app = router().await;
    let token = student_token(&app).await;
    let (status, _, queue) =
        authed_request(&app, "GET", "/api/v1/canteen/orders", &token, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(queue["error"]["code"], "FORBIDDEN");

    let (status, _, detail) = authed_request(
        &app,
        "GET",
        "/api/v1/canteen/orders/order_missing",
        &token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(detail["error"]["code"], "FORBIDDEN");

    let (status, _, response) = authed_request(
        &app,
        "PATCH",
        "/api/v1/canteen/orders/order_missing/status",
        &token,
        Some(serde_json::json!({"status": "confirmed"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(response["error"]["code"], "FORBIDDEN");
}

#[tokio::test]
async fn canteen_can_list_orders_and_apply_domain_status_transitions() {
    let app = router().await;
    let student = student_token(&app).await;
    let canteen = canteen_token(&app).await;
    let (_, _, created) = authed_request(
        &app,
        "POST",
        "/api/v1/orders",
        &student,
        Some(serde_json::json!({
            "items": [{"product_id": "product_coxinha", "quantity": 1}]
        })),
    )
    .await;
    let order_id = created["id"].as_str().unwrap();

    let (status, _, queue) =
        authed_request(&app, "GET", "/api/v1/canteen/orders", &canteen, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(queue["items"].as_array().unwrap().len(), 1);
    assert_eq!(queue["items"][0]["pickup_code"], created["pickup_code"]);

    let (status, _, canteen_detail) = authed_request(
        &app,
        "GET",
        &format!("/api/v1/canteen/orders/{order_id}"),
        &canteen,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(canteen_detail["id"], order_id);

    let (status, _, updated) = authed_request(
        &app,
        "PATCH",
        &format!("/api/v1/canteen/orders/{order_id}/status"),
        &canteen,
        Some(serde_json::json!({"status": "confirmed"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["status"], "confirmed");

    let (status, _, invalid) = authed_request(
        &app,
        "PATCH",
        &format!("/api/v1/canteen/orders/{order_id}/status"),
        &canteen,
        Some(serde_json::json!({"status": "ready"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(invalid["error"]["code"], "INVALID_STATUS_TRANSITION");
}

#[tokio::test]
async fn concurrent_identical_status_updates_have_one_success_and_one_conflict() {
    let app = router().await;
    let student = student_token(&app).await;
    let canteen = canteen_token(&app).await;
    let (_, _, created) = authed_request(
        &app,
        "POST",
        "/api/v1/orders",
        &student,
        Some(serde_json::json!({
            "items": [{"product_id": "product_coxinha", "quantity": 1}]
        })),
    )
    .await;
    let order_id = created["id"].as_str().unwrap();
    let status_uri = format!("/api/v1/canteen/orders/{order_id}/status");

    let first = authed_request(
        &app,
        "PATCH",
        &status_uri,
        &canteen,
        Some(serde_json::json!({"status": "confirmed"})),
    );
    let second = authed_request(
        &app,
        "PATCH",
        &status_uri,
        &canteen,
        Some(serde_json::json!({"status": "confirmed"})),
    );
    let (first, second) = tokio::join!(first, second);
    let responses = [first, second];

    assert_eq!(
        responses
            .iter()
            .filter(|response| response.0 == StatusCode::OK)
            .count(),
        1
    );
    assert_eq!(
        responses
            .iter()
            .filter(|response| response.0 == StatusCode::CONFLICT)
            .count(),
        1
    );
    assert!(
        responses
            .iter()
            .any(|response| { response.2["error"]["code"] == "INVALID_STATUS_TRANSITION" })
    );
}

#[tokio::test]
async fn order_creation_validates_body_quantities_products_and_duplicates() {
    let app = router().await;
    let token = student_token(&app).await;

    let (status, _, empty) = authed_request(
        &app,
        "POST",
        "/api/v1/orders",
        &token,
        Some(serde_json::json!({"items": []})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(empty["error"]["code"], "EMPTY_ORDER");

    let (status, _, invalid_quantity) = authed_request(
        &app,
        "POST",
        "/api/v1/orders",
        &token,
        Some(serde_json::json!({
            "items": [{"product_id": "product_coxinha", "quantity": 0}]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(invalid_quantity["error"]["code"], "INVALID_QUANTITY");

    let (status, _, missing) = authed_request(
        &app,
        "POST",
        "/api/v1/orders",
        &token,
        Some(serde_json::json!({
            "items": [{"product_id": "product_missing", "quantity": 1}]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(missing["error"]["code"], "PRODUCT_UNAVAILABLE");

    let (status, _, duplicate) = authed_request(
        &app,
        "POST",
        "/api/v1/orders",
        &token,
        Some(serde_json::json!({
            "items": [
                {"product_id": "product_coxinha", "quantity": 2},
                {"product_id": "product_coxinha", "quantity": 3}
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(duplicate["items"].as_array().unwrap().len(), 1);
    assert_eq!(duplicate["items"][0]["quantity"], 5);
}

#[tokio::test]
async fn orders_require_authentication() {
    let app = router().await;
    let (status, _, body) = raw_get("/api/v1/orders").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "AUTHENTICATION_REQUIRED");

    let (parts, body) = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/orders")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::json!({"items": []}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
        .into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    assert_eq!(parts.status, StatusCode::UNAUTHORIZED);
    let json: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["error"]["code"], "AUTHENTICATION_REQUIRED");
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
    let app = router().await;
    let token = student_token(&app).await;
    let (status, _, json) =
        authed_request(&app, "GET", "/api/v1/orders/order_missing", &token, None).await;
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
        "CreateOrderRequest",
        "OrderItemRequest",
        "OrderItemResponse",
        "OrderResponse",
        "OrderListResponse",
        "ChangeOrderStatusRequest",
    ] {
        assert!(
            schemas.contains_key(schema),
            "missing OpenAPI schema: {schema}"
        );
    }
}

#[tokio::test]
async fn openapi_documents_the_complete_orders_surface() {
    let (_, _, json) = get("/api/openapi.json").await;
    let paths = json["paths"].as_object().unwrap();
    for path in [
        "/api/v1/orders",
        "/api/v1/orders/{order_id}",
        "/api/v1/canteen/orders",
        "/api/v1/canteen/orders/{order_id}/status",
    ] {
        assert!(
            paths.contains_key(path),
            "missing OpenAPI order path: {path}"
        );
    }
    assert!(paths["/api/v1/orders"]["post"]["security"].is_array());
    assert!(paths["/api/v1/orders"]["post"]["requestBody"].is_object());
    assert!(paths["/api/v1/orders"]["post"]["responses"]["201"].is_object());
    assert_eq!(
        paths["/api/v1/orders"]["post"]["responses"]["409"]["description"],
        "Product unavailable"
    );
    assert!(paths["/api/v1/orders"]["post"]["responses"]["500"].is_object());
    assert!(paths["/api/v1/canteen/orders/{order_id}/status"]["patch"]["requestBody"].is_object());
    assert!(
        paths["/api/v1/canteen/orders/{order_id}/status"]["patch"]["responses"]["409"].is_object()
    );
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

#[tokio::test]
async fn login_with_valid_credentials_returns_session() {
    let router = router().await;
    let (status, headers, json) = post(
        &router,
        "/api/v1/auth/login",
        serde_json::json!({
            "email": "ana@example.com",
            "password": "StudentDemo!2026"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["cache-control"], "no-store");
    assert_eq!(headers["pragma"], "no-cache");
    assert!(json["access_token"].as_str().unwrap().len() > 32);
    assert_eq!(json["token_type"], "Bearer");
    assert!(json["expires_at_epoch_seconds"].as_u64().is_some());
    assert_eq!(json["user"]["email"], "ana@example.com");
    assert_eq!(json["user"]["role"], "student");
    assert_eq!(json["user"]["status"], "active");
}

#[tokio::test]
async fn login_with_wrong_password_is_a_stable_401() {
    let router = router().await;
    let (status, _, json) = post(
        &router,
        "/api/v1/auth/login",
        serde_json::json!({
            "email": "ana@example.com",
            "password": "wrong-password"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"]["code"], "INVALID_CREDENTIALS");
    assert_eq!(json["error"]["message"], "invalid email or password");
}

#[tokio::test]
async fn login_with_unknown_email_is_a_stable_401() {
    let router = router().await;
    let (status, _, json) = post(
        &router,
        "/api/v1/auth/login",
        serde_json::json!({
            "email": "ghost@example.com",
            "password": "whatever"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"]["code"], "INVALID_CREDENTIALS");
}

#[tokio::test]
async fn login_with_inactive_account_is_a_403() {
    let router = router().await;
    let (status, _, json) = post(
        &router,
        "/api/v1/auth/login",
        serde_json::json!({
            "email": "inactive@example.com",
            "password": "InactiveDemo!2026"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"]["code"], "ACCOUNT_INACTIVE");
}

#[tokio::test]
async fn login_with_invalid_body_is_a_400() {
    let router = router().await;
    let (status, _, json) = post(
        &router,
        "/api/v1/auth/login",
        serde_json::json!({ "email": "ana@example.com" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "INVALID_REQUEST_BODY");
}

#[tokio::test]
async fn login_normalizes_email_case() {
    let router = router().await;
    let (status, _, _) = post(
        &router,
        "/api/v1/auth/login",
        serde_json::json!({
            "email": "  ANA@Example.COM  ",
            "password": "StudentDemo!2026"
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "email must be normalized to ASCII lowercase"
    );
}

#[tokio::test]
async fn me_returns_the_authenticated_user() {
    let router = router().await;
    let token = student_token(&router).await;
    let (status, _, json) = authenticated_get(&router, "/api/v1/auth/me", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["email"], "ana@example.com");
    assert_eq!(json["role"], "student");
}

#[tokio::test]
async fn me_without_token_is_a_bearer_challenge() {
    let router = router().await;
    let (status, headers, json) = authenticated_get(&router, "/api/v1/auth/me", "").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"]["code"], "AUTHENTICATION_REQUIRED");
    assert_eq!(headers["www-authenticate"], "Bearer");
}

#[tokio::test]
async fn me_with_invalid_token_is_a_401() {
    let router = router().await;
    let (status, _, json) = authenticated_get(&router, "/api/v1/auth/me", "not-a-real-token").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"]["code"], "INVALID_ACCESS_TOKEN");
}

#[tokio::test]
async fn refresh_issues_a_new_valid_session() {
    let router = router().await;
    let token = student_token(&router).await;

    let (status, _, _) = post(&router, "/api/v1/auth/refresh", serde_json::json!({})).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "refresh requires a token");

    let refreshed = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/refresh")
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(refreshed.status(), StatusCode::OK);
    let headers = refreshed.headers().clone();
    let body = refreshed.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json["access_token"].as_str().is_some());
    assert_eq!(json["token_type"], "Bearer");
    assert_eq!(headers["cache-control"], "no-store");
}

#[tokio::test]
async fn logout_revokes_the_session() {
    let router = router().await;
    let token = student_token(&router).await;

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/logout")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let (status, _, _) = authenticated_get(&router, "/api/v1/auth/me", &token).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "revoked token must be rejected"
    );
}

#[tokio::test]
async fn admin_can_access_admin_only_route() {
    let router = router().await;
    let (status, _, json) = post(
        &router,
        "/api/v1/auth/login",
        serde_json::json!({
            "email": "admin@example.com",
            "password": "AdminDemo!2026"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let token = json["access_token"].as_str().unwrap().to_string();
    let (status, _, body) = raw_authenticated_get(&router, "/roles/admin-only", &token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(String::from_utf8(body).unwrap(), "allowed");
}

#[tokio::test]
async fn student_is_forbidden_from_admin_only_route() {
    let router = router().await;
    let token = student_token(&router).await;
    let (status, headers, json) = authenticated_get(&router, "/roles/admin-only", &token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"]["code"], "FORBIDDEN");
    assert_eq!(headers["www-authenticate"], "Bearer");
}

#[tokio::test]
async fn canteen_can_access_canteen_or_admin_route_but_student_cannot() {
    let router = router().await;

    let (status, _, json) = post(
        &router,
        "/api/v1/auth/login",
        serde_json::json!({
            "email": "cantina@example.com",
            "password": "CanteenDemo!2026"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let canteen_token = json["access_token"].as_str().unwrap().to_string();
    let (status, _, body) =
        raw_authenticated_get(&router, "/roles/canteen-admin", &canteen_token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(String::from_utf8(body).unwrap(), "allowed");

    let student = student_token(&router).await;
    let (status, _, json) = authenticated_get(&router, "/roles/canteen-admin", &student).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"]["code"], "FORBIDDEN");
}

#[tokio::test]
async fn concurrent_refresh_of_the_same_token_yields_exactly_one_success() {
    let router = router().await;
    let token = student_token(&router).await;

    let barrier = Arc::new(tokio::sync::Barrier::new(8));
    let mut handles = Vec::new();
    for _ in 0..8 {
        let router = router.clone();
        let barrier = barrier.clone();
        let token = token.clone();
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            post_refresh(&router, &token).await
        }));
    }

    let mut successes = 0;
    let mut rejected = 0;
    let mut winner: Option<String> = None;
    for handle in handles {
        let (status, headers, json) = handle.await.expect("task must not panic");
        if status == StatusCode::OK {
            successes += 1;
            winner = json["access_token"].as_str().map(ToOwned::to_owned);
            assert_eq!(headers["cache-control"], "no-store");
        } else {
            assert_eq!(
                status,
                StatusCode::UNAUTHORIZED,
                "losing refreshes must be 401"
            );
            assert_eq!(json["error"]["code"], "INVALID_ACCESS_TOKEN");
            assert!(
                json["error"]["request_id"].is_string(),
                "losing refreshes must carry a request id"
            );
            assert_eq!(
                headers["www-authenticate"], "Bearer",
                "losing refreshes must challenge with Bearer"
            );
            assert_eq!(
                headers["content-type"], "application/json",
                "losing refreshes must use JSON content type"
            );
            assert!(
                json["access_token"].is_null(),
                "a losing refresh must not return a token"
            );
            rejected += 1;
        }
    }

    assert_eq!(successes, 1, "exactly one HTTP refresh must succeed");
    assert_eq!(rejected, 7, "every other HTTP refresh must fail");

    let winner = winner.expect("the winner must return a token");
    let (status, _, _) = authenticated_get(&router, "/api/v1/auth/me", &winner).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "only the winning replacement token must authenticate"
    );

    let (status, _, _) = authenticated_get(&router, "/api/v1/auth/me", &token).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the original token must fail after refresh"
    );
}

#[tokio::test]
async fn auth_failures_carry_request_id_bearer_challenge_and_json() {
    let router = router().await;

    let cases: Vec<(String, &str)> = vec![
        ("missing authorization".to_string(), ""),
        ("invalid token".to_string(), "not-a-real-token"),
    ];
    for (label, token) in cases {
        let (status, headers, json) = authenticated_get(&router, "/api/v1/auth/me", token).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{label} must be 401");
        assert_eq!(headers["www-authenticate"], "Bearer", "{label}");
        assert_eq!(headers["content-type"], "application/json", "{label}");
        assert!(
            json["error"]["request_id"].is_string(),
            "{label} must carry a request id"
        );
    }

    let (status, headers, json) = post_refresh(&router, "not-a-real-token").await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "refresh failure must be 401"
    );
    assert_eq!(headers["www-authenticate"], "Bearer");
    assert_eq!(headers["content-type"], "application/json");
    assert!(
        json["error"]["request_id"].is_string(),
        "refresh failure must carry a request id"
    );

    let (status, headers, json) = logout_with_token(&router, "not-a-real-token").await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "logout failure must be 401"
    );
    assert_eq!(headers["www-authenticate"], "Bearer");
    assert_eq!(headers["content-type"], "application/json");
    assert!(
        json["error"]["request_id"].is_string(),
        "logout failure must carry a request id"
    );
}

#[tokio::test]
async fn forbidden_role_carries_request_id() {
    let router = router().await;
    let token = student_token(&router).await;
    let (status, headers, json) = authenticated_get(&router, "/roles/admin-only", &token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(headers["www-authenticate"], "Bearer");
    assert_eq!(headers["content-type"], "application/json");
    assert!(
        json["error"]["request_id"].is_string(),
        "a forbidden response must carry a request id"
    );
}

#[tokio::test]
async fn login_body_error_carries_request_id() {
    let router = router().await;
    let (status, _, json) = post(
        &router,
        "/api/v1/auth/login",
        serde_json::json!({ "email": "ana@example.com" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "INVALID_REQUEST_BODY");
    assert!(
        json["error"]["request_id"].is_string(),
        "a 400 body error must carry a request id"
    );
}

#[tokio::test]
async fn openapi_auth_security_is_correct() {
    let (_, _, json) = get("/api/openapi.json").await;
    let paths = json["paths"].as_object().unwrap();

    let login = &paths["/api/v1/auth/login"]["post"];
    assert!(
        login["security"].is_null()
            || login["security"]
                .as_array()
                .is_some_and(|items| items.is_empty()),
        "login must remain public"
    );

    for (path, method) in [
        ("/api/v1/auth/refresh", "post"),
        ("/api/v1/auth/logout", "post"),
        ("/api/v1/auth/me", "get"),
    ] {
        let operation = &paths[path][method];
        let security = operation["security"]
            .as_array()
            .unwrap_or_else(|| panic!("{path} must declare a security requirement"));
        assert!(!security.is_empty(), "{path} must require bearer security");
        assert!(
            security
                .iter()
                .any(|requirement| requirement["auth_bearer"].is_array()),
            "{path} must reference the auth_bearer scheme"
        );
    }
}

#[tokio::test]
async fn openapi_documents_all_catalog_mutation_paths() {
    let (_, _, json) = get("/api/openapi.json").await;
    let paths = json["paths"].as_object().unwrap();
    for (path, methods) in [
        ("/api/v1/products", vec!["get", "post"]),
        ("/api/v1/products/{product_id}", vec!["get", "patch"]),
        ("/api/v1/products/{product_id}/availability", vec!["patch"]),
        ("/api/v1/categories", vec!["get", "post"]),
        ("/api/v1/categories/{category_id}", vec!["get", "patch"]),
    ] {
        let operations = paths
            .get(path)
            .unwrap_or_else(|| panic!("missing OpenAPI path: {path}"));
        for method in methods {
            assert!(
                operations.get(method).is_some(),
                "missing {method} on {path}"
            );
        }
    }
}

#[tokio::test]
async fn openapi_mutations_require_bearer_but_public_reads_do_not() {
    let (_, _, json) = get("/api/openapi.json").await;
    let paths = json["paths"].as_object().unwrap();

    for (path, method) in [
        ("/api/v1/products", "post"),
        ("/api/v1/products/{product_id}", "patch"),
        ("/api/v1/products/{product_id}/availability", "patch"),
        ("/api/v1/categories", "post"),
        ("/api/v1/categories/{category_id}", "patch"),
    ] {
        let security = paths[path][method]["security"]
            .as_array()
            .unwrap_or_else(|| panic!("{method} {path} must declare security"));
        assert!(!security.is_empty(), "{method} {path} must require bearer");
        assert!(
            security
                .iter()
                .any(|requirement| requirement["auth_bearer"].is_array()),
            "{method} {path} must reference auth_bearer"
        );
    }

    for (path, method) in [("/api/v1/products", "get"), ("/api/v1/categories", "get")] {
        let security = &paths[path][method]["security"];
        assert!(
            security.is_null() || security.as_array().is_some_and(|items| items.is_empty()),
            "public {method} {path} must not require bearer"
        );
    }
}

#[tokio::test]
async fn openapi_documents_mutation_request_and_response_schemas() {
    let (_, _, json) = get("/api/openapi.json").await;
    let schemas = json["components"]["schemas"].as_object().unwrap();
    for schema in [
        "CreateProductRequest",
        "UpdateProductRequest",
        "SetProductAvailabilityRequest",
        "CreateCategoryRequest",
        "UpdateCategoryRequest",
        "ProductResponse",
        "CategoryResponse",
        "ErrorResponse",
    ] {
        assert!(
            schemas.contains_key(schema),
            "missing OpenAPI schema: {schema}"
        );
    }
}

#[tokio::test]
async fn openapi_create_product_has_201_and_location_and_error_schemas() {
    let (_, _, json) = get("/api/openapi.json").await;
    let operation = &json["paths"]["/api/v1/products"]["post"];
    let responses = operation["responses"].as_object().unwrap();
    assert!(
        responses.contains_key("201"),
        "create product must document a 201 response"
    );
    let created = &responses["201"];
    let ref_name = created["content"]["application/json"]["schema"]["$ref"]
        .as_str()
        .unwrap();
    assert_eq!(ref_name, "#/components/schemas/ProductResponse");
    let headers = created["headers"].as_object().unwrap();
    assert!(
        headers.contains_key("Location"),
        "201 must document the Location header"
    );
    for status in ["400", "401", "403", "404", "422", "500"] {
        let ref_name = responses[status]["content"]["application/json"]["schema"]["$ref"]
            .as_str()
            .unwrap();
        assert_eq!(
            ref_name, "#/components/schemas/ErrorResponse",
            "status {status} must reference the standardized error schema"
        );
    }
}

/// POSTs `/api/v1/auth/logout` with the given bearer token.
async fn logout_with_token(
    router: &axum::Router<()>,
    token: &str,
) -> (StatusCode, axum::http::HeaderMap, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/logout")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap()
    };
    (status, headers, json)
}

#[tokio::test]
async fn canteen_can_create_a_product_over_http() {
    let router = router().await;
    let token = canteen_token(&router).await;

    let (status, headers, json) = authed_request(
        &router,
        "POST",
        "/api/v1/products",
        &token,
        Some(serde_json::json!({
            "name": "Sanduíche natural",
            "description": "Pão integral, frango e salada",
            "price": 1200,
            "category_id": "category_snacks",
            "available": true,
        })),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(
        headers["location"]
            .to_str()
            .unwrap()
            .starts_with("/api/v1/products/")
    );
    assert_eq!(json["name"], "Sanduíche natural");
    assert_eq!(json["price"], 1200);
    assert_eq!(json["available"], true);
}

#[tokio::test]
async fn product_available_defaults_to_true_when_omitted() {
    let router = router().await;
    let token = canteen_token(&router).await;

    let (status, _, json) = authed_request(
        &router,
        "POST",
        "/api/v1/products",
        &token,
        Some(serde_json::json!({
            "name": "Bolo de cenoura",
            "description": "",
            "price": 700,
            "category_id": "category_snacks",
        })),
    )
    .await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["available"], true);
}

#[tokio::test]
async fn admin_can_create_a_product_over_http() {
    let router = router().await;
    let token = admin_token(&router).await;
    let (status, _, _) = authed_request(
        &router,
        "POST",
        "/api/v1/products",
        &token,
        Some(serde_json::json!({
            "name": "Suco de laranja",
            "description": "",
            "price": 400,
            "category_id": "category_drinks",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn create_product_without_token_is_unauthorized() {
    let router = router().await;
    let (status, headers, json) = authed_request(
        &router,
        "POST",
        "/api/v1/products",
        "",
        Some(serde_json::json!({
            "name": "Bolo",
            "description": "",
            "price": 700,
            "category_id": "category_snacks",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(json["error"]["code"], "AUTHENTICATION_REQUIRED");
    assert_eq!(headers["www-authenticate"], "Bearer");
    assert!(json["error"]["request_id"].is_string());
}

#[tokio::test]
async fn student_cannot_create_a_product_over_http() {
    let router = router().await;
    let token = student_token(&router).await;
    let (status, headers, json) = authed_request(
        &router,
        "POST",
        "/api/v1/products",
        &token,
        Some(serde_json::json!({
            "name": "Bolo",
            "description": "",
            "price": 700,
            "category_id": "category_snacks",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"]["code"], "FORBIDDEN");
    assert_eq!(headers["www-authenticate"], "Bearer");
}

#[tokio::test]
async fn malformed_product_body_is_a_400() {
    let router = router().await;
    let token = canteen_token(&router).await;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/products")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from("{ not json"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "INVALID_REQUEST_BODY");
    assert!(json["error"]["request_id"].is_string());
}

#[tokio::test]
async fn oversized_product_body_is_rejected() {
    let router = router().await;
    let token = canteen_token(&router).await;
    let huge_name = "a".repeat(40 * 1024);
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/products")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(format!(
                    "{{\"name\":\"{huge_name}\",\"description\":\"\",\"price\":100,\"category_id\":\"category_snacks\"}}"
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["code"], "INVALID_REQUEST_BODY");
    assert!(json["error"]["request_id"].is_string());
}

#[tokio::test]
async fn invalid_product_fields_are_a_422() {
    let router = router().await;
    let token = canteen_token(&router).await;
    let (status, _, json) = authed_request(
        &router,
        "POST",
        "/api/v1/products",
        &token,
        Some(serde_json::json!({
            "name": "Bolo",
            "description": "",
            "price": 0,
            "category_id": "category_snacks",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json["error"]["code"], "INVALID_PRODUCT_PRICE");
    assert!(json["error"]["request_id"].is_string());
}

#[tokio::test]
async fn create_product_with_unknown_category_is_404() {
    let router = router().await;
    let token = canteen_token(&router).await;
    let (status, _, json) = authed_request(
        &router,
        "POST",
        "/api/v1/products",
        &token,
        Some(serde_json::json!({
            "name": "Bolo",
            "description": "",
            "price": 700,
            "category_id": "category_missing",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "NOT_FOUND");
    assert!(json["error"]["request_id"].is_string());
}

#[tokio::test]
async fn newly_created_product_appears_in_public_listing() {
    let router = router().await;
    let token = canteen_token(&router).await;
    let (status, _, created) = authed_request(
        &router,
        "POST",
        "/api/v1/products",
        &token,
        Some(serde_json::json!({
            "name": "Escondidinho",
            "description": "Mashed potato with meat",
            "price": 1100,
            "category_id": "category_snacks",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap().to_string();

    let (status, _, list) = get_from(&router, "/api/v1/products").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        list["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == id)
    );
}

#[tokio::test]
async fn canteen_can_update_a_product_over_http() {
    let router = router().await;
    let token = canteen_token(&router).await;

    let (status, _, json) = authed_request(
        &router,
        "PATCH",
        "/api/v1/products/product_coxinha",
        &token,
        Some(serde_json::json!({
            "name": "Coxinha grande",
            "price": 900,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["name"], "Coxinha grande");
    assert_eq!(json["price"], 900);
    assert_eq!(json["description"], "Fried shredded-chicken pastry");
}

#[tokio::test]
async fn empty_product_update_is_a_422() {
    let router = router().await;
    let token = canteen_token(&router).await;
    let (status, _, json) = authed_request(
        &router,
        "PATCH",
        "/api/v1/products/product_coxinha",
        &token,
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json["error"]["code"], "EMPTY_UPDATE");
    assert!(json["error"]["request_id"].is_string());
}

#[tokio::test]
async fn update_unknown_product_is_a_404() {
    let router = router().await;
    let token = canteen_token(&router).await;
    let (status, _, json) = authed_request(
        &router,
        "PATCH",
        "/api/v1/products/product_missing",
        &token,
        Some(serde_json::json!({ "name": "Novo" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"]["code"], "NOT_FOUND");
    assert!(json["error"]["request_id"].is_string());
}

#[tokio::test]
async fn student_cannot_update_a_product_over_http() {
    let router = router().await;
    let token = student_token(&router).await;
    let (status, _, json) = authed_request(
        &router,
        "PATCH",
        "/api/v1/products/product_coxinha",
        &token,
        Some(serde_json::json!({ "name": "Novo" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"]["code"], "FORBIDDEN");
}

#[tokio::test]
async fn canteen_can_disable_and_enable_a_product_over_http() {
    let router = router().await;
    let token = canteen_token(&router).await;

    let (status, _, json) = authed_request(
        &router,
        "PATCH",
        "/api/v1/products/product_coxinha/availability",
        &token,
        Some(serde_json::json!({ "available": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["available"], false);

    let (status, _, json) = authed_request(
        &router,
        "PATCH",
        "/api/v1/products/product_coxinha/availability",
        &token,
        Some(serde_json::json!({ "available": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["available"], true);
}

#[tokio::test]
async fn availability_disable_is_idempotent() {
    let router = router().await;
    let token = canteen_token(&router).await;
    let (_, _, first) = authed_request(
        &router,
        "PATCH",
        "/api/v1/products/product_pastel/availability",
        &token,
        Some(serde_json::json!({ "available": false })),
    )
    .await;
    let (status, _, second) = authed_request(
        &router,
        "PATCH",
        "/api/v1/products/product_pastel/availability",
        &token,
        Some(serde_json::json!({ "available": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["available"], false);
    assert_eq!(second["available"], false);
}

#[tokio::test]
async fn disabled_product_disappears_from_default_listing_but_shows_when_requested() {
    let router = router().await;
    let token = canteen_token(&router).await;
    let (status, _, _) = authed_request(
        &router,
        "PATCH",
        "/api/v1/products/product_coxinha/availability",
        &token,
        Some(serde_json::json!({ "available": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, _, default) = get_from(&router, "/api/v1/products").await;
    assert!(
        !default["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "product_coxinha")
    );

    let (_, _, all) = get_from(&router, "/api/v1/products?include_unavailable=true").await;
    assert!(
        all["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "product_coxinha" && item["available"] == false)
    );
}

#[tokio::test]
async fn admin_can_create_a_category_over_http() {
    let router = router().await;
    let token = admin_token(&router).await;
    let (status, headers, json) = authed_request(
        &router,
        "POST",
        "/api/v1/categories",
        &token,
        Some(serde_json::json!({ "name": "Combos" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(
        headers["location"]
            .to_str()
            .unwrap()
            .starts_with("/api/v1/categories/")
    );
    assert_eq!(json["name"], "Combos");
}

#[tokio::test]
async fn canteen_cannot_create_a_category_over_http() {
    let router = router().await;
    let token = canteen_token(&router).await;
    let (status, _, json) = authed_request(
        &router,
        "POST",
        "/api/v1/categories",
        &token,
        Some(serde_json::json!({ "name": "Combos" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"]["code"], "FORBIDDEN");
}

#[tokio::test]
async fn student_cannot_create_a_category_over_http() {
    let router = router().await;
    let token = student_token(&router).await;
    let (status, _, json) = authed_request(
        &router,
        "POST",
        "/api/v1/categories",
        &token,
        Some(serde_json::json!({ "name": "Combos" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"]["code"], "FORBIDDEN");
}

#[tokio::test]
async fn admin_can_update_a_category_over_http() {
    let router = router().await;
    let token = admin_token(&router).await;
    let (status, _, json) = authed_request(
        &router,
        "PATCH",
        "/api/v1/categories/category_snacks",
        &token,
        Some(serde_json::json!({ "name": "Lanches e combos" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["name"], "Lanches e combos");

    let (_, _, read) = get_from(&router, "/api/v1/categories").await;
    assert!(
        read["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == "category_snacks" && item["name"] == "Lanches e combos")
    );
}

#[tokio::test]
async fn canteen_cannot_update_a_category_over_http() {
    let router = router().await;
    let token = canteen_token(&router).await;
    let (status, _, json) = authed_request(
        &router,
        "PATCH",
        "/api/v1/categories/category_snacks",
        &token,
        Some(serde_json::json!({ "name": "Lanches" })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(json["error"]["code"], "FORBIDDEN");
}

#[tokio::test]
async fn duplicate_normalized_category_name_is_a_409() {
    let router = router().await;
    let token = admin_token(&router).await;
    let (status, _, json) = authed_request(
        &router,
        "POST",
        "/api/v1/categories",
        &token,
        Some(serde_json::json!({ "name": "snacks" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(json["error"]["code"], "CATEGORY_NAME_CONFLICT");
    assert!(json["error"]["request_id"].is_string());
}

#[tokio::test]
async fn public_reads_remain_public_and_json() {
    let (status, headers, _) = get("/api/v1/products").await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers["content-type"]
            .to_str()
            .unwrap()
            .contains("application/json")
    );

    let (status, _, _) = get("/api/v1/categories").await;
    assert_eq!(status, StatusCode::OK);
}
