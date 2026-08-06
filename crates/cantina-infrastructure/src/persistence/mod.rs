//! Repository adapters implementing the application ports.

pub mod in_memory;
pub mod in_memory_credentials;
pub mod in_memory_sessions;

pub use in_memory::{
    InMemoryCategoryRepository, InMemoryOrderRepository, InMemoryProductRepository,
    InMemoryRepositories, InMemoryUserRepository,
};
pub use in_memory_credentials::InMemoryCredentialRepository;
pub use in_memory_sessions::InMemorySessionRepository;

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use cantina_application::ports::RepositoryError;

/// A string-keyed in-memory store guarded by a `Mutex`.
pub(crate) type Store<T> = Mutex<HashMap<String, T>>;

/// Locks an in-memory store, mapping a poisoned lock to a typed error.
pub(crate) fn lock_store<T>(
    store: &Store<T>,
) -> Result<MutexGuard<'_, HashMap<String, T>>, RepositoryError> {
    store
        .lock()
        .map_err(|_| RepositoryError::new("repository lock poisoned"))
}
