//! Typed application errors.

use thiserror::Error;

use crate::ports::{RepositoryError, SessionRotationError};

/// Error returned by application use cases.
///
/// Domain errors are translated into this type without losing meaning.
#[derive(Debug, Error)]
pub enum ApplicationError {
    /// A requested entity does not exist.
    #[error("the {resource} with id '{id}' was not found")]
    NotFound { resource: &'static str, id: String },

    /// The product cannot be ordered right now (unknown or unavailable).
    #[error("product '{product_id}' is not available")]
    ProductUnavailable { product_id: String },

    /// The order must contain at least one item.
    #[error("an order must contain at least one item")]
    EmptyOrder,

    /// An item quantity is not valid.
    #[error("item quantity must be greater than zero")]
    InvalidQuantity,

    /// The requested order status change is not allowed.
    #[error("cannot transition order from '{from}' to '{to}'")]
    InvalidStatusTransition { from: String, to: String },

    /// A use case that has not been implemented yet.
    #[error("not implemented yet: {feature}")]
    NotImplemented { feature: &'static str },

    /// The supplied email or password did not match.
    #[error("invalid email or password")]
    InvalidCredentials,

    /// The account exists but has been disabled.
    #[error("this account is inactive")]
    AccountInactive,

    /// The request carried no usable authentication.
    #[error("authentication is required")]
    AuthenticationRequired,

    /// The access token does not correspond to a valid session.
    #[error("invalid access token")]
    InvalidAccessToken,

    /// The session has expired and must be re-issued.
    #[error("the session has expired")]
    SessionExpired,

    /// The authenticated principal is not allowed to perform the action.
    #[error("forbidden")]
    Forbidden,

    /// Password hashing or verification could not complete.
    #[error("password processing failed")]
    PasswordProcessing,

    /// A session token could not be generated or fingerprinted.
    #[error("session token generation failed")]
    TokenGeneration,

    /// The storage backend failed.
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<RepositoryError> for ApplicationError {
    fn from(err: RepositoryError) -> Self {
        Self::Storage(err.to_string())
    }
}

impl From<SessionRotationError> for ApplicationError {
    fn from(err: SessionRotationError) -> Self {
        match err {
            // Stale or already-used refresh sessions must look exactly like an
            // invalid access token; internal repository failures stay internal.
            SessionRotationError::OldSessionMissing
            | SessionRotationError::OldSessionRevoked
            | SessionRotationError::NewFingerprintCollision => Self::InvalidAccessToken,
            SessionRotationError::Internal(message) => Self::Storage(message),
        }
    }
}

impl From<cantina_core::orders::OrderError> for ApplicationError {
    fn from(err: cantina_core::orders::OrderError) -> Self {
        use cantina_core::orders::OrderError;
        match err {
            OrderError::EmptyOrder => Self::EmptyOrder,
            OrderError::InvalidQuantity => Self::InvalidQuantity,
            OrderError::InvalidStatusTransition { from, to } => Self::InvalidStatusTransition {
                from: from.to_string(),
                to: to.to_string(),
            },
        }
    }
}
