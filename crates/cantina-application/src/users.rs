//! User-facing DTOs.

use cantina_core::users::{User, UserId, UserRole, UserStatus};

/// Application representation of a [`User`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserDto {
    pub id: UserId,
    pub name: String,
    pub email: String,
    pub role: UserRole,
    pub status: UserStatus,
}

impl From<User> for UserDto {
    fn from(user: User) -> Self {
        Self {
            id: user.id().clone(),
            name: user.name().to_string(),
            email: user.email().to_string(),
            role: user.role(),
            status: user.status(),
        }
    }
}
