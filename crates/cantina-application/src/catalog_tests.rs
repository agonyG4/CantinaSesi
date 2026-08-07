//! Tests for the catalog use cases, driven entirely through the repository
//! ports and in-memory adapters.

#![cfg(test)]

use std::sync::Arc;

use cantina_core::auth::Principal;
use cantina_core::catalog::{Category, CategoryId, Product, ProductId};
use cantina_core::orders::OrderId;
use cantina_core::users::{UserId, UserRole};

use crate::catalog::{
    CatalogService, CreateCategoryCommand, CreateProductCommand, ListProductsQuery,
    SetProductAvailabilityCommand, UpdateCategoryCommand, UpdateProductCommand,
};
use crate::error::ApplicationError;
use crate::ports::IdGenerator;
use crate::test_support::{MemCategoryRepository, MemProductRepository};

/// Deterministic id generator for catalog tests.
#[derive(Default)]
struct CountingIdGenerator {
    products: std::sync::atomic::AtomicU64,
    categories: std::sync::atomic::AtomicU64,
}

impl IdGenerator for CountingIdGenerator {
    fn new_user_id(&self) -> UserId {
        UserId::new("user_1")
    }

    fn new_product_id(&self) -> ProductId {
        let n = self
            .products
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        ProductId::new(format!("generated_product_{n}"))
    }

    fn new_category_id(&self) -> CategoryId {
        let n = self
            .categories
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        CategoryId::new(format!("generated_category_{n}"))
    }

    fn new_order_id(&self) -> OrderId {
        OrderId::new("order_1")
    }
}

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
    .expect("test product must be valid")
}

fn category(id: &str, name: &str) -> Category {
    Category::new(CategoryId::new(id), name).expect("test category must be valid")
}

