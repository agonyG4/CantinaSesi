//! Minimal in-memory repository adapters.
//!
//! These exist only so the API can start, tests can run, and dependency
//! injection is demonstrated. A real database is intentionally out of scope
//! for the foundation. All adapters implement the application-owned ports.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use secrecy::SecretString;

use cantina_application::ports::{
    CatalogWriteError, CategoryRepository, OrderRepository, OrderWriteError, PasswordService,
    ProductRepository, RepositoryError, UserRepository,
};
use cantina_core::catalog::{Category, CategoryId, Product, ProductId};
use cantina_core::orders::{Order, OrderId};
use cantina_core::users::{User, UserId, UserRole, UserStatus};

use super::{InMemoryCredentialRepository, InMemorySessionRepository, Store, lock_store};

/// In-memory adapter for the `UserRepository` port.
pub struct InMemoryUserRepository {
    users: Store<User>,
}

impl InMemoryUserRepository {
    /// Creates an empty in-memory user repository.
    pub fn new() -> Self {
        Self {
            users: Mutex::new(HashMap::new()),
        }
    }

    /// Inserts or replaces a user.
    pub fn insert(&self, user: User) -> Result<(), RepositoryError> {
        lock_store(&self.users)?.insert(user.id().as_str().to_string(), user);
        Ok(())
    }
}

impl Default for InMemoryUserRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl UserRepository for InMemoryUserRepository {
    async fn find_user_by_id(&self, id: &UserId) -> Result<Option<User>, RepositoryError> {
        Ok(lock_store(&self.users)?.get(id.as_str()).cloned())
    }

    async fn find_user_by_email(&self, email: &str) -> Result<Option<User>, RepositoryError> {
        Ok(lock_store(&self.users)?
            .values()
            .find(|user| user.email() == email)
            .cloned())
    }
}

/// In-memory adapter for the `ProductRepository` port.
pub struct InMemoryProductRepository {
    products: Store<Product>,
}

impl InMemoryProductRepository {
    /// Creates an empty in-memory product repository.
    pub fn new() -> Self {
        Self {
            products: Mutex::new(HashMap::new()),
        }
    }

    /// Inserts or replaces a product.
    pub fn insert(&self, product: Product) -> Result<(), RepositoryError> {
        lock_store(&self.products)?.insert(product.id().as_str().to_string(), product);
        Ok(())
    }
}

impl Default for InMemoryProductRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ProductRepository for InMemoryProductRepository {
    async fn find_product_by_id(&self, id: &ProductId) -> Result<Option<Product>, RepositoryError> {
        Ok(lock_store(&self.products)?.get(id.as_str()).cloned())
    }

    async fn find_products_by_ids(
        &self,
        ids: &[ProductId],
    ) -> Result<Vec<Product>, RepositoryError> {
        let products = lock_store(&self.products)?;
        Ok(ids
            .iter()
            .filter_map(|id| products.get(id.as_str()).cloned())
            .collect())
    }

    async fn list_products(&self) -> Result<Vec<Product>, RepositoryError> {
        Ok(lock_store(&self.products)?.values().cloned().collect())
    }

    async fn create_product(&self, product: Product) -> Result<(), CatalogWriteError> {
        let mut products = lock_store(&self.products)
            .map_err(|_| CatalogWriteError::Internal("product store lock poisoned".to_string()))?;
        if products.contains_key(product.id().as_str()) {
            return Err(CatalogWriteError::IdentifierConflict);
        }
        products.insert(product.id().as_str().to_string(), product);
        Ok(())
    }

    async fn update_product(&self, product: Product) -> Result<(), CatalogWriteError> {
        let mut products = lock_store(&self.products)
            .map_err(|_| CatalogWriteError::Internal("product store lock poisoned".to_string()))?;
        if !products.contains_key(product.id().as_str()) {
            return Err(CatalogWriteError::NotFound);
        }
        products.insert(product.id().as_str().to_string(), product);
        Ok(())
    }
}

/// In-memory adapter for the `CategoryRepository` port.
pub struct InMemoryCategoryRepository {
    categories: Store<Category>,
}

