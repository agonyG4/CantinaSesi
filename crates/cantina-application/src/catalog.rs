//! Catalog use cases: `ListProducts`, `GetProduct`, `ListCategories`,
//! `GetCategory`.

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

/// Query filters for the `ListProducts` use case.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListProductsQuery {
    /// Restricts the results to a single category.
    pub category_id: Option<CategoryId>,
    /// Case-insensitive substring match against the name or description.
    pub search: Option<String>,
    /// When `false` (default) only currently available products are listed.
    pub include_unavailable: bool,
}

impl ListProductsQuery {
    /// A query without any filter, listing only available products.
    pub fn default_available() -> Self {
        Self::default()
    }
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

    /// `ListProducts` use case.
    ///
    /// Applies availability, category and text filters, then orders the
    /// results deterministically by name and identifier.
    pub async fn list_products(
        &self,
        query: &ListProductsQuery,
    ) -> Result<Vec<ProductDto>, ApplicationError> {
        let mut products: Vec<ProductDto> = self
            .products
            .list_products()
            .await?
            .into_iter()
            .filter(|product| self.matches_query(product, query))
            .map(Into::into)
            .collect();

        products.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.as_str().cmp(b.id.as_str()))
        });
        Ok(products)
    }

    /// `ListCategories` use case.
    ///
    /// Orders the categories deterministically by name and identifier.
    pub async fn list_categories(&self) -> Result<Vec<CategoryDto>, ApplicationError> {
        let mut categories: Vec<CategoryDto> = self
            .categories
            .list_categories()
            .await?
            .into_iter()
            .map(Into::into)
            .collect();

        categories.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.id.as_str().cmp(b.id.as_str()))
        });
        Ok(categories)
    }

    fn matches_query(&self, product: &Product, query: &ListProductsQuery) -> bool {
        if !query.include_unavailable && !product.is_available() {
            return false;
        }
        if let Some(category_id) = &query.category_id
            && product.category_id() != category_id
        {
            return false;
        }
        match query
            .search
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            Some(term) => {
                let term = term.to_lowercase();
                product.name().to_lowercase().contains(&term)
                    || product.description().to_lowercase().contains(&term)
            }
            None => true,
        }
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
