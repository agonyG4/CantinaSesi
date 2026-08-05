//! Tests for the order use cases, driven entirely through the repository
//! ports and in-memory adapters.

#![cfg(test)]

use std::sync::Arc;
use std::time::SystemTime;

use cantina_core::catalog::{CategoryId, Product, ProductId};
use cantina_core::orders::{OrderId, OrderStatus, PickupCode};
use cantina_core::users::UserId;

use crate::error::ApplicationError;
use crate::orders::{CreateOrderCommand, OrderItemInput, OrderService};
use crate::ports::{Clock, IdGenerator, PickupCodeGenerator};
use crate::test_support::{MemOrderRepository, MemProductRepository};

/// Deterministic id generator for tests.
struct FixedIdGenerator;

impl IdGenerator for FixedIdGenerator {
    fn new_user_id(&self) -> UserId {
        UserId::new("user_1")
    }

    fn new_product_id(&self) -> ProductId {
        ProductId::new("product_1")
    }

    fn new_category_id(&self) -> CategoryId {
        CategoryId::new("category_1")
    }

    fn new_order_id(&self) -> OrderId {
        OrderId::new("order_1")
    }
}

/// Deterministic pickup code generator for tests.
struct FixedPickupCodeGenerator;

impl PickupCodeGenerator for FixedPickupCodeGenerator {
    fn generate(&self) -> PickupCode {
        PickupCode::new("A7F2")
    }
}

/// Fixed clock for tests.
struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH
    }
}

fn available_product() -> Product {
    Product::new(
        ProductId::new("product_1"),
        "Coxinha",
        "Fried chicken pastry",
        500,
        CategoryId::new("category_1"),
        true,
    )
}

fn unavailable_product() -> Product {
    Product::new(
        ProductId::new("product_2"),
        "Brigadeiro",
        "Chocolate sweet",
        300,
        CategoryId::new("category_1"),
        false,
    )
}

fn build_service(
    orders: Arc<MemOrderRepository>,
    products: Arc<MemProductRepository>,
) -> OrderService {
    OrderService::new(
        orders,
        products,
        Arc::new(FixedIdGenerator),
        Arc::new(FixedPickupCodeGenerator),
        Arc::new(FixedClock),
    )
}

/// Service backed by an empty order repository and a catalog with one
/// available product. Returns the service plus the repos for inspection.
fn seeded_service() -> (
    OrderService,
    Arc<MemOrderRepository>,
    Arc<MemProductRepository>,
) {
    let products = Arc::new(MemProductRepository::new());
    let orders = Arc::new(MemOrderRepository::new());
    products.insert(available_product());
    let service = build_service(Arc::clone(&orders), Arc::clone(&products));
    (service, orders, products)
}

fn order_command(items: Vec<OrderItemInput>) -> CreateOrderCommand {
    CreateOrderCommand {
        student_id: UserId::new("student_1"),
        items,
    }
}

#[tokio::test]
async fn creates_an_order_through_the_repository_ports() {
    let (service, orders, _) = seeded_service();
    let dto = service
        .create_order(order_command(vec![OrderItemInput {
            product_id: ProductId::new("product_1"),
            quantity: 2,
        }]))
        .await
        .unwrap();

    assert_eq!(dto.id.as_str(), "order_1");
    assert_eq!(dto.status, OrderStatus::Pending);
    assert_eq!(dto.total, 1000);
    assert_eq!(dto.pickup_code.as_str(), "A7F2");
    assert_eq!(dto.items.len(), 1);

    let stored = orders
        .order(&OrderId::new("order_1"))
        .expect("order persisted");
    assert_eq!(stored.total(), 1000);
}

#[tokio::test]
async fn rejects_unavailable_products() {
    let products = Arc::new(MemProductRepository::new());
    let orders = Arc::new(MemOrderRepository::new());
    products.insert(unavailable_product());

    let service = build_service(Arc::clone(&orders), Arc::clone(&products));
    let err = service
        .create_order(order_command(vec![OrderItemInput {
            product_id: ProductId::new("product_2"),
            quantity: 1,
        }]))
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        ApplicationError::ProductUnavailable { ref product_id } if product_id == "product_2"
    ));
}