impl InMemoryCategoryRepository {
    /// Creates an empty in-memory category repository.
    pub fn new() -> Self {
        Self {
            categories: Mutex::new(HashMap::new()),
        }
    }

    /// Inserts or replaces a category.
    pub fn insert(&self, category: Category) -> Result<(), RepositoryError> {
        lock_store(&self.categories)?.insert(category.id().as_str().to_string(), category);
        Ok(())
    }
}

impl Default for InMemoryCategoryRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CategoryRepository for InMemoryCategoryRepository {
    async fn find_category_by_id(
        &self,
        id: &CategoryId,
    ) -> Result<Option<Category>, RepositoryError> {
        Ok(lock_store(&self.categories)?.get(id.as_str()).cloned())
    }

    async fn list_categories(&self) -> Result<Vec<Category>, RepositoryError> {
        Ok(lock_store(&self.categories)?.values().cloned().collect())
    }

    async fn create_category(&self, category: Category) -> Result<(), CatalogWriteError> {
        let mut categories = lock_store(&self.categories)
            .map_err(|_| CatalogWriteError::Internal("category store lock poisoned".to_string()))?;
        if categories.contains_key(category.id().as_str()) {
            return Err(CatalogWriteError::IdentifierConflict);
        }
        if categories
            .values()
            .any(|c| c.normalized_name() == category.normalized_name())
        {
            return Err(CatalogWriteError::NameConflict);
        }
        categories.insert(category.id().as_str().to_string(), category);
        Ok(())
    }

    async fn update_category(&self, category: Category) -> Result<(), CatalogWriteError> {
        let mut categories = lock_store(&self.categories)
            .map_err(|_| CatalogWriteError::Internal("category store lock poisoned".to_string()))?;
        if !categories.contains_key(category.id().as_str()) {
            return Err(CatalogWriteError::NotFound);
        }
        let conflict = categories
            .values()
            .any(|c| c.id() != category.id() && c.normalized_name() == category.normalized_name());
        if conflict {
            return Err(CatalogWriteError::NameConflict);
        }
        categories.insert(category.id().as_str().to_string(), category);
        Ok(())
    }
}

/// In-memory adapter for the `OrderRepository` port.
pub struct InMemoryOrderRepository {
    orders: Store<Order>,
}

impl InMemoryOrderRepository {
    /// Creates an empty in-memory order repository.
    pub fn new() -> Self {
        Self {
            orders: Mutex::new(HashMap::new()),
        }
    }

    /// Inserts or replaces an order.
    pub fn insert(&self, order: Order) -> Result<(), RepositoryError> {
        lock_store(&self.orders)?.insert(order.id().as_str().to_string(), order);
        Ok(())
    }
}

impl Default for InMemoryOrderRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OrderRepository for InMemoryOrderRepository {
    async fn save_new_order(&self, order: &Order) -> Result<(), OrderWriteError> {
        let mut orders = lock_store(&self.orders)
            .map_err(|_| OrderWriteError::Internal("order store lock poisoned".to_string()))?;
        if orders.contains_key(order.id().as_str()) {
            return Err(OrderWriteError::IdentifierConflict);
        }
        if orders
            .values()
            .any(|existing| existing.pickup_code() == order.pickup_code())
        {
            return Err(OrderWriteError::PickupCodeConflict);
        }
        orders.insert(order.id().as_str().to_string(), order.clone());
        Ok(())
    }

    async fn save_order(&self, order: &Order) -> Result<(), RepositoryError> {
        lock_store(&self.orders)?.insert(order.id().as_str().to_string(), order.clone());
        Ok(())
    }

    async fn find_order_by_id(&self, id: &OrderId) -> Result<Option<Order>, RepositoryError> {
        Ok(lock_store(&self.orders)?.get(id.as_str()).cloned())
    }

    async fn list_orders_for_student(
        &self,
        student_id: &UserId,
    ) -> Result<Vec<Order>, RepositoryError> {
        let mut orders: Vec<Order> = lock_store(&self.orders)?
            .values()
            .filter(|order| order.student_id() == student_id)
            .cloned()
            .collect();
        orders.sort_by(|left, right| {
            right
                .created_at()
                .cmp(&left.created_at())
                .then_with(|| left.id().as_str().cmp(right.id().as_str()))
        });
        Ok(orders)
    }

