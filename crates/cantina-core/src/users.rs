//! Registered users and their roles.

use std::error::Error;
use std::fmt;
use std::str::FromStr;

/// Strongly typed identifier for a [`User`].
///
/// Identifiers are opaque strings. Prefer generating them through the
/// application `IdGenerator` port instead of building them by hand.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserId(String);

impl UserId {
    /// Creates a user identifier from a raw string value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the underlying identifier value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Role assigned to a [`User`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserRole {
    /// Student ordering from the catalog.
    Student,
    /// Cafeteria staff fulfilling orders.
    Canteen,
    /// System administrator.
    Admin,
}

impl UserRole {
    /// Returns the canonical lowercase representation of the role.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Student => "student",
            Self::Canteen => "canteen",
            Self::Admin => "admin",
        }
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for UserRole {
    type Err = ParseRoleError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "student" => Ok(Self::Student),
            "canteen" => Ok(Self::Canteen),
            "admin" => Ok(Self::Admin),
            other => Err(ParseRoleError {
                value: other.to_string(),
            }),
        }
    }
}

/// Error returned when a [`UserRole`] cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseRoleError {
    value: String,
}

impl fmt::Display for ParseRoleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown user role '{}'", self.value)
    }
}

impl Error for ParseRoleError {}

/// Lifecycle state of a [`User`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserStatus {
    /// The user may use the platform.
    Active,
    /// The user has been disabled and must not be served.
    Inactive,
}

impl UserStatus {
    /// Returns the canonical lowercase representation of the status.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }
}

impl fmt::Display for UserStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A registered platform user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    id: UserId,
    name: String,
    email: String,
    role: UserRole,
    status: UserStatus,
}

impl User {
    /// Creates a new user.
    pub fn new(
        id: UserId,
        name: impl Into<String>,
        email: impl Into<String>,
        role: UserRole,
        status: UserStatus,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            email: email.into(),
            role,
            status,
        }
    }

    /// Returns the user identifier.
    pub fn id(&self) -> &UserId {
        &self.id
    }

    /// Returns the display name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the e-mail address.
    pub fn email(&self) -> &str {
        &self.email
    }

    /// Returns the assigned role.
    pub fn role(&self) -> UserRole {
        self.role
    }

    /// Returns the current status.
    pub fn status(&self) -> UserStatus {
        self.status
    }

    /// Returns `true` when the user can use the platform.
    pub fn is_active(&self) -> bool {
        self.status == UserStatus::Active
    }
}

#[cfg(test)]
mod tests {
    use super::{UserId, UserRole, UserStatus};
    use std::collections::HashMap;

    #[test]
    fn parses_known_roles_case_insensitively() {
        assert_eq!("student".parse::<UserRole>().unwrap(), UserRole::Student);
        assert_eq!("CANTEEN".parse::<UserRole>().unwrap(), UserRole::Canteen);
        assert_eq!("  Admin  ".parse::<UserRole>().unwrap(), UserRole::Admin);
    }

    #[test]
    fn rejects_unknown_roles() {
        let err = "cashier".parse::<UserRole>().unwrap_err();
        assert_eq!(err.to_string(), "unknown user role 'cashier'");
    }

    #[test]
    fn role_string_round_trips() {
        for role in [UserRole::Student, UserRole::Canteen, UserRole::Admin] {
            assert_eq!(role.as_str().parse::<UserRole>().unwrap(), role);
        }
    }

    #[test]
    fn user_ids_are_strongly_typed() {
        let a = UserId::new("user_1");
        let same = UserId::new("user_1");
        let other = UserId::new("user_2");

        assert_eq!(a, same);
        assert_ne!(a, other);
        assert_eq!(a.as_str(), "user_1");
        assert_eq!(a.to_string(), "user_1");

        let mut map = HashMap::new();
        map.insert(a.clone(), 1);
        map.insert(other, 2);
        assert_eq!(map[&a], 1);
    }

    #[test]
    fn user_tracks_status() {
        let user = super::User::new(
            UserId::new("user_1"),
            "Ana",
            "ana@example.com",
            UserRole::Student,
            UserStatus::Active,
        );
        assert!(user.is_active());
        assert_eq!(user.id().as_str(), "user_1");
        assert_eq!(user.role(), UserRole::Student);
    }
}
