//! Authentication use cases.
//!
//! `AuthenticateUser` and `GetCurrentUser`. Real credential verification is
//! intentionally not implemented; only the contracts are prepared.

use std::sync::Arc;

use cantina_core::auth::Principal;

use crate::error::ApplicationError;
use crate::ports::UserRepository;
use crate::users::UserDto;

/// Command for the `AuthenticateUser` use case.
///
/// The exact payload will be defined together with the credential scheme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticateUserCommand {
    /// User-supplied identifier (e-mail or username).
    pub identifier: String,
}

/// Coordinates authentication use cases.
pub struct AuthService {
    users: Arc<dyn UserRepository>,
}

impl AuthService {
    /// Creates the service with its user repository.
    pub fn new(users: Arc<dyn UserRepository>) -> Self {
        Self { users }
    }

    /// `AuthenticateUser` use case. Not implemented in the foundation.
    pub async fn authenticate_user(
        &self,
        _command: AuthenticateUserCommand,
    ) -> Result<Principal, ApplicationError> {
        Err(ApplicationError::NotImplemented {
            feature: "authentication",
        })
    }

    /// `GetCurrentUser` use case: resolves a [`Principal`] to its [`User`].
    pub async fn get_current_user(
        &self,
        principal: &Principal,
    ) -> Result<UserDto, ApplicationError> {
        let user = self
            .users
            .find_user_by_id(principal.user_id())
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "user",
                id: principal.user_id().to_string(),
            })?;
        Ok(user.into())
    }
}