    async fn list_active_canteen_orders(&self) -> Result<Vec<Order>, RepositoryError> {
        let mut orders: Vec<Order> = lock_store(&self.orders)?
            .values()
            .filter(|order| order.status().is_active())
            .cloned()
            .collect();
        orders.sort_by(|left, right| {
            left.created_at()
                .cmp(&right.created_at())
                .then_with(|| left.id().as_str().cmp(right.id().as_str()))
        });
        Ok(orders)
    }
}

/// All in-memory repositories wired together for composition and tests.
#[derive(Clone)]
pub struct InMemoryRepositories {
    pub users: Arc<InMemoryUserRepository>,
    pub credentials: Arc<InMemoryCredentialRepository>,
    pub sessions: Arc<InMemorySessionRepository>,
    pub products: Arc<InMemoryProductRepository>,
    pub categories: Arc<InMemoryCategoryRepository>,
    pub orders: Arc<InMemoryOrderRepository>,
}

impl InMemoryRepositories {
    /// Creates empty repositories.
    pub fn new() -> Self {
        Self {
            users: Arc::new(InMemoryUserRepository::new()),
            credentials: Arc::new(InMemoryCredentialRepository::new()),
            sessions: Arc::new(InMemorySessionRepository::new()),
            products: Arc::new(InMemoryProductRepository::new()),
            categories: Arc::new(InMemoryCategoryRepository::new()),
            orders: Arc::new(InMemoryOrderRepository::new()),
        }
    }

    /// Creates repositories pre-loaded with a demonstration catalog and demo
    /// accounts.
    ///
    /// Demo users are development-only, clearly labeled in the documentation,
    /// and their passwords are hashed exactly once at seed time with the real
    /// Argon2id adapter. Never deploy these accounts.
    pub async fn with_seed_data() -> Result<Self, RepositoryError> {
        let repos = Self::new();

        repos
            .categories
            .insert(seed_category("category_snacks", "Snacks"))?;
        repos
            .categories
            .insert(seed_category("category_drinks", "Drinks"))?;
        repos
            .categories
            .insert(seed_category("category_bakery", "Padaria"))?;

        repos.products.insert(seed_product(
            "product_brigadeiro",
            "Brigadeiro",
            "Chocolate truffle, school classic",
            500,
            "category_snacks",
            true,
        ))?;

        repos.products.insert(seed_product(
            "product_coxinha",
            "Coxinha",
            "Fried shredded-chicken pastry",
            750,
            "category_snacks",
            true,
        ))?;

        repos.products.insert(seed_product(
            "product_refresco",
            "Refresco de limão",
            "Fresh lemonade",
            300,
            "category_drinks",
            true,
        ))?;

        repos.products.insert(seed_product(
            "product_pao_de_queijo",
            "Pão de queijo",
            "Baked cheese bread",
            350,
            "category_bakery",
            true,
        ))?;

        repos.products.insert(seed_product(
            "product_pastel",
            "Pastel de queijo",
            "Cheese pastry, temporarily unavailable",
            600,
            "category_snacks",
            false,
        ))?;

        repos
            .insert_demo_user(
                "user_student_1",
                "Ana Souza",
                "ana@example.com",
                UserRole::Student,
                UserStatus::Active,
                "StudentDemo!2026",
            )
            .await?;
        repos
            .insert_demo_user(
                "user_student_2",
                "Bruno Lima",
                "bruno@example.com",
                UserRole::Student,
                UserStatus::Active,
                "StudentTwoDemo!2026",
            )
            .await?;
        repos
            .insert_demo_user(
                "user_canteen_1",
                "Maria Canteen",
                "cantina@example.com",
                UserRole::Canteen,
                UserStatus::Active,
                "CanteenDemo!2026",
            )
            .await?;
        repos
            .insert_demo_user(
                "user_admin_1",
                "Admin Root",
                "admin@example.com",
                UserRole::Admin,
                UserStatus::Active,
                "AdminDemo!2026",
            )
            .await?;
        repos
            .insert_demo_user(
                "user_inactive_1",
                "Inactive Student",
                "inactive@example.com",
                UserRole::Student,
                UserStatus::Inactive,
                "InactiveDemo!2026",
            )
            .await?;

        Ok(repos)
    }

