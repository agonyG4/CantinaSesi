//! Catalog use cases: reads plus administrative product/category mutations.
//!
//! Read use cases (`ListProducts`, `GetProduct`, `ListCategories`,
//! `GetCategory`) are public. Mutation use cases (`CreateProduct`,
//! `UpdateProduct`, `SetProductAvailability`, `CreateCategory`,
//! `UpdateCategory`) authorize the caller in the application layer before
//! touching any repository, so HTTP handlers never enforce RBAC alone.

use std::sync::Arc;

use cantina_core::auth::Principal;
use cantina_core::catalog::{Category, CategoryId, Product, ProductId};
use cantina_core::users::UserRole;

use crate::auth::AuthorizationService;
use crate::error::ApplicationError;
use crate::ports::{CategoryRepository, IdGenerator, ProductRepository};

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

/// Command for the `CreateProduct` use case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateProductCommand {
    pub name: String,
    pub description: String,
    /// Price in minor units.
    pub price: u64,
    pub category_id: CategoryId,
    pub available: bool,
}

/// Command for the `UpdateProduct` use case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateProductCommand {
    pub product_id: ProductId,
    pub name: Option<String>,
    pub description: Option<String>,
    /// Price in minor units.
    pub price: Option<u64>,
    pub category_id: Option<CategoryId>,
}

/// Command for the `SetProductAvailability` use case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetProductAvailabilityCommand {
    pub product_id: ProductId,
    pub available: bool,
}

/// Command for the `CreateCategory` use case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCategoryCommand {
    pub name: String,
}

/// Command for the `UpdateCategory` use case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateCategoryCommand {
    pub category_id: CategoryId,
    pub name: String,
}

/// Coordinates catalog use cases.
pub struct CatalogService {
    products: Arc<dyn ProductRepository>,
    categories: Arc<dyn CategoryRepository>,
    ids: Arc<dyn IdGenerator>,
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
    /// Creates the service with its repositories and identifier generator.
    pub fn new(
        products: Arc<dyn ProductRepository>,
        categories: Arc<dyn CategoryRepository>,
        ids: Arc<dyn IdGenerator>,
    ) -> Self {
        Self {
            products,
            categories,
            ids,
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

    /// `CreateProduct` use case.
    ///
    /// Authorizes a Canteen or Admin caller, confirms the referenced category
    /// exists, generates the product identifier server-side, validates the
    /// domain aggregate and persists it.
    pub async fn create_product(
        &self,
        principal: &Principal,
        command: CreateProductCommand,
    ) -> Result<ProductDto, ApplicationError> {
        AuthorizationService::require_any_role(principal, &[UserRole::Canteen, UserRole::Admin])?;

        self.categories
            .find_category_by_id(&command.category_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "category",
                id: command.category_id.to_string(),
            })?;

        let id = self.ids.new_product_id();
        let product = Product::new(
            id.clone(),
            command.name,
            command.description,
            command.price,
            command.category_id,
            command.available,
        )?;
        self.products.create_product(product).await?;
        let created = self
            .products
            .find_product_by_id(&id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "product",
                id: id.to_string(),
            })?;
        Ok(created.into())
    }

    /// `UpdateProduct` use case.
    ///
    /// Authorizes a Canteen or Admin caller, loads the product, preserves
    /// current values for absent fields, verifies a newly referenced category,
    /// applies validation atomically and persists the result.
    pub async fn update_product(
        &self,
        principal: &Principal,
        command: UpdateProductCommand,
    ) -> Result<ProductDto, ApplicationError> {
        AuthorizationService::require_any_role(principal, &[UserRole::Canteen, UserRole::Admin])?;

        if command.name.is_none()
            && command.description.is_none()
            && command.price.is_none()
            && command.category_id.is_none()
        {
            return Err(ApplicationError::EmptyUpdate);
        }

        let mut product = self
            .products
            .find_product_by_id(&command.product_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "product",
                id: command.product_id.to_string(),
            })?;

        let category_id = match &command.category_id {
            Some(category_id) => {
                self.categories
                    .find_category_by_id(category_id)
                    .await?
                    .ok_or_else(|| ApplicationError::NotFound {
                        resource: "category",
                        id: category_id.to_string(),
                    })?;
                category_id.clone()
            }
            None => product.category_id().clone(),
        };
        let name = command.name.unwrap_or_else(|| product.name().to_string());
        let description = command
            .description
            .unwrap_or_else(|| product.description().to_string());
        let price = command.price.unwrap_or_else(|| product.price());

        product.update_details(name, description, price, category_id)?;
        self.products.update_product(product).await?;
        let updated = self
            .products
            .find_product_by_id(&command.product_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "product",
                id: command.product_id.to_string(),
            })?;
        Ok(updated.into())
    }

    /// `SetProductAvailability` use case.
    ///
    /// Authorizes a Canteen or Admin caller, loads the product and changes
    /// only its availability. Enable/disable of an already
    /// available/unavailable product is idempotent.
    pub async fn set_product_availability(
        &self,
        principal: &Principal,
        command: SetProductAvailabilityCommand,
    ) -> Result<ProductDto, ApplicationError> {
        AuthorizationService::require_any_role(principal, &[UserRole::Canteen, UserRole::Admin])?;

        let mut product = self
            .products
            .find_product_by_id(&command.product_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "product",
                id: command.product_id.to_string(),
            })?;

        product.set_available(command.available);
        self.products.update_product(product).await?;
        let updated = self
            .products
            .find_product_by_id(&command.product_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "product",
                id: command.product_id.to_string(),
            })?;
        Ok(updated.into())
    }

    /// `CreateCategory` use case.
    ///
    /// Requires an Admin caller, generates the category identifier
    /// server-side, validates the name and persists it. The repository
    /// enforces case-insensitive normalized-name uniqueness.
    pub async fn create_category(
        &self,
        principal: &Principal,
        command: CreateCategoryCommand,
    ) -> Result<CategoryDto, ApplicationError> {
        AuthorizationService::require_any_role(principal, &[UserRole::Admin])?;

        let id = self.ids.new_category_id();
        let category = Category::new(id.clone(), command.name)?;
        self.categories.create_category(category).await?;
        let created = self
            .categories
            .find_category_by_id(&id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "category",
                id: id.to_string(),
            })?;
        Ok(created.into())
    }

    /// `UpdateCategory` use case.
    ///
    /// Requires an Admin caller, loads the category, validates the new name
    /// and persists it. The repository rejects a normalized-name conflict with
    /// another category while allowing the same category to keep its name.
    pub async fn update_category(
        &self,
        principal: &Principal,
        command: UpdateCategoryCommand,
    ) -> Result<CategoryDto, ApplicationError> {
        AuthorizationService::require_any_role(principal, &[UserRole::Admin])?;

        let mut category = self
            .categories
            .find_category_by_id(&command.category_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "category",
                id: command.category_id.to_string(),
            })?;

        category.rename(command.name)?;
        self.categories.update_category(category).await?;
        let updated = self
            .categories
            .find_category_by_id(&command.category_id)
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "category",
                id: command.category_id.to_string(),
            })?;
        Ok(updated.into())
    }
}
