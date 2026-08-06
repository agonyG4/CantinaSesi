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

    /// Returns `true` when the principal carries exactly the given role.
    pub fn has_role(&self, role: UserRole) -> bool {
        self.role == role
    }

    /// Returns `true` when the principal carries any of the given roles.
    pub fn has_any_role(&self, roles: &[UserRole]) -> bool {
        roles.contains(&self.role)
    }
}

/// Error returned when a principal is not allowed to perform an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationError {
    /// The principal's role does not permit the requested action.
    Forbidden,
}

impl fmt::Display for AuthorizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Forbidden => write!(f, "forbidden"),
        }
    }
}

impl Error for AuthorizationError {}

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
    use super::{AuthError, AuthorizationError, Principal};
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

    #[test]
    fn principal_has_role_matches_its_own_role() {
        let student = Principal::new(UserId::new("user_1"), UserRole::Student);
        let admin = Principal::new(UserId::new("user_2"), UserRole::Admin);

        assert!(student.has_role(UserRole::Student));
        assert!(!student.has_role(UserRole::Canteen));
        assert!(!student.has_role(UserRole::Admin));
        assert!(admin.has_role(UserRole::Admin));
    }

    #[test]
    fn principal_has_any_role_matches_a_given_role() {
        let canteen = Principal::new(UserId::new("user_1"), UserRole::Canteen);
        assert!(canteen.has_any_role(&[UserRole::Canteen, UserRole::Admin]));
        assert!(canteen.has_any_role(&[UserRole::Student, UserRole::Canteen]));
        assert!(!canteen.has_any_role(&[UserRole::Student]));
        assert!(!canteen.has_any_role(&[UserRole::Admin]));
        assert!(!canteen.has_any_role(&[]));
    }

    #[test]
    fn student_does_not_satisfy_canteen_or_admin() {
        let student = Principal::new(UserId::new("user_1"), UserRole::Student);
        assert!(student.has_role(UserRole::Student));
        assert!(!student.has_any_role(&[UserRole::Canteen, UserRole::Admin]));
    }

    #[test]
    fn authorization_error_is_typed_forbidden() {
        let err = AuthorizationError::Forbidden;
        assert_eq!(err, AuthorizationError::Forbidden);
        assert_eq!(err.to_string(), "forbidden");
    }
}
