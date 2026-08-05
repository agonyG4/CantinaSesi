//! Authentication endpoints (prepared, not implemented).

use axum::Router;

use crate::state::AppState;

/// Prepares the `/api/v1/auth` router.
pub fn router() -> Router<AppState> {
    Router::new()
}
