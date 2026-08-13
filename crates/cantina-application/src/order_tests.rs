//! Tests for the order use cases, driven entirely through the repository
//! ports and in-memory adapters.

#![cfg(test)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use cantina_core::auth::Principal;
use cantina_core::catalog::{CategoryId, Product, ProductId};
use cantina_core::orders::{Order, OrderId, OrderItem, OrderStatus, PickupCode};
use cantina_core::users::{UserId, UserRole};
use tokio::sync::Barrier;

use crate::error::ApplicationError;
use crate::orders::{CreateOrderCommand, OrderItemInput, OrderService};
use crate::ports::{
    Clock, IdGenerator, OrderRepository, OrderStatusTransitionError, PickupCodeGenerationError,
    PickupCodeGenerator,
};
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
    fn generate(&self) -> Result<PickupCode, PickupCodeGenerationError> {
        Ok(PickupCode::new("A7F2"))
    }
}

struct SequencePickupCodeGenerator {
    codes: Mutex<VecDeque<PickupCode>>,
}

impl SequencePickupCodeGenerator {
    fn new(codes: impl IntoIterator<Item = &'static str>) -> Self {
        Self {
            codes: Mutex::new(codes.into_iter().map(PickupCode::new).collect()),
        }
    }
}

impl PickupCodeGenerator for SequencePickupCodeGenerator {
    fn generate(&self) -> Result<PickupCode, PickupCodeGenerationError> {
        Ok(self
            .codes
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| PickupCode::new("A7F2")))
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
    .expect("test product must be valid")
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
    .expect("test product must be valid")
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

fn build_service_with_pickup_generator(
    orders: Arc<MemOrderRepository>,
    products: Arc<MemProductRepository>,
    pickup_codes: Arc<dyn PickupCodeGenerator>,
) -> OrderService {
    OrderService::new(
        orders,
        products,
        Arc::new(FixedIdGenerator),
        pickup_codes,
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
    CreateOrderCommand { items }
}

fn student_principal(id: &str) -> Principal {
    Principal::new(UserId::new(id), UserRole::Student)
}

fn canteen_principal() -> Principal {
    Principal::new(UserId::new("canteen_1"), UserRole::Canteen)
}

fn admin_principal() -> Principal {
    Principal::new(UserId::new("admin_1"), UserRole::Admin)
}

#[tokio::test]
async fn creates_an_order_through_the_repository_ports() {
    let (service, orders, products) = seeded_service();
    let dto = service
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_1"),
                quantity: 2,
            }]),
        )
        .await
        .unwrap();

    assert_eq!(dto.id.as_str(), "order_1");
    assert_eq!(dto.status, OrderStatus::Pending);
    assert_eq!(dto.total, 1000);
    assert_eq!(dto.pickup_code.as_str(), "A7F2");
    assert_eq!(dto.items.len(), 1);
    assert_eq!(products.bulk_find_calls(), 1);

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
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_2"),
                quantity: 1,
            }]),
        )
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
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("missing"),
                quantity: 1,
            }]),
        )
        .await
        .unwrap_err();

    assert!(matches!(err, ApplicationError::ProductUnavailable { .. }));
}

#[tokio::test]
async fn rejects_invalid_quantities() {
    let (service, _, _) = seeded_service();
    let err = service
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_1"),
                quantity: 0,
            }]),
        )
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
        .create_order(&student_principal("student_1"), order_command(vec![]))
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::EmptyOrder));
}

#[tokio::test]
async fn changes_status_through_valid_transitions() {
    let (service, _, _) = seeded_service();
    service
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_1"),
                quantity: 1,
            }]),
        )
        .await
        .unwrap();

    for target in [
        OrderStatus::Confirmed,
        OrderStatus::Preparing,
        OrderStatus::Ready,
        OrderStatus::Delivered,
    ] {
        let dto = service
            .change_status(&canteen_principal(), OrderId::new("order_1"), target)
            .await
            .unwrap();
        assert_eq!(dto.status, target);
    }
}

#[tokio::test]
async fn rejects_invalid_transitions() {
    let (service, _, _) = seeded_service();
    service
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_1"),
                quantity: 1,
            }]),
        )
        .await
        .unwrap();

    let err = service
        .change_status(
            &canteen_principal(),
            OrderId::new("order_1"),
            OrderStatus::Preparing,
        )
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
        .get_order(
            &student_principal("student_1"),
            OrderId::new("order_missing"),
        )
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
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_1"),
                quantity: 1,
            }]),
        )
        .await
        .unwrap();

    let list = service
        .list_student_orders(&student_principal("student_1"))
        .await
        .unwrap();
    assert_eq!(list.len(), 1);

    let other = service
        .list_student_orders(&student_principal("student_2"))
        .await
        .unwrap();
    assert!(other.is_empty());
}

