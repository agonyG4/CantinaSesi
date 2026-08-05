//! Composition root.
//!
//! Wires the infrastructure adapters into the application services and the
//! [`AppState`]. Keeping this in one place makes swapping the in-memory
//! repositories for a real database a single-file change.

use std::sync::Arc;

use cantina_application::auth::AuthService;
use cantina_application::catalog::CatalogService;
use cantina_application::orders::OrderService;
use cantina_application::ports::{Clock, IdGenerator, PickupCodeGenerator};
use cantina_infrastructure::identifiers::{DefaultPickupCodeGenerator, Generator};
use cantina_infrastructure::persistence::InMemoryRepositories;
use cantina_infrastructure::time::SystemClock;

use crate::state::AppState;

/// Composes the application state on top of the given repositories.
pub fn build_state(repositories: InMemoryRepositories) -> AppState {
    let ids: Arc<dyn IdGenerator> = Arc::new(Generator::new());
    let pickup_codes: Arc<dyn PickupCodeGenerator> = Arc::new(DefaultPickupCodeGenerator::new());
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);

    AppState::new(
        Arc::new(CatalogService::new(
            repositories.products.clone(),
            repositories.categories.clone(),
        )),
        Arc::new(OrderService::new(
            repositories.orders.clone(),
            repositories.products.clone(),
            ids,
            pickup_codes,
            clock,
        )),
        Arc::new(AuthService::new(repositories.users.clone())),
    )
}
