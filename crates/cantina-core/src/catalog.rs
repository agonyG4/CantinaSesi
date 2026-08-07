//! Product catalog concepts.

use std::fmt;

/// Maximum allowed length (in Unicode characters) of a product name.
pub const MAX_PRODUCT_NAME_CHARS: usize = 120;
/// Maximum allowed length (in Unicode characters) of a product description.
pub const MAX_PRODUCT_DESCRIPTION_CHARS: usize = 500;
/// Maximum allowed length (in Unicode characters) of a category name.
pub const MAX_CATEGORY_NAME_CHARS: usize = 80;

/// Typed validation errors for catalog aggregates.
///
/// Framework-independent and dependency-free: `Display` is hand-written and
/// the crate does not pull in `thiserror`. The application layer translates
/// these into stable public HTTP codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogError {
    /// The product name is required.
    EmptyProductName,
    /// The product name exceeds [`MAX_PRODUCT_NAME_CHARS`].
    ProductNameTooLong,
    /// The product description exceeds [`MAX_PRODUCT_DESCRIPTION_CHARS`].
    ProductDescriptionTooLong,
    /// The product price must be greater than zero.
    InvalidProductPrice,
    /// The category name is required.
    EmptyCategoryName,
    /// The category name exceeds [`MAX_CATEGORY_NAME_CHARS`].
    CategoryNameTooLong,
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProductName => write!(f, "a product name is required"),
            Self::ProductNameTooLong => write!(
                f,
                "the product name must not exceed {MAX_PRODUCT_NAME_CHARS} characters"
            ),
            Self::ProductDescriptionTooLong => write!(
                f,
                "the product description must not exceed {MAX_PRODUCT_DESCRIPTION_CHARS} characters"
            ),
            Self::InvalidProductPrice => write!(f, "the product price must be greater than zero"),
            Self::EmptyCategoryName => write!(f, "a category name is required"),
            Self::CategoryNameTooLong => write!(
                f,
                "the category name must not exceed {MAX_CATEGORY_NAME_CHARS} characters"
            ),
        }
    }
}

impl std::error::Error for CatalogError {}

/// Counts the Unicode characters in a string (not UTF-8 bytes).
fn char_len(value: &str) -> usize {
    value.chars().count()
}

/// Strongly typed identifier for a [`Product`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProductId(String);

impl ProductId {
    /// Creates a product identifier from a raw string value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the underlying identifier value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProductId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Strongly typed identifier for a [`Category`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CategoryId(String);

impl CategoryId {
    /// Creates a category identifier from a raw string value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the underlying identifier value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CategoryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A product category (for example "Snacks" or "Drinks").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    id: CategoryId,
    name: String,
}

impl Category {
    /// Validates and creates a category.
    ///
    /// The name is trimmed of surrounding whitespace before validation and
    /// storage. A blank name or one longer than [`MAX_CATEGORY_NAME_CHARS`]
    /// is rejected.
    pub fn new(id: CategoryId, name: impl Into<String>) -> Result<Self, CatalogError> {
        let name = name.into().trim().to_string();
        validate_category_name(&name)?;
        Ok(Self { id, name })
    }

    /// Renames the category, returning an error without mutating on failure.
    ///
    /// The new name is trimmed before validation; a failed rename leaves the
    /// category unchanged.
    pub fn rename(&mut self, name: impl Into<String>) -> Result<(), CatalogError> {
        let name = name.into().trim().to_string();
        validate_category_name(&name)?;
        self.name = name;
        Ok(())
    }

    /// Returns the category identifier.
    pub fn id(&self) -> &CategoryId {
        &self.id
    }

    /// Returns the category name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a normalized key used for case-insensitive, trim-aware
    /// uniqueness comparisons.
    pub fn normalized_name(&self) -> String {
        self.name.trim().to_lowercase()
    }
}

/// Validates a trimmed category name.
fn validate_category_name(name: &str) -> Result<(), CatalogError> {
    if name.is_empty() {
        return Err(CatalogError::EmptyCategoryName);
    }
    if char_len(name) > MAX_CATEGORY_NAME_CHARS {
        return Err(CatalogError::CategoryNameTooLong);
    }
    Ok(())
}

/// A product that can be ordered from the cafeteria.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Product {
    id: ProductId,
    name: String,
    description: String,
    /// Price in minor units (centavos). Stored as an integer to avoid
    /// floating point rounding issues; see `docs/decisions/0001-foundation-decisions.md`.
    price: u64,
    category_id: CategoryId,
    available: bool,
}