#[tokio::test]
async fn lists_active_canteen_orders() {
    let (service, _, _) = seeded_service();
    service
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_1"),
                quantity: 1,
            }]),
        )
        .await
        .unwrap();

    assert_eq!(
        service
            .list_canteen_orders(&canteen_principal())
            .await
            .unwrap()
            .len(),
        1
    );

    service
        .change_status(
            &canteen_principal(),
            OrderId::new("order_1"),
            OrderStatus::Cancelled,
        )
        .await
        .unwrap();

    assert!(
        service
            .list_canteen_orders(&admin_principal())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn student_cannot_access_canteen_operations() {
    let (service, _, _) = seeded_service();

    assert!(matches!(
        service
            .list_canteen_orders(&student_principal("student_1"))
            .await,
        Err(ApplicationError::Forbidden)
    ));
}

#[tokio::test]
async fn student_cannot_change_order_status() {
    let (service, _, _) = seeded_service();
    service
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_1"),
                quantity: 1,
            }]),
        )
        .await
        .unwrap();

    assert!(matches!(
        service
            .change_status(
                &student_principal("student_1"),
                OrderId::new("order_1"),
                OrderStatus::Confirmed,
            )
            .await,
        Err(ApplicationError::Forbidden)
    ));
}

#[tokio::test]
async fn student_cannot_read_another_students_order() {
    let (service, orders, _) = seeded_service();
    let other_order = Order::new(
        OrderId::new("order_other"),
        UserId::new("student_2"),
        vec![OrderItem::new(
            ProductId::new("product_1"),
            "Coxinha",
            500,
            1,
        )],
        PickupCode::new("OTHER2"),
        SystemTime::UNIX_EPOCH,
    )
    .unwrap();
    orders.insert(other_order);

    let err = service
        .get_order(&student_principal("student_1"), OrderId::new("order_other"))
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
async fn duplicate_product_lines_are_consolidated() {
    let (service, _, _) = seeded_service();
    let dto = service
        .create_order(
            &student_principal("student_1"),
            order_command(vec![
                OrderItemInput {
                    product_id: ProductId::new("product_1"),
                    quantity: 2,
                },
                OrderItemInput {
                    product_id: ProductId::new("product_1"),
                    quantity: 3,
                },
            ]),
        )
        .await
        .unwrap();

    assert_eq!(dto.items.len(), 1);
    assert_eq!(dto.items[0].quantity, 5);
    assert_eq!(dto.total, 2500);
}

#[tokio::test]
async fn rejects_quantity_above_the_centralized_limit() {
    let (service, _, _) = seeded_service();
    let err = service
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_1"),
                quantity: 21,
            }]),
        )
        .await
        .unwrap_err();

    assert!(matches!(err, ApplicationError::QuantityLimitExceeded));
}

#[tokio::test]
async fn rejects_total_quantity_above_the_centralized_limit() {
    let products = Arc::new(MemProductRepository::new());
    let orders = Arc::new(MemOrderRepository::new());
    products.insert(available_product());
    for index in 2..=6 {
        products.insert(
            Product::new(
                ProductId::new(format!("product_{index}")),
                format!("Product {index}"),
                "Test product",
                100,
                CategoryId::new("category_1"),
                true,
            )
            .unwrap(),
        );
    }
    let service = build_service(Arc::clone(&orders), Arc::clone(&products));
    let err = service
        .create_order(
            &student_principal("student_1"),
            order_command(
                (1..=6)
                    .map(|index| OrderItemInput {
                        product_id: ProductId::new(format!("product_{index}")),
                        quantity: 20,
                    })
                    .collect(),
            ),
        )
        .await
        .unwrap_err();

    assert!(matches!(err, ApplicationError::TotalQuantityLimitExceeded));
}

#[tokio::test]
async fn competing_status_transitions_allow_only_one_winner() {
    let (service, orders, _) = seeded_service();
    service
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_1"),
                quantity: 1,
            }]),
        )
        .await
        .unwrap();

    let barrier = Arc::new(Barrier::new(3));
    let confirmed_orders = Arc::clone(&orders);
    let confirmed_barrier = Arc::clone(&barrier);
    let confirmed = tokio::spawn(async move {
        confirmed_barrier.wait().await;
        confirmed_orders
            .transition_order_status(&OrderId::new("order_1"), OrderStatus::Confirmed)
            .await
    });
    let cancelled_orders = Arc::clone(&orders);
    let cancelled_barrier = Arc::clone(&barrier);
    let cancelled = tokio::spawn(async move {
        cancelled_barrier.wait().await;
        cancelled_orders
            .transition_order_status(&OrderId::new("order_1"), OrderStatus::Cancelled)
            .await
    });

    barrier.wait().await;
    let results = [confirmed.await.unwrap(), cancelled.await.unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
    assert!(results.iter().any(|result| matches!(
        result,
        Err(OrderStatusTransitionError::InvalidStatusTransition { .. })
    )));

    let stored = orders.order(&OrderId::new("order_1")).unwrap();
    assert!(matches!(
        stored.status(),
        OrderStatus::Confirmed | OrderStatus::Cancelled
    ));
}

