//! HTTP routes. Each module is a versioned resource router.

pub mod auth;
pub mod canteen;
pub mod categories;
pub mod health;
pub mod openapi;
pub mod orders;
pub mod products;
pub mod users;

use axum::Router;

use crate::state::AppState;

/// Assembles the versioned `v1` API tree.
pub fn api_v1() -> Router<AppState> {
    Router::new()
        .route("/health", axum::routing::get(health::versioned_health))
        .nest("/auth", auth::router())
        .nest("/users", users::router())
        .nest("/products", products::router())
        .nest("/categories", categories::router())
        .nest("/orders", orders::router())
        .nest("/canteen", canteen::router())
}
