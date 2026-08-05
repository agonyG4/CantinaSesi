//! Application state: composed services injected into every handler.
//!
//! The state only holds already-composed services. How those services are
//! built (which repositories and generators back them) is decided by the
//! composition root in [`crate::bootstrap`], keeping this module free of any
//! infrastructure dependency.

use std::sync::Arc;

use cantina_application::auth::AuthService;
use cantina_application::catalog::CatalogService;
use cantina_application::orders::OrderService;

/// Immutable application state shared by all requests.
#[derive(Clone)]
pub struct AppState {
    pub catalog: Arc<CatalogService>,
    pub orders: Arc<OrderService>,
    pub auth: Arc<AuthService>,
}

impl AppState {
    /// Creates the state from already-composed services.
    pub fn new(
        catalog: Arc<CatalogService>,
        orders: Arc<OrderService>,
        auth: Arc<AuthService>,
    ) -> Self {
        Self {
            catalog,
            orders,
            auth,
        }
    }
}