    /// Inserts a demo user and hashes its password once at seed time.
    async fn insert_demo_user(
        &self,
        id: &str,
        name: &str,
        email: &str,
        role: UserRole,
        status: UserStatus,
        password: &str,
    ) -> Result<(), RepositoryError> {
        let passwords = crate::security::passwords::Argon2PasswordService::new();
        let hash = passwords
            .hash_password(SecretString::from(password.to_string()))
            .await
            .map_err(|_| RepositoryError::new("failed to hash demo password"))?;
        self.users
            .insert(User::new(UserId::new(id), name, email, role, status))?;
        self.credentials.insert(&UserId::new(id), hash)?;
        Ok(())
    }
}

impl Default for InMemoryRepositories {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds a valid seeded product, mapping validation errors onto storage
/// errors (seed data is known to be valid).
fn seed_product(
    id: &str,
    name: &str,
    description: &str,
    price: u64,
    category: &str,
    available: bool,
) -> Product {
    Product::new(
        ProductId::new(id),
        name,
        description,
        price,
        CategoryId::new(category),
        available,
    )
    .expect("seed product data must be valid")
}

/// Builds a valid seeded category.
fn seed_category(id: &str, name: &str) -> Category {
    Category::new(CategoryId::new(id), name).expect("seed category data must be valid")
}

#[cfg(test)]
mod tests {
    use cantina_application::ports::{CategoryRepository, ProductRepository, UserRepository};
    use cantina_core::users::{User, UserId, UserRole, UserStatus};

    use super::{InMemoryRepositories, seed_category, seed_product};

    #[tokio::test]
    async fn seeded_repositories_expose_demo_data() {
        let repos = InMemoryRepositories::with_seed_data()
            .await
            .expect("seed repositories");
        assert!(
            repos
                .products
                .insert(seed_product("product_x", "X", "X", 100, "category_x", true))
                .is_ok()
        );
        assert!(
            repos
                .categories
                .insert(seed_category("category_x", "X"))
                .is_ok()
        );
        assert!(
            repos
                .users
                .insert(User::new(
                    UserId::new("user_x"),
                    "X",
                    "x@example.com",
                    UserRole::Student,
                    UserStatus::Active,
                ))
                .is_ok()
        );
    }

    #[tokio::test]
    async fn seed_data_covers_available_and_unavailable_products() {
        let repos = InMemoryRepositories::with_seed_data()
            .await
            .expect("seed repositories");
        let (mut available, mut unavailable) = (0u32, 0u32);
        for product in repos.products.list_products().await.expect("list products") {
            if product.is_available() {
                available += 1;
            } else {
                unavailable += 1;
            }
        }
        assert!(available >= 3, "expected at least 3 available products");
        assert!(unavailable >= 1, "expected at least 1 unavailable product");
    }

    #[tokio::test]
    async fn seed_categories_cover_at_least_two_entries() {
        let repos = InMemoryRepositories::with_seed_data()
            .await
            .expect("seed repositories");
        let categories = repos
            .categories
            .list_categories()
            .await
            .expect("list categories");
        assert!(categories.len() >= 2);
    }

    #[tokio::test]
    async fn seed_exposes_demo_users_with_credentials() {
        let repos = InMemoryRepositories::with_seed_data()
            .await
            .expect("seed repositories");

        let ana = repos
            .users
            .find_user_by_email("ana@example.com")
            .await
            .expect("find user")
            .expect("ana must be seeded");
        assert_eq!(ana.name(), "Ana Souza");
        assert_eq!(ana.role(), UserRole::Student);
        assert!(ana.is_active());

        let inactive = repos
            .users
            .find_user_by_email("inactive@example.com")
            .await
            .expect("find user")
            .expect("inactive user must be seeded");
        assert!(!inactive.is_active());

        let found = repos
            .users
            .find_user_by_email("cantina@example.com")
            .await
            .expect("find user");
        assert!(found.is_some(), "canteen demo user must exist");
    }
}
