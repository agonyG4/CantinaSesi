//! In-memory adapter for the `CredentialRepository` port.
//!
//! Stores only password hashes (PHC strings), never plaintext passwords.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use cantina_application::ports::{CredentialRepository, RepositoryError, StoredPasswordHash};
use cantina_core::users::UserId;

use super::{Store, lock_store};

/// In-memory adapter for stored password hashes.
pub struct InMemoryCredentialRepository {
    hashes: Store<StoredPasswordHash>,
}

impl InMemoryCredentialRepository {
    /// Creates an empty in-memory credential repository.
    pub fn new() -> Self {
        Self {
            hashes: Mutex::new(HashMap::new()),
        }
    }

    /// Stores the password hash for a user.
    pub fn insert(
        &self,
        user_id: &UserId,
        hash: StoredPasswordHash,
    ) -> Result<(), RepositoryError> {
        lock_store(&self.hashes)?.insert(user_id.as_str().to_string(), hash);
        Ok(())
    }
}

impl Default for InMemoryCredentialRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CredentialRepository for InMemoryCredentialRepository {
    async fn find_password_hash_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<StoredPasswordHash>, RepositoryError> {
        Ok(lock_store(&self.hashes)?.get(user_id.as_str()).cloned())
    }
}
