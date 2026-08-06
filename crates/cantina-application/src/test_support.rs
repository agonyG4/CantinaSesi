//! In-memory repository adapters used only by the application tests.
//!
//! Kept inside this crate so the application tests never depend on
//! `cantina-infrastructure`, which would create a circular dependency.

#![cfg(test)]

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};

use cantina_core::catalog::{Category, CategoryId, Product, ProductId};
use cantina_core::orders::{Order, OrderId};
use cantina_core::users::{User, UserId};

use crate::ports::{
    CategoryRepository, Clock, CredentialRepository, OrderRepository, PasswordError,
    PasswordService, ProductRepository, RepositoryError, SessionRecord, SessionRepository,
    SessionRotationError, SessionTokenService, StoredPasswordHash, TokenError, UserRepository,
};

#[derive(Default)]
pub struct MemUserRepository {
    users: Mutex<HashMap<String, User>>,
}

impl MemUserRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, user: User) {
        self.users
            .lock()
            .unwrap()
            .insert(user.id().as_str().to_string(), user);
    }
}

#[async_trait]
impl UserRepository for MemUserRepository {
    async fn find_user_by_id(&self, id: &UserId) -> Result<Option<User>, RepositoryError> {
        Ok(self.users.lock().unwrap().get(id.as_str()).cloned())
    }

    async fn find_user_by_email(&self, email: &str) -> Result<Option<User>, RepositoryError> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .values()
            .find(|user| user.email() == email)
            .cloned())
    }
}

#[derive(Default)]
pub struct MemProductRepository {
    products: Mutex<HashMap<String, Product>>,
}

impl MemProductRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, product: Product) {
        self.products
            .lock()
            .unwrap()
            .insert(product.id().as_str().to_string(), product);
    }
}

#[async_trait]
impl ProductRepository for MemProductRepository {
    async fn find_product_by_id(&self, id: &ProductId) -> Result<Option<Product>, RepositoryError> {
        Ok(self.products.lock().unwrap().get(id.as_str()).cloned())
    }

    async fn list_products(&self) -> Result<Vec<Product>, RepositoryError> {
        Ok(self.products.lock().unwrap().values().cloned().collect())
    }
}

#[derive(Default)]
pub struct MemCategoryRepository {
    categories: Mutex<HashMap<String, Category>>,
}

impl MemCategoryRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, category: Category) {
        self.categories
            .lock()
            .unwrap()
            .insert(category.id().as_str().to_string(), category);
    }
}

#[async_trait]
impl CategoryRepository for MemCategoryRepository {
    async fn find_category_by_id(
        &self,
        id: &CategoryId,
    ) -> Result<Option<Category>, RepositoryError> {
        Ok(self.categories.lock().unwrap().get(id.as_str()).cloned())
    }

    async fn list_categories(&self) -> Result<Vec<Category>, RepositoryError> {
        Ok(self.categories.lock().unwrap().values().cloned().collect())
    }
}

#[derive(Default)]
pub struct MemOrderRepository {
    orders: Mutex<HashMap<String, Order>>,
}

impl MemOrderRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn order(&self, id: &OrderId) -> Option<Order> {
        self.orders.lock().unwrap().get(id.as_str()).cloned()
    }
}

#[async_trait]
impl OrderRepository for MemOrderRepository {
    async fn save_order(&self, order: &Order) -> Result<(), RepositoryError> {
        self.orders
            .lock()
            .unwrap()
            .insert(order.id().as_str().to_string(), order.clone());
        Ok(())
    }

    async fn find_order_by_id(&self, id: &OrderId) -> Result<Option<Order>, RepositoryError> {
        Ok(self.orders.lock().unwrap().get(id.as_str()).cloned())
    }

    async fn list_orders_for_student(
        &self,
        student_id: &UserId,
    ) -> Result<Vec<Order>, RepositoryError> {
        Ok(self
            .orders
            .lock()
            .unwrap()
            .values()
            .filter(|order| order.student_id() == student_id)
            .cloned()
            .collect())
    }

