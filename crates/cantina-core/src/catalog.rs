//! Product catalog concepts.

use std::fmt;

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
    /// Creates a category.
    pub fn new(id: CategoryId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
        }
    }

    /// Returns the category identifier.
    pub fn id(&self) -> &CategoryId {
        &self.id
    }

    /// Returns the category name.
    pub fn name(&self) -> &str {
        &self.name
    }
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
    /// Creates a product.
    pub fn new(
        id: ProductId,
        name: impl Into<String>,
        description: impl Into<String>,
        price: u64,
        category_id: CategoryId,
        available: bool,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            description: description.into(),
            price,
            category_id,
            available,
        }
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

#[cfg(test)]
mod tests {
    use super::{CategoryId, Product, ProductId};

    #[test]
    fn product_exposes_availability() {
        let product = Product::new(
            ProductId::new("p1"),
            "Coxinha",
            "Fried chicken pastry",
            500,
            CategoryId::new("c1"),
            true,
        );
        assert!(product.is_available());
        assert_eq!(product.price(), 500);
        assert_eq!(product.id().as_str(), "p1");
        assert_eq!(product.category_id().as_str(), "c1");
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
