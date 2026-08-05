//! Application state: composed services injected into every handler.

use std::sync::Arc;

use cantina_application::auth::AuthService;
use cantina_application::catalog::CatalogService;
use cantina_application::orders::OrderService;
use cantina_infrastructure::identifiers::{DefaultPickupCodeGenerator, Generator};
use cantina_infrastructure::persistence::InMemoryRepositories;
use cantina_infrastructure::time::SystemClock;

/// Immutable application state shared by all requests.
#[derive(Clone)]
pub struct AppState {
    pub catalog: Arc<CatalogService>,
    pub orders: Arc<OrderService>,
    pub auth: Arc<AuthService>,
}

impl AppState {
    /// Composes the application services on top of the given repositories.
    pub fn new(repositories: InMemoryRepositories) -> Self {
        let ids: Arc<dyn cantina_application::ports::IdGenerator> = Arc::new(Generator::new());
        let pickup_codes: Arc<dyn cantina_application::ports::PickupCodeGenerator> =
            Arc::new(DefaultPickupCodeGenerator::new());
        let clock: Arc<dyn cantina_application::ports::Clock> = Arc::new(SystemClock);

        Self {
            catalog: Arc::new(CatalogService::new(
                repositories.products.clone(),
                repositories.categories.clone(),
            )),
            orders: Arc::new(OrderService::new(
                repositories.orders.clone(),
                repositories.products.clone(),
                ids,
                pickup_codes,
                clock,
            )),
            auth: Arc::new(AuthService::new(repositories.users.clone())),
        }
    }
}