    async fn list_active_canteen_orders(&self) -> Result<Vec<Order>, RepositoryError> {
        Ok(self
            .orders
            .lock()
            .unwrap()
            .values()
            .filter(|order| order.status().is_active())
            .cloned()
            .collect())
    }
}

/// In-memory credential repository for application tests.
#[derive(Default)]
pub struct MemCredentialRepository {
    hashes: Mutex<HashMap<String, StoredPasswordHash>>,
}

impl MemCredentialRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, user_id: &UserId, hash: StoredPasswordHash) {
        self.hashes
            .lock()
            .unwrap()
            .insert(user_id.as_str().to_string(), hash);
    }
}

#[async_trait]
impl CredentialRepository for MemCredentialRepository {
    async fn find_password_hash_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<StoredPasswordHash>, RepositoryError> {
        Ok(self.hashes.lock().unwrap().get(user_id.as_str()).cloned())
    }
}

/// In-memory session repository for application tests.
#[derive(Default)]
pub struct MemSessionRepository {
    sessions: Mutex<HashMap<String, SessionRecord>>,
    fail_saves: AtomicBool,
    find_calls: AtomicU64,
}

impl MemSessionRepository {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn session(&self, fingerprint: &str) -> Option<SessionRecord> {
        self.sessions.lock().unwrap().get(fingerprint).cloned()
    }

    /// Number of `find_session_by_fingerprint` calls made.
    ///
    /// Used by tests to prove that refresh/logout authenticate a request only
    /// once: every authentication performs exactly one session lookup.
    pub fn find_calls(&self) -> u64 {
        self.find_calls.load(Ordering::SeqCst)
    }

    /// Makes every mutation fail, simulating a storage outage.
    pub fn fail_saves(&self) {
        self.fail_saves.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl SessionRepository for MemSessionRepository {
    async fn save_session(&self, session: SessionRecord) -> Result<(), RepositoryError> {
        if self.fail_saves.load(Ordering::SeqCst) {
            return Err(RepositoryError::new("injected storage failure"));
        }
        self.sessions
            .lock()
            .unwrap()
            .insert(session.token_fingerprint.clone(), session);
        Ok(())
    }

    async fn find_session_by_fingerprint(
        &self,
        fingerprint: &str,
    ) -> Result<Option<SessionRecord>, RepositoryError> {
        self.find_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self.sessions.lock().unwrap().get(fingerprint).cloned())
    }

    async fn update_last_seen_at(
        &self,
        fingerprint: &str,
        at: SystemTime,
    ) -> Result<(), RepositoryError> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(fingerprint) {
            session.last_seen_at = at;
        }
        Ok(())
    }

    async fn revoke_session(
        &self,
        fingerprint: &str,
        revoked_at: SystemTime,
    ) -> Result<(), RepositoryError> {
        let mut sessions = self.sessions.lock().unwrap();
        if let Some(session) = sessions.get_mut(fingerprint) {
            session.revoked_at = Some(revoked_at);
        }
        Ok(())
    }

    async fn rotate_session(
        &self,
        old_fingerprint: &str,
        new_session: SessionRecord,
        revoked_at: SystemTime,
    ) -> Result<(), SessionRotationError> {
        let mut sessions = self.sessions.lock().map_err(|_| {
            SessionRotationError::Internal("session store lock poisoned".to_string())
        })?;

        match sessions.get_mut(old_fingerprint) {
            None => return Err(SessionRotationError::OldSessionMissing),
            Some(old) if old.revoked_at.is_some() => {
                return Err(SessionRotationError::OldSessionRevoked);
            }
            Some(_) => {}
        }

        if sessions.contains_key(&new_session.token_fingerprint) {
            return Err(SessionRotationError::NewFingerprintCollision);
        }

        if let Some(old) = sessions.get_mut(old_fingerprint) {
            old.revoked_at = Some(revoked_at);
        }
        sessions.insert(new_session.token_fingerprint.clone(), new_session);
        Ok(())
    }
}