impl Product {
    /// Validates and creates a product.
    ///
    /// The name and description are trimmed of surrounding whitespace before
    /// validation and storage. A blank or overlong name, an overlong
    /// description or a zero price is rejected.
    pub fn new(
        id: ProductId,
        name: impl Into<String>,
        description: impl Into<String>,
        price: u64,
        category_id: CategoryId,
        available: bool,
    ) -> Result<Self, CatalogError> {
        let name = name.into().trim().to_string();
        let description = description.into().trim().to_string();
        validate_product(name.as_str(), description.as_str(), price)?;
        Ok(Self {
            id,
            name,
            description,
            price,
            category_id,
            available,
        })
    }

    /// Validates and updates the mutable product details.
    ///
    /// All inputs are validated before any field is mutated, so a failed
    /// update leaves the product completely unchanged (atomic update). The
    /// identifier is never modified.
    pub fn update_details(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        price: u64,
        category_id: CategoryId,
    ) -> Result<(), CatalogError> {
        let name = name.into().trim().to_string();
        let description = description.into().trim().to_string();
        validate_product(&name, &description, price)?;
        self.name = name;
        self.description = description;
        self.price = price;
        self.category_id = category_id;
        Ok(())
    }

    /// Changes only the availability of the product.
    ///
    /// No other field is modified.
    pub fn set_available(&mut self, available: bool) {
        self.available = available;
    }

    /// Returns the product identifier.
    pub fn id(&self) -> &ProductId {
        &self.id
    }

    /// Returns the product name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the product description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Returns the price in minor units.
    pub fn price(&self) -> u64 {
        self.price
    }

    /// Returns the category the product belongs to.
    pub fn category_id(&self) -> &CategoryId {
        &self.category_id
    }

    /// Returns `true` when the product can currently be ordered.
    pub fn is_available(&self) -> bool {
        self.available
    }
}

