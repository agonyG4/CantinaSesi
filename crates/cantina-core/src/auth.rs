//! Domain-level authentication concepts.
//!
//! Real authentication (passwords, JWT, sessions, token storage) is
//! intentionally out of scope for the foundation. Only the concepts and
//! error types the domain needs are defined here.

use std::error::Error;
use std::fmt;

use crate::users::{UserId, UserRole};

/// An authenticated actor.
///
/// This is what the API will eventually attach to a request once real
/// authentication middleware exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    user_id: UserId,
    role: UserRole,
}

impl Principal {
    /// Creates a principal for a user.
    pub fn new(user_id: UserId, role: UserRole) -> Self {
        Self { user_id, role }
    }

    /// Returns the underlying user identifier.
    pub fn user_id(&self) -> &UserId {
        &self.user_id
    }

    /// Returns the role of the principal.
    pub fn role(&self) -> UserRole {
        self.role
    }
}

/// Domain error raised during authentication flows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// No user matches the supplied credentials.
    UserNotFound,
    /// The user exists but is inactive.
    AccountInactive,
    /// The supplied credentials are invalid.
    InvalidCredentials,
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UserNotFound => write!(f, "user not found"),
            Self::AccountInactive => write!(f, "account is inactive"),
            Self::InvalidCredentials => write!(f, "invalid credentials"),
        }
    }
}

impl Error for AuthError {}

#[cfg(test)]
mod tests {
    use super::{AuthError, Principal};
    use crate::users::{UserId, UserRole};

    #[test]
    fn principal_exposes_identity() {
        let principal = Principal::new(UserId::new("user_1"), UserRole::Canteen);
        assert_eq!(principal.user_id().as_str(), "user_1");
        assert_eq!(principal.role(), UserRole::Canteen);
    }

    #[test]
    fn auth_errors_are_typed() {
        assert_eq!(AuthError::UserNotFound.to_string(), "user not found");
        assert_eq!(
            AuthError::AccountInactive.to_string(),
            "account is inactive"
        );
    }
}
