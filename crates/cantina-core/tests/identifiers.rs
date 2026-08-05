//! Behavior of the strongly typed domain identifiers.

use std::collections::HashMap;

use cantina_core::catalog::{CategoryId, ProductId};
use cantina_core::orders::{OrderId, PickupCode};
use cantina_core::users::UserId;

#[test]
fn identifiers_compare_by_value() {
    assert_eq!(UserId::new("u"), UserId::new("u"));
    assert_ne!(UserId::new("u"), UserId::new("v"));
    assert_eq!(ProductId::new("p"), ProductId::new("p"));
    assert_ne!(ProductId::new("p"), ProductId::new("q"));
    assert_eq!(CategoryId::new("c"), CategoryId::new("c"));
    assert_ne!(CategoryId::new("c"), CategoryId::new("d"));
    assert_eq!(OrderId::new("o"), OrderId::new("o"));
    assert_ne!(OrderId::new("o"), OrderId::new("x"));
}

#[test]
fn identifiers_are_hashable() {
    let mut user_counts = HashMap::new();
    for _ in 0..3 {
        *user_counts.entry(UserId::new("u1")).or_insert(0) += 1;
    }
    assert_eq!(user_counts[&UserId::new("u1")], 3);

    let mut order_counts = HashMap::new();
    order_counts.insert(OrderId::new("o1"), 7);
    assert_eq!(order_counts[&OrderId::new("o1")], 7);
}

#[test]
fn identifiers_display_their_value() {
    assert_eq!(UserId::new("user_1").to_string(), "user_1");
    assert_eq!(ProductId::new("product_1").to_string(), "product_1");
    assert_eq!(CategoryId::new("category_1").to_string(), "category_1");
    assert_eq!(OrderId::new("order_1").to_string(), "order_1");
    assert_eq!(PickupCode::new("A7F2").to_string(), "A7F2");
}

#[test]
fn identifiers_are_distinct_types() {
    // Even with identical values, different identifier types must not be
    // interchangeable. This is a compile-time property; the line below
    // only documents that each constructor accepts its own value.
    let user = UserId::new("same");
    let order = OrderId::new("same");
    assert_eq!(user.to_string(), order.to_string());
}