#[tokio::test]
async fn rejects_a_stale_transition_after_the_first_transition_commits() {
    let (service, orders, _) = seeded_service();
    service
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_1"),
                quantity: 1,
            }]),
        )
        .await
        .unwrap();

    orders
        .transition_order_status(&OrderId::new("order_1"), OrderStatus::Cancelled)
        .await
        .unwrap();
    let stale = orders
        .transition_order_status(&OrderId::new("order_1"), OrderStatus::Confirmed)
        .await
        .unwrap_err();

    assert_eq!(
        stale,
        OrderStatusTransitionError::InvalidStatusTransition {
            from: OrderStatus::Cancelled,
            to: OrderStatus::Confirmed,
        }
    );
}

#[tokio::test]
async fn concurrent_identical_transitions_report_one_success_and_one_conflict() {
    let (service, orders, _) = seeded_service();
    service
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_1"),
                quantity: 1,
            }]),
        )
        .await
        .unwrap();

    let barrier = Arc::new(Barrier::new(3));
    let first_orders = Arc::clone(&orders);
    let first_barrier = Arc::clone(&barrier);
    let first = tokio::spawn(async move {
        first_barrier.wait().await;
        first_orders
            .transition_order_status(&OrderId::new("order_1"), OrderStatus::Confirmed)
            .await
    });
    let second_orders = Arc::clone(&orders);
    let second_barrier = Arc::clone(&barrier);
    let second = tokio::spawn(async move {
        second_barrier.wait().await;
        second_orders
            .transition_order_status(&OrderId::new("order_1"), OrderStatus::Confirmed)
            .await
    });

    barrier.wait().await;
    let results = [first.await.unwrap(), second.await.unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert!(results.iter().any(|result| matches!(
        result,
        Err(OrderStatusTransitionError::InvalidStatusTransition {
            from: OrderStatus::Confirmed,
            to: OrderStatus::Confirmed,
        })
    )));
    assert_eq!(
        orders.order(&OrderId::new("order_1")).unwrap().status(),
        OrderStatus::Confirmed
    );
}

#[tokio::test]
async fn retries_pickup_code_collision_and_persists_the_next_candidate() {
    let products = Arc::new(MemProductRepository::new());
    let orders = Arc::new(MemOrderRepository::new());
    products.insert(available_product());
    orders.insert(
        Order::new(
            OrderId::new("existing"),
            UserId::new("student_2"),
            vec![OrderItem::new(
                ProductId::new("product_1"),
                "Coxinha",
                500,
                1,
            )],
            PickupCode::new("A7F2"),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap(),
    );
    let service = build_service_with_pickup_generator(
        Arc::clone(&orders),
        Arc::clone(&products),
        Arc::new(SequencePickupCodeGenerator::new(["A7F2", "B8G3"])),
    );

    let dto = service
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_1"),
                quantity: 1,
            }]),
        )
        .await
        .unwrap();

    assert_eq!(dto.pickup_code.as_str(), "B8G3");
}

#[tokio::test]
async fn pickup_code_retry_exhaustion_is_a_stable_error() {
    let products = Arc::new(MemProductRepository::new());
    let orders = Arc::new(MemOrderRepository::new());
    products.insert(available_product());
    orders.insert(
        Order::new(
            OrderId::new("existing"),
            UserId::new("student_2"),
            vec![OrderItem::new(
                ProductId::new("product_1"),
                "Coxinha",
                500,
                1,
            )],
            PickupCode::new("A7F2"),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap(),
    );
    let service = build_service_with_pickup_generator(
        Arc::clone(&orders),
        Arc::clone(&products),
        Arc::new(SequencePickupCodeGenerator::new([])),
    );

    let err = service
        .create_order(
            &student_principal("student_1"),
            order_command(vec![OrderItemInput {
                product_id: ProductId::new("product_1"),
                quantity: 1,
            }]),
        )
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        ApplicationError::PickupCodeGenerationExhausted
    ));
    assert!(orders.order(&OrderId::new("order_1")).is_none());
}
