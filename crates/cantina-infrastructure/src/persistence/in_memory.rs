//! Minimal in-memory repository adapters.
//!
//! These exist only so the API can start, tests can run, and dependency
//! injection is demonstrated. A real database is intentionally out of scope
//! for the foundation. All adapters implement the application-owned ports.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;

use cantina_application::ports::{
    CategoryRepository, OrderRepository, ProductRepository, RepositoryError, UserRepository,
};
use cantina_core::catalog::{Category, CategoryId, Product, ProductId};
use cantina_core::orders::{Order, OrderId};
use cantina_core::users::{User, UserId, UserRole, UserStatus};

type Store<T> = Mutex<HashMap<String, T>>;

fn lock_store<T>(store: &Store<T>) -> Result<MutexGuard<'_, HashMap<String, T>>, RepositoryError> {
    store
        .lock()
        .map_err(|_| RepositoryError::new("repository lock poisoned"))
}

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

    async fn list_available_products(&self) -> Result<Vec<Product>, RepositoryError> {
        Ok(lock_store(&self.products)?
            .values()
            .filter(|product| product.is_available())
            .cloned()
            .collect())
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
        Ok(lock_store(&self.orders)?
            .values()
            .filter(|order| order.student_id() == student_id)
            .cloned()
            .collect())
    }

    async fn list_active_canteen_orders(&self) -> Result<Vec<Order>, RepositoryError> {
        Ok(lock_store(&self.orders)?
            .values()
            .filter(|order| order.status().is_active())
            .cloned()
            .collect())
    }
}

/// All in-memory repositories wired together for composition and tests.
#[derive(Clone)]
pub struct InMemoryRepositories {
    pub users: Arc<InMemoryUserRepository>,
    pub products: Arc<InMemoryProductRepository>,
    pub categories: Arc<InMemoryCategoryRepository>,
    pub orders: Arc<InMemoryOrderRepository>,
}

impl InMemoryRepositories {
    /// Creates empty repositories.
    pub fn new() -> Self {
        Self {
            users: Arc::new(InMemoryUserRepository::new()),
            products: Arc::new(InMemoryProductRepository::new()),
            categories: Arc::new(InMemoryCategoryRepository::new()),
            orders: Arc::new(InMemoryOrderRepository::new()),
        }
    }

    /// Creates repositories pre-loaded with a small demonstration catalog.
    pub fn with_seed_data() -> Self {
        let repos = Self::new();

        repos
            .categories
            .insert(Category::new(CategoryId::new("category_snacks"), "Snacks"))
            .expect("seed category");

        repos
            .products
            .insert(Product::new(
                ProductId::new("product_brigadeiro"),
                "Brigadeiro",
                "Chocolate truffle, school classic",
                500,
                CategoryId::new("category_snacks"),
                true,
            ))
            .expect("seed product");

        repos
            .products
            .insert(Product::new(
                ProductId::new("product_coxinha"),
                "Coxinha",
                "Fried shredded-chicken pastry",
                750,
                CategoryId::new("category_snacks"),
                true,
            ))
            .expect("seed product");

        repos
            .users
            .insert(User::new(
                UserId::new("user_student_1"),
                "Ana Souza",
                "ana@example.com",
                UserRole::Student,
                UserStatus::Active,
            ))
            .expect("seed user");

        repos
    }
}

impl Default for InMemoryRepositories {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use cantina_core::catalog::{Category, CategoryId, Product, ProductId};
    use cantina_core::users::{User, UserId, UserRole, UserStatus};

    use super::InMemoryRepositories;

    #[test]
    fn seeded_repositories_expose_demo_data() {
        let repos = InMemoryRepositories::with_seed_data();
        assert!(
            repos
                .products
                .insert(Product::new(
                    ProductId::new("product_x"),
                    "X",
                    "X",
                    100,
                    CategoryId::new("category_x"),
                    true,
                ))
                .is_ok()
        );
        assert!(
            repos
                .categories
                .insert(Category::new(CategoryId::new("category_x"), "X"))
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
}
