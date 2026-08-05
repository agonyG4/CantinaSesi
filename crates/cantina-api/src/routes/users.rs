//! User endpoints (prepared, not implemented).

use axum::Router;

use crate::state::AppState;

/// Prepares the `/api/v1/users` router.
pub fn router() -> Router<AppState> {
    Router::new()
}
