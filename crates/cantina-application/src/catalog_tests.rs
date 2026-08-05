//! Tests for the catalog use cases, driven entirely through the repository
//! ports and in-memory adapters.

#![cfg(test)]

use std::sync::Arc;

use cantina_core::catalog::{Category, CategoryId, Product, ProductId};

use crate::catalog::{CatalogService, ListProductsQuery};
use crate::test_support::{MemCategoryRepository, MemProductRepository};

fn product(
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
}

fn category(id: &str, name: &str) -> Category {
    Category::new(CategoryId::new(id), name)
}

fn seeded_service() -> CatalogService {
    let products = Arc::new(MemProductRepository::new());
    let categories = Arc::new(MemCategoryRepository::new());

    products.insert(product(
        "product_brigadeiro",
        "Brigadeiro",
        "Chocolate truffle, school classic",
        500,
        "category_snacks",
        true,
    ));
    products.insert(product(
        "product_coxinha",
        "Coxinha",
        "Fried shredded-chicken pastry",
        750,
        "category_snacks",
        true,
    ));
    products.insert(product(
        "product_refresco",
        "Refresco de limão",
        "Fresh lemonade",
        300,
        "category_drinks",
        true,
    ));
    products.insert(product(
        "product_pastel_disabled",
        "Pastel de queijo",
        "Cheese pastry, temporarily unavailable",
        600,
        "category_snacks",
        false,
    ));

    categories.insert(category("category_snacks", "Snacks"));
    categories.insert(category("category_drinks", "Drinks"));

    CatalogService::new(products, categories)
}

#[tokio::test]
async fn lists_only_available_products_by_default() {
    let service = seeded_service();
    let list = service
        .list_products(&ListProductsQuery::default())
        .await
        .unwrap();

    let ids: Vec<String> = list.iter().map(|p| p.id.as_str().to_string()).collect();
    assert!(ids.contains(&"product_brigadeiro".to_string()));
    assert!(ids.contains(&"product_coxinha".to_string()));
    assert!(!ids.contains(&"product_pastel_disabled".to_string()));
    assert!(!list.iter().any(|p| !p.available));
}

#[tokio::test]
async fn include_unavailable_lists_every_product() {
    let service = seeded_service();
    let list = service
        .list_products(&ListProductsQuery {
            include_unavailable: true,
            ..ListProductsQuery::default()
        })
        .await
        .unwrap();
    assert_eq!(list.len(), 4);
}

#[tokio::test]
async fn filters_by_category() {
    let service = seeded_service();
    let list = service
        .list_products(&ListProductsQuery {
            category_id: Some(CategoryId::new("category_drinks")),
            ..ListProductsQuery::default()
        })
        .await
        .unwrap();

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id.as_str(), "product_refresco");
}

#[tokio::test]
async fn searches_by_name_case_insensitively() {
    let service = seeded_service();
    let list = service
        .list_products(&ListProductsQuery {
            search: Some("COXINHA".to_string()),
            ..ListProductsQuery::default()
        })
        .await
        .unwrap();

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id.as_str(), "product_coxinha");
}

#[tokio::test]
async fn searches_by_description() {
    let service = seeded_service();
    let list = service
        .list_products(&ListProductsQuery {
            search: Some("lemonade".to_string()),
            ..ListProductsQuery::default()
        })
        .await
        .unwrap();

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id.as_str(), "product_refresco");
}

#[tokio::test]
async fn search_can_match_unavailable_products_when_requested() {
    let service = seeded_service();
    let list = service
        .list_products(&ListProductsQuery {
            search: Some("queijo".to_string()),
            include_unavailable: true,
            ..ListProductsQuery::default()
        })
        .await
        .unwrap();

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id.as_str(), "product_pastel_disabled");
}

#[tokio::test]
async fn combines_search_and_category_filters() {
    let service = seeded_service();
    let list = service
        .list_products(&ListProductsQuery {
            category_id: Some(CategoryId::new("category_snacks")),
            search: Some("pastry".to_string()),
            include_unavailable: true,
        })
        .await
        .unwrap();

    let ids: Vec<String> = list.iter().map(|p| p.id.as_str().to_string()).collect();
    assert!(ids.contains(&"product_coxinha".to_string()));
    assert!(ids.contains(&"product_pastel_disabled".to_string()));
    assert!(!ids.contains(&"product_brigadeiro".to_string()));
}

#[tokio::test]
async fn products_are_sorted_by_name_then_id() {
    let service = seeded_service();
    let list = service
        .list_products(&ListProductsQuery {
            include_unavailable: true,
            ..ListProductsQuery::default()
        })
        .await
        .unwrap();

    let names: Vec<String> = list.iter().map(|p| p.name.clone()).collect();
    assert_eq!(
        names,
        vec![
            "Brigadeiro".to_string(),
            "Coxinha".to_string(),
            "Pastel de queijo".to_string(),
            "Refresco de limão".to_string(),
        ]
    );
}

#[tokio::test]
async fn categories_are_sorted_by_name_then_id() {
    let service = seeded_service();
    let list = service.list_categories().await.unwrap();

    let names: Vec<String> = list.iter().map(|c| c.name.clone()).collect();
    assert_eq!(names, vec!["Drinks".to_string(), "Snacks".to_string()]);
}

#[tokio::test]
async fn gets_a_product_by_id() {
    let service = seeded_service();
    let product = service
        .get_product(ProductId::new("product_coxinha"))
        .await
        .unwrap();

    assert_eq!(product.name, "Coxinha");
    assert_eq!(product.price, 750);
    assert_eq!(product.category_id.as_str(), "category_snacks");
    assert!(product.available);
}

#[tokio::test]
async fn get_product_returns_not_found_for_unknown_products() {
    let service = seeded_service();
    let err = service
        .get_product(ProductId::new("product_missing"))
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        crate::error::ApplicationError::NotFound {
            resource: "product",
            ..
        }
    ));
}

#[tokio::test]
async fn gets_a_category_by_id() {
    let service = seeded_service();
    let category = service
        .get_category(CategoryId::new("category_drinks"))
        .await
        .unwrap();

    assert_eq!(category.name, "Drinks");
}

#[tokio::test]
async fn get_category_returns_not_found_for_unknown_categories() {
    let service = seeded_service();
    let err = service
        .get_category(CategoryId::new("category_missing"))
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        crate::error::ApplicationError::NotFound {
            resource: "category",
            ..
        }
    ));
}
