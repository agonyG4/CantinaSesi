//! Catalog use cases: `ListProducts`, `GetProduct`.

use std::sync::Arc;

use cantina_core::catalog::{Category, CategoryId, Product, ProductId};

use crate::error::ApplicationError;
use crate::ports::{CategoryRepository, ProductRepository};

/// Application representation of a [`Product`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDto {
    pub id: ProductId,
    pub name: String,
    pub description: String,
    /// Price in minor units.
    pub price: u64,
    pub category_id: CategoryId,
    pub available: bool,
}

impl From<Product> for ProductDto {
    fn from(product: Product) -> Self {
        Self {
            id: product.id().clone(),
            name: product.name().to_string(),
            description: product.description().to_string(),
            price: product.price(),
            category_id: product.category_id().clone(),
            available: product.is_available(),
        }
    }
}

/// Application representation of a [`Category`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryDto {
    pub id: CategoryId,
    pub name: String,
}

impl From<Category> for CategoryDto {
    fn from(category: Category) -> Self {
        Self {
            id: category.id().clone(),
            name: category.name().to_string(),
        }
    }
}

/// Coordinates catalog use cases.
pub struct CatalogService {
    products: Arc<dyn ProductRepository>,
    categories: Arc<dyn CategoryRepository>,
}

impl CatalogService {
    /// Creates the service with its repositories.
    pub fn new(
        products: Arc<dyn ProductRepository>,
        categories: Arc<dyn CategoryRepository>,
    ) -> Self {
        Self {
            products,
            categories,
        }
    }

    /// `ListProducts` use case: lists the currently available products.
    pub async fn list_products(&self) -> Result<Vec<ProductDto>, ApplicationError> {
        let products = self.products.list_available_products().await?;
        Ok(products.into_iter().map(Into::into).collect())
    }

    /// `GetProduct` use case.
    pub async fn get_product(&self, id: ProductId) -> Result<ProductDto, ApplicationError> {
        let product = self
            .products
            .find_product_by_id(&id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "product",
                id: id.to_string(),
            })?;
        Ok(product.into())
    }

    /// Resolves a category by identifier.
    pub async fn get_category(&self, id: CategoryId) -> Result<CategoryDto, ApplicationError> {
        let category = self
            .categories
            .find_category_by_id(&id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "category",
                id: id.to_string(),
            })?;
        Ok(category.into())
    }
}