/// Validates the mutable product fields.
fn validate_product(name: &str, description: &str, price: u64) -> Result<(), CatalogError> {
    if name.is_empty() {
        return Err(CatalogError::EmptyProductName);
    }
    if char_len(name) > MAX_PRODUCT_NAME_CHARS {
        return Err(CatalogError::ProductNameTooLong);
    }
    if char_len(description) > MAX_PRODUCT_DESCRIPTION_CHARS {
        return Err(CatalogError::ProductDescriptionTooLong);
    }
    if price == 0 {
        return Err(CatalogError::InvalidProductPrice);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CatalogError, Category, CategoryId, Product, ProductId};

    #[test]
    fn valid_product_construction() {
        let product = Product::new(
            ProductId::new("p1"),
            "Coxinha",
            "Fried chicken pastry",
            500,
            CategoryId::new("c1"),
            true,
        )
        .expect("a valid product must be constructed");
        assert!(product.is_available());
        assert_eq!(product.price(), 500);
        assert_eq!(product.id().as_str(), "p1");
        assert_eq!(product.category_id().as_str(), "c1");
    }

    #[test]
    fn product_name_is_trimmed() {
        let product = Product::new(
            ProductId::new("p1"),
            "  Coxinha  ",
            "Fried chicken pastry",
            500,
            CategoryId::new("c1"),
            true,
        )
        .expect("a valid product must be constructed");
        assert_eq!(product.name(), "Coxinha");
    }

    #[test]
    fn empty_product_name_is_rejected() {
        let err = Product::new(
            ProductId::new("p1"),
            "   ",
            "x",
            500,
            CategoryId::new("c1"),
            true,
        )
        .expect_err("a blank product name must be rejected");
        assert_eq!(err, CatalogError::EmptyProductName);
    }

    #[test]
    fn overlong_product_name_is_rejected() {
        let name = "a".repeat(super::MAX_PRODUCT_NAME_CHARS + 1);
        let err = Product::new(
            ProductId::new("p1"),
            name,
            "x",
            500,
            CategoryId::new("c1"),
            true,
        )
        .expect_err("an overlong product name must be rejected");
        assert_eq!(err, CatalogError::ProductNameTooLong);
    }

    #[test]
    fn description_is_trimmed() {
        let product = Product::new(
            ProductId::new("p1"),
            "Coxinha",
            "  Fried chicken pastry  ",
            500,
            CategoryId::new("c1"),
            true,
        )
        .expect("a valid product must be constructed");
        assert_eq!(product.description(), "Fried chicken pastry");
    }

    #[test]
    fn overlong_description_is_rejected() {
        let description = "a".repeat(super::MAX_PRODUCT_DESCRIPTION_CHARS + 1);
        let err = Product::new(
            ProductId::new("p1"),
            "Coxinha",
            description,
            500,
            CategoryId::new("c1"),
            true,
        )
        .expect_err("an overlong description must be rejected");
        assert_eq!(err, CatalogError::ProductDescriptionTooLong);
    }

    #[test]
    fn zero_price_is_rejected() {
        let err = Product::new(
            ProductId::new("p1"),
            "Coxinha",
            "x",
            0,
            CategoryId::new("c1"),
            true,
        )
        .expect_err("a zero price must be rejected");
        assert_eq!(err, CatalogError::InvalidProductPrice);
    }

    #[test]
    fn update_details_is_atomic() {
        let mut product = Product::new(
            ProductId::new("p1"),
            "Coxinha",
            "Fried chicken pastry",
            500,
            CategoryId::new("c1"),
            true,
        )
        .expect("construct");
        product
            .update_details("Pastel", "Cheese pastry", 600, CategoryId::new("c2"))
            .expect("a valid update must succeed");
        assert_eq!(product.name(), "Pastel");
        assert_eq!(product.description(), "Cheese pastry");
        assert_eq!(product.price(), 600);
        assert_eq!(product.category_id().as_str(), "c2");
        assert_eq!(
            product.id().as_str(),
            "p1",
            "the identifier must not change"
        );
        assert!(product.is_available(), "availability must not change");
    }

    #[test]
    fn failed_update_leaves_original_values_unchanged() {
        let mut product = Product::new(
            ProductId::new("p1"),
            "Coxinha",
            "Fried chicken pastry",
            500,
            CategoryId::new("c1"),
            true,
        )
        .expect("construct");
        let err = product
            .update_details("  ", "New description", 600, CategoryId::new("c2"))
            .expect_err("a blank name must be rejected");
        assert_eq!(err, CatalogError::EmptyProductName);
        assert_eq!(product.name(), "Coxinha");
        assert_eq!(product.description(), "Fried chicken pastry");
        assert_eq!(product.price(), 500);
        assert_eq!(product.category_id().as_str(), "c1");
    }

    #[test]
    fn availability_changes_only_availability() {
        let mut product = Product::new(
            ProductId::new("p1"),
            "Coxinha",
            "Fried chicken pastry",
            500,
            CategoryId::new("c1"),
            true,
        )
        .expect("construct");
        product.set_available(false);
        assert!(!product.is_available());
        assert_eq!(product.name(), "Coxinha");
        assert_eq!(product.price(), 500);
    }

    #[test]
    fn valid_category_construction() {
        let category = Category::new(CategoryId::new("c1"), "Snacks")
            .expect("a valid category must be constructed");
        assert_eq!(category.name(), "Snacks");
        assert_eq!(category.id().as_str(), "c1");
    }

    #[test]
    fn category_name_is_trimmed() {
        let category = Category::new(CategoryId::new("c1"), "  Snacks  ")
            .expect("a valid category must be constructed");
        assert_eq!(category.name(), "Snacks");
    }

    #[test]
    fn empty_category_name_is_rejected() {
        let err = Category::new(CategoryId::new("c1"), "   ")
            .expect_err("a blank category name must be rejected");
        assert_eq!(err, CatalogError::EmptyCategoryName);
    }

    #[test]
    fn overlong_category_name_is_rejected() {
        let name = "a".repeat(super::MAX_CATEGORY_NAME_CHARS + 1);
        let err = Category::new(CategoryId::new("c1"), name)
            .expect_err("an overlong category name must be rejected");
        assert_eq!(err, CatalogError::CategoryNameTooLong);
    }

    #[test]
    fn valid_rename() {
        let mut category = Category::new(CategoryId::new("c1"), "Snacks").expect("construct");
        category
            .rename("  Lanches  ")
            .expect("a valid rename must succeed");
        assert_eq!(category.name(), "Lanches");
        assert_eq!(
            category.id().as_str(),
            "c1",
            "the identifier must not change"
        );
    }

    #[test]
    fn failed_rename_leaves_original_name_unchanged() {
        let mut category = Category::new(CategoryId::new("c1"), "Snacks").expect("construct");
        let err = category
            .rename("   ")
            .expect_err("a blank rename must be rejected");
        assert_eq!(err, CatalogError::EmptyCategoryName);
        assert_eq!(category.name(), "Snacks");
    }

    #[test]
    fn normalized_name_is_case_insensitive_and_trimmed() {
        let a = Category::new(CategoryId::new("a"), "Snacks").expect("construct");
        let b = Category::new(CategoryId::new("b"), "  snacks  ").expect("construct");
        assert_eq!(a.normalized_name(), b.normalized_name());
    }

    #[test]
    fn product_ids_are_strongly_typed() {
        let a = ProductId::new("prod_x");
        let b = ProductId::new("prod_x");
        let c = ProductId::new("prod_y");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