/// How the fake password service answers verification calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakePasswordMode {
    /// Every verification succeeds.
    Accept,
    /// Every verification fails.
    Reject,
    /// Every verification errors internally.
    Error,
}

/// Fake password service for application tests.
pub struct FakePasswordService {
    mode: Mutex<FakePasswordMode>,
    /// Records whether each verification received a stored hash.
    verify_calls: Mutex<Vec<bool>>,
    hash_calls: AtomicU64,
}

impl FakePasswordService {
    pub fn new(mode: FakePasswordMode) -> Self {
        Self {
            mode: Mutex::new(mode),
            verify_calls: Mutex::new(Vec::new()),
            hash_calls: AtomicU64::new(0),
        }
    }

    /// Records whether each verify call received `Some(hash)`.
    pub fn verify_calls(&self) -> Vec<bool> {
        self.verify_calls.lock().unwrap().clone()
    }

    /// Number of hashing calls made.
    pub fn hash_calls(&self) -> u64 {
        self.hash_calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl PasswordService for FakePasswordService {
    async fn hash_password(
        &self,
        _password: SecretString,
    ) -> Result<StoredPasswordHash, PasswordError> {
        self.hash_calls.fetch_add(1, Ordering::SeqCst);
        Ok(StoredPasswordHash::new(SecretString::from(
            "$argon2id$v=19$dummy".to_string(),
        )))
    }

    async fn verify_password(
        &self,
        _password: SecretString,
        expected_hash: Option<&StoredPasswordHash>,
    ) -> Result<bool, PasswordError> {
        self.verify_calls
            .lock()
            .unwrap()
            .push(expected_hash.is_some());
        match *self.mode.lock().unwrap() {
            FakePasswordMode::Accept => Ok(true),
            FakePasswordMode::Reject => Ok(false),
            FakePasswordMode::Error => Err(PasswordError::HashingFailed),
        }
    }
}

/// Fake token service producing deterministic, test-friendly tokens.
#[derive(Default)]
pub struct FakeTokenService {
    next: AtomicU64,
    fail_generation: AtomicBool,
    fail_fingerprint: AtomicBool,
}

impl FakeTokenService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Makes token generation fail.
    pub fn failing_generation(&self) {
        self.fail_generation.store(true, Ordering::SeqCst);
    }

    /// Makes fingerprinting fail.
    pub fn failing_fingerprint(&self) {
        self.fail_fingerprint.store(true, Ordering::SeqCst);
    }
}

impl SessionTokenService for FakeTokenService {
    fn generate_token(&self) -> Result<SecretString, TokenError> {
        if self.fail_generation.load(Ordering::SeqCst) {
            return Err(TokenError::Generation);
        }
        let sequence = self.next.fetch_add(1, Ordering::SeqCst);
        Ok(SecretString::from(format!("token-{sequence}")))
    }

    fn fingerprint(&self, token: &SecretString) -> Result<String, TokenError> {
        if self.fail_fingerprint.load(Ordering::SeqCst) {
            return Err(TokenError::Fingerprinting);
        }
        Ok(format!("fp-{}", token.expose_secret()))
    }
}

/// Manually advanced clock for time-dependent tests.
pub struct ManualClock {
    now: Mutex<SystemTime>,
}

impl ManualClock {
    pub fn new(at: SystemTime) -> Self {
        Self {
            now: Mutex::new(at),
        }
    }

    pub fn advance(&self, by: Duration) {
        let mut now = self.now.lock().unwrap();
        *now = now.checked_add(by).expect("manual clock advance");
    }
}

impl Clock for ManualClock {
    fn now(&self) -> SystemTime {
        *self.now.lock().unwrap()
    }
}
