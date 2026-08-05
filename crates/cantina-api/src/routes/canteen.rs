//! Canteen staff endpoints (prepared, not implemented).

use axum::Router;

use crate::state::AppState;

/// Prepares the `/api/v1/canteen` router.
pub fn router() -> Router<AppState> {
    Router::new()
}
