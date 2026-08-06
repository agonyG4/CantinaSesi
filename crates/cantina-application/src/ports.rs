//! Application-owned ports (interfaces owned by the application layer).
//!
//! The application decides exactly which operations it needs. The future
//! database implementation, and every other technical adapter, must adapt to
//! these interfaces — never the opposite. Ports are deliberately described
//! in domain terms (`find_user_by_email`, `save_order`) instead of SQL terms
//! (`execute_query`, `insert_record`).

use std::fmt;
use std::time::SystemTime;

use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

use cantina_core::catalog::{Category, CategoryId, Product, ProductId};
use cantina_core::orders::{Order, OrderId, PickupCode};
use cantina_core::users::{User, UserId};

/// Storage failure surfaced by a repository adapter.
#[derive(Debug, Error)]
#[error("storage failure: {0}")]
pub struct RepositoryError(String);

impl RepositoryError {
    /// Creates a storage error from a message.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// Repository for [`User`] aggregates.
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Finds a user by its identifier.
    async fn find_user_by_id(&self, id: &UserId) -> Result<Option<User>, RepositoryError>;

    /// Finds a user by its e-mail address.
    async fn find_user_by_email(&self, email: &str) -> Result<Option<User>, RepositoryError>;
}

/// Password hash stored for a user.
///
/// The value is the PHC string produced by the password hashing scheme. It is
/// treated as a secret: `Debug` never reveals the stored hash.
#[derive(Clone)]
pub struct StoredPasswordHash {
    phc: SecretString,
}

impl StoredPasswordHash {
    /// Wraps a PHC password-hash string.
    pub fn new(phc: SecretString) -> Self {
        Self { phc }
    }

    /// Exposes the PHC string at the narrow hashing/verification boundary.
    pub fn as_phc(&self) -> &str {
        self.phc.expose_secret()
    }
}

impl fmt::Debug for StoredPasswordHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StoredPasswordHash([REDACTED])")
    }
}

/// Repository for stored password hashes.
#[async_trait]
pub trait CredentialRepository: Send + Sync {
    /// Finds the password hash stored for a user.
    async fn find_password_hash_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<StoredPasswordHash>, RepositoryError>;
}

/// Server-side session record.
///
/// Only the SHA-256 fingerprint of the bearer token is stored; the plaintext
/// bearer token is never persisted.
#[derive(Debug, Clone)]
pub struct SessionRecord {
    /// Fingerprint of the bearer token identifying this session.
    pub token_fingerprint: String,
    /// The user the session belongs to.
    pub user_id: UserId,
    /// When the session was created.
    pub created_at: SystemTime,
    /// When the session was last used for authenticated activity.
    pub last_seen_at: SystemTime,
    /// Absolute expiration enforced server-side.
    pub absolute_expires_at: SystemTime,
    /// When the session was revoked, if it has been.
    pub revoked_at: Option<SystemTime>,
}

/// Repository for server-side sessions, addressed by token fingerprint.
///
/// Operations are described in domain terms (`revoke_session`,
/// `rotate_session`) rather than generic storage operations so the
/// application stays in control of the session lifecycle.
#[async_trait]
pub trait SessionRepository: Send + Sync {
    /// Persists a newly issued session.
    async fn save_session(&self, session: SessionRecord) -> Result<(), RepositoryError>;

    /// Finds a session by its token fingerprint.
    async fn find_session_by_fingerprint(
        &self,
        fingerprint: &str,
    ) -> Result<Option<SessionRecord>, RepositoryError>;

    /// Records authenticated activity on a session.
    async fn update_last_seen_at(
        &self,
        fingerprint: &str,
        at: SystemTime,
    ) -> Result<(), RepositoryError>;

    /// Revokes a session.
    async fn revoke_session(
        &self,
        fingerprint: &str,
        revoked_at: SystemTime,
    ) -> Result<(), RepositoryError>;

    /// Atomically revokes an old session and stores a new one.
    ///
    /// A single operation prevents a refresh from leaving inconsistent state
    /// (for example the new session stored without the old one being revoked).
    async fn rotate_session(
        &self,
        old_fingerprint: &str,
        new_session: SessionRecord,
        revoked_at: SystemTime,
    ) -> Result<(), RepositoryError>;
}

