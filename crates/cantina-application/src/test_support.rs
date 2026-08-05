//! In-memory repository adapters used only by the application tests.
//!
//! Kept inside this crate so the application tests never depend on
//! `cantina-infrastructure`, which would create a circular dependency.

#![cfg(test)]

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use cantina_core::catalog::{Category, CategoryId, Product, ProductId};
use cantina_core::orders::{Order, OrderId};
use cantina_core::users::{User, UserId};

use crate::ports::{
    CategoryRepository, OrderRepository, ProductRepository, RepositoryError, UserRepository,
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
