//! Composition root.
//!
//! Wires the infrastructure adapters into the application services and the
//! [`AppState`]. Keeping this in one place makes swapping the in-memory
//! repositories for a real database a single-file change.

use std::sync::Arc;

use cantina_application::auth::AuthService;
use cantina_application::catalog::CatalogService;
use cantina_application::orders::OrderService;
use cantina_application::ports::{
    Clock, CredentialRepository, IdGenerator, PasswordService, PickupCodeGenerator,
    SessionRepository, SessionTokenService, UserRepository,
};
use cantina_infrastructure::config::Config;
use cantina_infrastructure::identifiers::{DefaultPickupCodeGenerator, Generator};
use cantina_infrastructure::persistence::InMemoryRepositories;
use cantina_infrastructure::security::passwords::Argon2PasswordService;
use cantina_infrastructure::security::tokens::OsRngSessionTokenService;
use cantina_infrastructure::time::SystemClock;

use crate::state::AppState;

/// Composes the application state on top of the given repositories and config.
pub fn build_state(repositories: InMemoryRepositories, config: &Config) -> AppState {
    let ids: Arc<dyn IdGenerator> = Arc::new(Generator::new());
    let pickup_codes: Arc<dyn PickupCodeGenerator> = Arc::new(DefaultPickupCodeGenerator::new());
    let clock: Arc<dyn Clock> = Arc::new(SystemClock);

    let passwords: Arc<dyn PasswordService> = Arc::new(Argon2PasswordService::new());
    let tokens: Arc<dyn SessionTokenService> = Arc::new(OsRngSessionTokenService);
    let users: Arc<dyn UserRepository> = repositories.users.clone();
    let credentials: Arc<dyn CredentialRepository> = repositories.credentials.clone();
    let sessions: Arc<dyn SessionRepository> = repositories.sessions.clone();

    let auth = AuthService::new(
        users,
        credentials,
        sessions,
        passwords,
        tokens,
        clock.clone(),
        config.session_config(),
    );

    AppState::new(
        Arc::new(CatalogService::new(
            repositories.products.clone(),
            repositories.categories.clone(),
            ids.clone(),
        )),
        Arc::new(OrderService::new(
            repositories.orders.clone(),
            repositories.products.clone(),
            ids,
            pickup_codes,
            clock,
        )),
        Arc::new(auth),
    )
}