/// Hashing and verification of plain passwords against stored hashes.
#[async_trait]
pub trait PasswordService: Send + Sync {
    /// Hashes a plain password into a new [`StoredPasswordHash`].
    async fn hash_password(
        &self,
        password: SecretString,
    ) -> Result<StoredPasswordHash, PasswordError>;

    /// Verifies a plain password against a stored hash.
    ///
    /// When `expected_hash` is `None` the adapter must still run a dummy
    /// verification so unknown users are not distinguishable by timing.
    async fn verify_password(
        &self,
        password: SecretString,
        expected_hash: Option<&StoredPasswordHash>,
    ) -> Result<bool, PasswordError>;
}

/// Failure while hashing or verifying a password.
///
/// Internal only: these errors are never exposed to clients verbatim.
#[derive(Debug, Error)]
pub enum PasswordError {
    /// Hashing a password failed.
    #[error("password hashing failed")]
    HashingFailed,
    /// The stored password hash could not be parsed.
    #[error("stored password hash is malformed")]
    MalformedHash,
    /// Verifying a password failed.
    #[error("password verification failed")]
    VerificationFailed,
}

/// Generates opaque bearer tokens and fingerprints them.
pub trait SessionTokenService: Send + Sync {
    /// Generates a fresh random bearer token.
    fn generate_token(&self) -> Result<SecretString, TokenError>;

    /// Returns the deterministic fingerprint for a token.
    fn fingerprint(&self, token: &SecretString) -> Result<String, TokenError>;
}

/// Failure while generating or fingerprinting a session token.
///
/// Internal only: never exposed to clients verbatim.
#[derive(Debug, Error)]
pub enum TokenError {
    /// Random token generation failed.
    #[error("session token generation failed")]
    Generation,
    /// Token fingerprinting failed.
    #[error("session token fingerprinting failed")]
    Fingerprinting,
}

/// Repository for [`Product`] aggregates.
#[async_trait]
pub trait ProductRepository: Send + Sync {
    /// Finds a product by its identifier.
    async fn find_product_by_id(&self, id: &ProductId) -> Result<Option<Product>, RepositoryError>;

    /// Lists every known product regardless of availability.
    ///
    /// Availability filtering is an application rule owned by the use case,
    /// not by the adapter, so the same data can feed both the public catalog
    /// and the administrative views.
    async fn list_products(&self) -> Result<Vec<Product>, RepositoryError>;
}

/// Repository for [`Category`] aggregates.
#[async_trait]
pub trait CategoryRepository: Send + Sync {
    /// Finds a category by its identifier.
    async fn find_category_by_id(
        &self,
        id: &CategoryId,
    ) -> Result<Option<Category>, RepositoryError>;

    /// Lists every known category.
    async fn list_categories(&self) -> Result<Vec<Category>, RepositoryError>;
}

/// Repository for [`Order`] aggregates.
#[async_trait]
pub trait OrderRepository: Send + Sync {
    /// Persists an order (insert or update).
    async fn save_order(&self, order: &Order) -> Result<(), RepositoryError>;

    /// Finds an order by its identifier.
    async fn find_order_by_id(&self, id: &OrderId) -> Result<Option<Order>, RepositoryError>;

    /// Lists the orders placed by a student.
    async fn list_orders_for_student(
        &self,
        student_id: &UserId,
    ) -> Result<Vec<Order>, RepositoryError>;

    /// Lists the active orders visible to the cafeteria staff.
    async fn list_active_canteen_orders(&self) -> Result<Vec<Order>, RepositoryError>;
}

/// Provides the current time.
pub trait Clock: Send + Sync {
    /// Returns the current wall-clock time.
    fn now(&self) -> SystemTime;
}

/// Generates strongly typed domain identifiers.
pub trait IdGenerator: Send + Sync {
    /// Generates a [`UserId`].
    fn new_user_id(&self) -> UserId;

    /// Generates a [`ProductId`].
    fn new_product_id(&self) -> ProductId;

    /// Generates a [`CategoryId`].
    fn new_category_id(&self) -> CategoryId;

    /// Generates an [`OrderId`].
    fn new_order_id(&self) -> OrderId;
}

/// Generates short pickup codes for orders.
pub trait PickupCodeGenerator: Send + Sync {
    /// Generates a new pickup code.
    fn generate(&self) -> PickupCode;
}