fn principal(role: UserRole) -> Principal {
    Principal::new(UserId::new("user_1"), role)
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

    CatalogService::new(
        products,
        categories,
        Arc::new(CountingIdGenerator::default()),
    )
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

#[tokio::test]
async fn canteen_can_create_a_product() {
    let service = seeded_service();
    let created = service
        .create_product(
            &principal(UserRole::Canteen),
            CreateProductCommand {
                name: "Torta de frango".to_string(),
                description: "Chicken pie slice".to_string(),
                price: 900,
                category_id: CategoryId::new("category_snacks"),
                available: true,
            },
        )
        .await
        .unwrap();

    assert_eq!(created.id.as_str(), "generated_product_0");
    assert_eq!(created.name, "Torta de frango");
    assert_eq!(created.description, "Chicken pie slice");
    assert_eq!(created.price, 900);
    assert_eq!(created.category_id.as_str(), "category_snacks");
    assert!(created.available);

    let fetched = service
        .get_product(ProductId::new("generated_product_0"))
        .await
        .unwrap();
    assert_eq!(fetched.name, "Torta de frango");
}

#[tokio::test]
async fn admin_can_create_a_product() {
    let service = seeded_service();
    let created = service
        .create_product(
            &principal(UserRole::Admin),
            CreateProductCommand {
                name: "Suco de laranja".to_string(),
                description: "Fresh orange juice".to_string(),
                price: 400,
                category_id: CategoryId::new("category_drinks"),
                available: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(created.name, "Suco de laranja");
}

#[tokio::test]
async fn student_cannot_create_a_product() {
    let service = seeded_service();
    let err = service
        .create_product(
            &principal(UserRole::Student),
            CreateProductCommand {
                name: "Bolo".to_string(),
                description: "Cake".to_string(),
                price: 800,
                category_id: CategoryId::new("category_snacks"),
                available: true,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::Forbidden));
}

#[tokio::test]
async fn create_product_requires_an_existing_category() {
    let service = seeded_service();
    let err = service
        .create_product(
            &principal(UserRole::Canteen),
            CreateProductCommand {
                name: "Bolo".to_string(),
                description: "Cake".to_string(),
                price: 800,
                category_id: CategoryId::new("category_missing"),
                available: true,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        ApplicationError::NotFound {
            resource: "category",
            ..
        }
    ));
}

#[tokio::test]
async fn create_product_rejects_blank_name() {
    let service = seeded_service();
    let err = service
        .create_product(
            &principal(UserRole::Canteen),
            CreateProductCommand {
                name: "   ".to_string(),
                description: "Cake".to_string(),
                price: 800,
                category_id: CategoryId::new("category_snacks"),
                available: true,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::InvalidProductName));
}

#[tokio::test]
async fn create_product_rejects_zero_price() {
    let service = seeded_service();
    let err = service
        .create_product(
            &principal(UserRole::Canteen),
            CreateProductCommand {
                name: "Bolo".to_string(),
                description: "Cake".to_string(),
                price: 0,
                category_id: CategoryId::new("category_snacks"),
                available: true,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::InvalidProductPrice));
}

#[tokio::test]
async fn canteen_can_update_product_fields() {
    let service = seeded_service();
    let updated = service
        .update_product(
            &principal(UserRole::Canteen),
            UpdateProductCommand {
                product_id: ProductId::new("product_coxinha"),
                name: Some("Coxinha grande".to_string()),
                description: None,
                price: Some(900),
                category_id: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(updated.name, "Coxinha grande");
    assert_eq!(updated.description, "Fried shredded-chicken pastry");
    assert_eq!(updated.price, 900);
    assert_eq!(updated.category_id.as_str(), "category_snacks");
}

#[tokio::test]
async fn update_product_rejects_an_empty_update() {
    let service = seeded_service();
    let err = service
        .update_product(
            &principal(UserRole::Canteen),
            UpdateProductCommand {
                product_id: ProductId::new("product_coxinha"),
                name: None,
                description: None,
                price: None,
                category_id: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::EmptyUpdate));
}

#[tokio::test]
async fn update_product_returns_not_found_for_unknown_product() {
    let service = seeded_service();
    let err = service
        .update_product(
            &principal(UserRole::Canteen),
            UpdateProductCommand {
                product_id: ProductId::new("product_missing"),
                name: Some("Novo".to_string()),
                description: None,
                price: None,
                category_id: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        ApplicationError::NotFound {
            resource: "product",
            ..
        }
    ));
}

#[tokio::test]
async fn update_product_rejects_an_unknown_category() {
    let service = seeded_service();
    let err = service
        .update_product(
            &principal(UserRole::Canteen),
            UpdateProductCommand {
                product_id: ProductId::new("product_coxinha"),
                name: None,
                description: None,
                price: None,
                category_id: Some(CategoryId::new("category_missing")),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        ApplicationError::NotFound {
            resource: "category",
            ..
        }
    ));
}

#[tokio::test]
async fn student_cannot_update_a_product() {
    let service = seeded_service();
    let err = service
        .update_product(
            &principal(UserRole::Student),
            UpdateProductCommand {
                product_id: ProductId::new("product_coxinha"),
                name: Some("Novo".to_string()),
                description: None,
                price: None,
                category_id: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::Forbidden));
}

#[tokio::test]
async fn canteen_can_disable_and_re_enable_a_product() {
    let service = seeded_service();
    let disabled = service
        .set_product_availability(
            &principal(UserRole::Canteen),
            SetProductAvailabilityCommand {
                product_id: ProductId::new("product_coxinha"),
                available: false,
            },
        )
        .await
        .unwrap();
    assert!(!disabled.available);

    let enabled = service
        .set_product_availability(
            &principal(UserRole::Canteen),
            SetProductAvailabilityCommand {
                product_id: ProductId::new("product_coxinha"),
                available: true,
            },
        )
        .await
        .unwrap();
    assert!(enabled.available);
}

#[tokio::test]
async fn disabling_is_idempotent() {
    let service = seeded_service();
    let first = service
        .set_product_availability(
            &principal(UserRole::Admin),
            SetProductAvailabilityCommand {
                product_id: ProductId::new("product_pastel_disabled"),
                available: false,
            },
        )
        .await
        .unwrap();
    let second = service
        .set_product_availability(
            &principal(UserRole::Admin),
            SetProductAvailabilityCommand {
                product_id: ProductId::new("product_pastel_disabled"),
                available: false,
            },
        )
        .await
        .unwrap();
    assert!(!first.available);
    assert!(!second.available);
}

#[tokio::test]
async fn set_availability_returns_not_found_for_unknown_product() {
    let service = seeded_service();
    let err = service
        .set_product_availability(
            &principal(UserRole::Canteen),
            SetProductAvailabilityCommand {
                product_id: ProductId::new("product_missing"),
                available: true,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        ApplicationError::NotFound {
            resource: "product",
            ..
        }
    ));
}

#[tokio::test]
async fn student_cannot_change_product_availability() {
    let service = seeded_service();
    let err = service
        .set_product_availability(
            &principal(UserRole::Student),
            SetProductAvailabilityCommand {
                product_id: ProductId::new("product_coxinha"),
                available: false,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::Forbidden));
}

#[tokio::test]
async fn admin_can_create_a_category() {
    let service = seeded_service();
    let created = service
        .create_category(
            &principal(UserRole::Admin),
            CreateCategoryCommand {
                name: "Doces".to_string(),
            },
        )
        .await
        .unwrap();

    assert_eq!(created.id.as_str(), "generated_category_0");
    assert_eq!(created.name, "Doces");

    let fetched = service
        .get_category(CategoryId::new("generated_category_0"))
        .await
        .unwrap();
    assert_eq!(fetched.name, "Doces");
}

#[tokio::test]
async fn create_category_is_case_insensitively_unique() {
    let service = seeded_service();
    let err = service
        .create_category(
            &principal(UserRole::Admin),
            CreateCategoryCommand {
                name: "drinks".to_string(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::CategoryNameConflict));
}

#[tokio::test]
async fn create_category_rejects_blank_name() {
    let service = seeded_service();
    let err = service
        .create_category(
            &principal(UserRole::Admin),
            CreateCategoryCommand {
                name: "  ".to_string(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::InvalidCategoryName));
}

#[tokio::test]
async fn non_admin_cannot_create_a_category() {
    let service = seeded_service();
    for role in [UserRole::Student, UserRole::Canteen] {
        let err = service
            .create_category(
                &principal(role),
                CreateCategoryCommand {
                    name: "Doces".to_string(),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, ApplicationError::Forbidden), "role {role:?}");
    }
}

#[tokio::test]
async fn admin_can_rename_a_category() {
    let service = seeded_service();
    let updated = service
        .update_category(
            &principal(UserRole::Admin),
            UpdateCategoryCommand {
                category_id: CategoryId::new("category_snacks"),
                name: "Lanches".to_string(),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.name, "Lanches");
}

#[tokio::test]
async fn update_category_keeps_its_own_name() {
    let service = seeded_service();
    let updated = service
        .update_category(
            &principal(UserRole::Admin),
            UpdateCategoryCommand {
                category_id: CategoryId::new("category_snacks"),
                name: "SNACKS".to_string(),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.name, "SNACKS");
}

#[tokio::test]
async fn update_category_rejects_another_categories_name() {
    let service = seeded_service();
    let err = service
        .update_category(
            &principal(UserRole::Admin),
            UpdateCategoryCommand {
                category_id: CategoryId::new("category_snacks"),
                name: "drinks".to_string(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::CategoryNameConflict));
}

#[tokio::test]
async fn update_category_returns_not_found_for_unknown_category() {
    let service = seeded_service();
    let err = service
        .update_category(
            &principal(UserRole::Admin),
            UpdateCategoryCommand {
                category_id: CategoryId::new("category_missing"),
                name: "Doces".to_string(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        err,
        ApplicationError::NotFound {
            resource: "category",
            ..
        }
    ));
}

#[tokio::test]
async fn canteen_cannot_update_a_category() {
    let service = seeded_service();
    let err = service
        .update_category(
            &principal(UserRole::Canteen),
            UpdateCategoryCommand {
                category_id: CategoryId::new("category_snacks"),
                name: "Lanches".to_string(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::Forbidden));
}

#[tokio::test]
async fn create_product_maps_a_storage_failure_to_internal_error() {
    let products = Arc::new(MemProductRepository::new());
    products.fail_writes();
    let categories = Arc::new(MemCategoryRepository::new());
    categories.insert(category("category_snacks", "Snacks"));
    let service = CatalogService::new(
        products,
        categories,
        Arc::new(CountingIdGenerator::default()),
    );

    let err = service
        .create_product(
            &principal(UserRole::Canteen),
            CreateProductCommand {
                name: "Bolo".to_string(),
                description: "Cake".to_string(),
                price: 800,
                category_id: CategoryId::new("category_snacks"),
                available: true,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::Storage(_)));
}

#[tokio::test]
async fn create_category_maps_a_name_conflict_to_category_name_conflict() {
    let categories = Arc::new(MemCategoryRepository::new());
    categories.insert(category("category_snacks", "Snacks"));
    let service = CatalogService::new(
        Arc::new(MemProductRepository::new()),
        categories,
        Arc::new(CountingIdGenerator::default()),
    );

    let err = service
        .create_category(
            &principal(UserRole::Admin),
            CreateCategoryCommand {
                name: "SNACKS".to_string(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::CategoryNameConflict));
}
