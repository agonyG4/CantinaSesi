//! Integration tests over the HTTP adapter.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::util::ServiceExt;

use cantina_api::app;
use cantina_api::state::AppState;
use cantina_infrastructure::config::Config;
use cantina_infrastructure::persistence::InMemoryRepositories;

async fn router() -> axum::Router<()> {
    let repositories = InMemoryRepositories::with_seed_data();
    let state = AppState::new(repositories);
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
async fn versioned_resources_are_mounted() {
    let (status, _, _) = raw_get("/api/v1/auth").await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "empty prepared routers answer 404"
    );

    let (status, _, _) = raw_get("/api/v1/products").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no collection route yet");
}