#[tokio::test]
async fn rejects_unknown_products() {
    let products = Arc::new(MemProductRepository::new());
    let orders = Arc::new(MemOrderRepository::new());

    let service = build_service(Arc::clone(&orders), Arc::clone(&products));
    let err = service
        .create_order(order_command(vec![OrderItemInput {
            product_id: ProductId::new("missing"),
            quantity: 1,
        }]))
        .await
        .unwrap_err();

    assert!(matches!(err, ApplicationError::ProductUnavailable { .. }));
}

#[tokio::test]
async fn rejects_invalid_quantities() {
    let (service, _, _) = seeded_service();
    let err = service
        .create_order(order_command(vec![OrderItemInput {
            product_id: ProductId::new("product_1"),
            quantity: 0,
        }]))
        .await
        .unwrap_err();

    assert!(matches!(err, ApplicationError::InvalidQuantity));
}

#[tokio::test]
async fn rejects_empty_orders() {
    let products = Arc::new(MemProductRepository::new());
    let orders = Arc::new(MemOrderRepository::new());

    let service = build_service(Arc::clone(&orders), Arc::clone(&products));
    let err = service
        .create_order(order_command(vec![]))
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::EmptyOrder));
}

#[tokio::test]
async fn changes_status_through_valid_transitions() {
    let (service, _, _) = seeded_service();
    service
        .create_order(order_command(vec![OrderItemInput {
            product_id: ProductId::new("product_1"),
            quantity: 1,
        }]))
        .await
        .unwrap();

    for target in [
        OrderStatus::Confirmed,
        OrderStatus::Preparing,
        OrderStatus::Ready,
        OrderStatus::Delivered,
    ] {
        let dto = service
            .change_status(OrderId::new("order_1"), target)
            .await
            .unwrap();
        assert_eq!(dto.status, target);
    }
}

#[tokio::test]
async fn rejects_invalid_transitions() {
    let (service, _, _) = seeded_service();
    service
        .create_order(order_command(vec![OrderItemInput {
            product_id: ProductId::new("product_1"),
            quantity: 1,
        }]))
        .await
        .unwrap();

    let err = service
        .change_status(OrderId::new("order_1"), OrderStatus::Preparing)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        ApplicationError::InvalidStatusTransition {
            ref from,
            ref to
        } if from == "pending" && to == "preparing"
    ));
}

#[tokio::test]
async fn get_order_returns_not_found_for_unknown_orders() {
    let products = Arc::new(MemProductRepository::new());
    let orders = Arc::new(MemOrderRepository::new());

    let service = build_service(Arc::clone(&orders), Arc::clone(&products));
    let err = service
        .get_order(OrderId::new("order_missing"))
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        ApplicationError::NotFound {
            resource: "order",
            ..
        }
    ));
}

#[tokio::test]
async fn lists_orders_for_a_student() {
    let (service, _, _) = seeded_service();
    service
        .create_order(order_command(vec![OrderItemInput {
            product_id: ProductId::new("product_1"),
            quantity: 1,
        }]))
        .await
        .unwrap();

    let list = service
        .list_student_orders(UserId::new("student_1"))
        .await
        .unwrap();
    assert_eq!(list.len(), 1);

    let other = service
        .list_student_orders(UserId::new("student_2"))
        .await
        .unwrap();
    assert!(other.is_empty());
}

#[tokio::test]
async fn lists_active_canteen_orders() {
    let (service, _, _) = seeded_service();
    service
        .create_order(order_command(vec![OrderItemInput {
            product_id: ProductId::new("product_1"),
            quantity: 1,
        }]))
        .await
        .unwrap();

    assert_eq!(service.list_canteen_orders().await.unwrap().len(), 1);

    service
        .change_status(OrderId::new("order_1"), OrderStatus::Cancelled)
        .await
        .unwrap();

    assert!(service.list_canteen_orders().await.unwrap().is_empty());
}
