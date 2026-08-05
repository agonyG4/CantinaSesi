//! Application-owned ports (interfaces owned by the application layer).
//!
//! The application decides exactly which operations it needs. The future
//! database implementation, and every other technical adapter, must adapt to
//! these interfaces — never the opposite. Ports are deliberately described
//! in domain terms (`find_user_by_email`, `save_order`) instead of SQL terms
//! (`execute_query`, `insert_record`).

use std::time::SystemTime;

use async_trait::async_trait;
use thiserror::Error;

use cantina_core::auth::Principal;
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

/// Verifies a plain password against a stored hash.
///
/// The real password hashing scheme is not implemented in the foundation.
pub trait PasswordVerifier: Send + Sync {
    /// Returns `true` when `password` matches `expected_hash`.
    fn verify(&self, password: &str, expected_hash: &str) -> bool;
}

/// Issues and validates authentication tokens.
///
/// The real token scheme (JWT or otherwise) is not implemented yet.
pub trait TokenIssuer: Send + Sync {
    /// Issues a token for the given principal.
    fn issue(&self, principal: &Principal) -> Result<String, TokenError>;
}

/// Failure while issuing an authentication token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum TokenError {
    /// Token issuance is not supported yet.
    #[error("token issuance is not implemented yet")]
    NotImplemented,
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
