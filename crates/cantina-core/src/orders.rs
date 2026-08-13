//! Order lifecycle concepts, including valid status transitions.

use std::error::Error;
use std::fmt;
use std::str::FromStr;
use std::time::SystemTime;

use crate::catalog::ProductId;
use crate::users::UserId;

/// Maximum number of distinct product lines in one order.
pub const MAX_ORDER_LINES: usize = 20;
/// Maximum quantity of one product in one order.
pub const MAX_QUANTITY_PER_PRODUCT: u32 = 20;
/// Maximum total quantity across all consolidated order lines.
pub const MAX_TOTAL_QUANTITY: u32 = 100;

/// Strongly typed identifier for an [`Order`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OrderId(String);

impl OrderId {
    /// Creates an order identifier from a raw string value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the underlying identifier value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Short code shown to the student so they can collect their order.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PickupCode(String);

impl PickupCode {
    /// Creates a pickup code from a raw string value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Borrows the underlying code value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PickupCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Lifecycle state of an [`Order`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OrderStatus {
    /// Order received, awaiting confirmation.
    Pending,
    /// Order accepted by the cafeteria.
    Confirmed,
    /// Order is being prepared.
    Preparing,
    /// Order is ready for pickup.
    Ready,
    /// Order was handed to the student.
    Delivered,
    /// Order was cancelled.
    Cancelled,
}

impl OrderStatus {
    /// Returns the canonical lowercase representation of the status.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Confirmed => "confirmed",
            Self::Preparing => "preparing",
            Self::Ready => "ready",
            Self::Delivered => "delivered",
            Self::Cancelled => "cancelled",
        }
    }

    /// Returns `true` when the order is still being worked on.
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Pending | Self::Confirmed | Self::Preparing | Self::Ready
        )
    }

    /// Returns `true` when `self` may legally move to `target`.
    pub fn can_transition_to(self, target: Self) -> bool {
        matches!(
            (self, target),
            (Self::Pending, Self::Confirmed)
                | (Self::Pending, Self::Cancelled)
                | (Self::Confirmed, Self::Preparing)
                | (Self::Confirmed, Self::Cancelled)
                | (Self::Preparing, Self::Ready)
                | (Self::Ready, Self::Delivered)
        )
    }
}

impl fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for OrderStatus {
    type Err = ParseStatusError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "confirmed" => Ok(Self::Confirmed),
            "preparing" => Ok(Self::Preparing),
            "ready" => Ok(Self::Ready),
            "delivered" => Ok(Self::Delivered),
            "cancelled" => Ok(Self::Cancelled),
            other => Err(ParseStatusError {
                value: other.to_string(),
            }),
        }
    }
}

/// Error returned when an [`OrderStatus`] cannot be parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseStatusError {
    value: String,
}

impl fmt::Display for ParseStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown order status '{}'", self.value)
    }
}

impl Error for ParseStatusError {}

/// A single line of an [`Order`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderItem {
    pub product_id: ProductId,
    pub product_name: String,
    /// Unit price in minor units at the moment the order was placed.
    pub unit_price: u64,
    pub quantity: u32,
}

impl OrderItem {
    /// Creates an order item.
    pub fn new(
        product_id: ProductId,
        product_name: impl Into<String>,
        unit_price: u64,
        quantity: u32,
    ) -> Self {
        Self {
            product_id,
            product_name: product_name.into(),
            unit_price,
            quantity,
        }
    }
}

/// A student order placed at the cafeteria.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    id: OrderId,
    student_id: UserId,
    items: Vec<OrderItem>,
    total: u64,
    status: OrderStatus,
    pickup_code: PickupCode,
    created_at: SystemTime,
}

impl Order {
    /// Creates an order in `Pending` status.
    ///
    /// Rejects orders without items and items with quantity zero.
    pub fn new(
        id: OrderId,
        student_id: UserId,
        items: Vec<OrderItem>,
        pickup_code: PickupCode,
        created_at: SystemTime,
    ) -> Result<Self, OrderError> {
        if items.is_empty() {
            return Err(OrderError::EmptyOrder);
        }
        if items.iter().any(|item| item.quantity == 0) {
            return Err(OrderError::InvalidQuantity);
        }
        let total = checked_total(&items)?;
        Ok(Self {
            id,
            student_id,
            items,
            total,
            status: OrderStatus::Pending,
            pickup_code,
            created_at,
        })
    }

    /// Returns the order identifier.
    pub fn id(&self) -> &OrderId {
        &self.id
    }

    /// Returns the student who placed the order.
    pub fn student_id(&self) -> &UserId {
        &self.student_id
    }

    /// Returns the order lines.
    pub fn items(&self) -> &[OrderItem] {
        &self.items
    }

    /// Returns the current status.
    pub fn status(&self) -> OrderStatus {
        self.status
    }

    /// Returns the pickup code assigned to the order.
    pub fn pickup_code(&self) -> &PickupCode {
        &self.pickup_code
    }

    /// Returns the creation timestamp.
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    /// Returns the order total in minor units.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Moves the order to `target` when the transition is legal.
    pub fn change_status(&mut self, target: OrderStatus) -> Result<(), OrderError> {
        if self.status.can_transition_to(target) {
            self.status = target;
            Ok(())
        } else {
            Err(OrderError::InvalidStatusTransition {
                from: self.status,
                to: target,
            })
        }
    }
}

/// Domain error raised while building or mutating an [`Order`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderError {
    /// The order must contain at least one item.
    EmptyOrder,
    /// Every item must have a quantity greater than zero.
    InvalidQuantity,
    /// The order total cannot be represented safely in minor currency units.
    TotalOverflow,
    /// The requested status change is not allowed.
    InvalidStatusTransition { from: OrderStatus, to: OrderStatus },
}

impl fmt::Display for OrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyOrder => write!(f, "an order must contain at least one item"),
            Self::InvalidQuantity => write!(f, "item quantity must be greater than zero"),
            Self::TotalOverflow => write!(f, "the order total is too large"),
            Self::InvalidStatusTransition { from, to } => {
                write!(f, "cannot transition order from '{from}' to '{to}'")
            }
        }
    }
}

fn checked_total(items: &[OrderItem]) -> Result<u64, OrderError> {
    items.iter().try_fold(0_u64, |total, item| {
        let subtotal = item
            .unit_price
            .checked_mul(u64::from(item.quantity))
            .ok_or(OrderError::TotalOverflow)?;
        total.checked_add(subtotal).ok_or(OrderError::TotalOverflow)
    })
}

impl Error for OrderError {}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use crate::catalog::ProductId;
    use crate::users::UserId;

    use super::{Order, OrderError, OrderId, OrderItem, OrderStatus, PickupCode};

    fn sample_order() -> Order {
        Order::new(
            OrderId::new("order_1"),
            UserId::new("student_1"),
            vec![OrderItem::new(ProductId::new("p1"), "Coxinha", 500, 2)],
            PickupCode::new("A7F2"),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    /// Returns a sample order moved to `status` through a legal path.
    fn order_in(status: OrderStatus) -> Order {
        let mut order = sample_order();
        let path: &[OrderStatus] = match status {
            OrderStatus::Pending => &[],
            OrderStatus::Confirmed => &[OrderStatus::Confirmed],
            OrderStatus::Preparing => &[OrderStatus::Confirmed, OrderStatus::Preparing],
            OrderStatus::Ready => &[
                OrderStatus::Confirmed,
                OrderStatus::Preparing,
                OrderStatus::Ready,
            ],
            OrderStatus::Delivered => &[
                OrderStatus::Confirmed,
                OrderStatus::Preparing,
                OrderStatus::Ready,
                OrderStatus::Delivered,
            ],
            OrderStatus::Cancelled => &[OrderStatus::Cancelled],
        };
        for target in path {
            order.change_status(*target).unwrap();
        }
        order
    }

    #[test]
    fn parses_known_statuses_case_insensitively() {
        assert_eq!(
            "pending".parse::<OrderStatus>().unwrap(),
            OrderStatus::Pending
        );
        assert_eq!(
            "CONFIRMED".parse::<OrderStatus>().unwrap(),
            OrderStatus::Confirmed
        );
        assert_eq!(
            " Ready ".parse::<OrderStatus>().unwrap(),
            OrderStatus::Ready
        );
        assert_eq!(
            "delivered".parse::<OrderStatus>().unwrap(),
            OrderStatus::Delivered
        );
        assert_eq!(
            "cancelled".parse::<OrderStatus>().unwrap(),
            OrderStatus::Cancelled
        );
    }

    #[test]
    fn rejects_unknown_statuses() {
        let err = "shipped".parse::<OrderStatus>().unwrap_err();
        assert_eq!(err.to_string(), "unknown order status 'shipped'");
    }

    #[test]
    fn accepts_every_valid_transition() {
        let transitions = [
            (OrderStatus::Pending, OrderStatus::Confirmed),
            (OrderStatus::Pending, OrderStatus::Cancelled),
            (OrderStatus::Confirmed, OrderStatus::Preparing),
            (OrderStatus::Confirmed, OrderStatus::Cancelled),
            (OrderStatus::Preparing, OrderStatus::Ready),
            (OrderStatus::Ready, OrderStatus::Delivered),
        ];
        for (from, to) in transitions {
            assert!(from.can_transition_to(to), "{from} -> {to} must be valid");
            let mut order = order_in(from);
            order.change_status(to).unwrap();
            assert_eq!(order.status(), to);
        }
    }

    #[test]
    fn rejects_invalid_transitions_with_typed_error() {
        let cases = [
            (OrderStatus::Pending, OrderStatus::Preparing),
            (OrderStatus::Confirmed, OrderStatus::Ready),
            (OrderStatus::Ready, OrderStatus::Cancelled),
            (OrderStatus::Delivered, OrderStatus::Confirmed),
            (OrderStatus::Cancelled, OrderStatus::Pending),
        ];
        for (from, to) in cases {
            assert!(
                !from.can_transition_to(to),
                "{from} -> {to} must be invalid"
            );
            let mut order = order_in(from);
            let err = order.change_status(to).unwrap_err();
            assert_eq!(err, OrderError::InvalidStatusTransition { from, to });
        }
    }

    #[test]
    fn rejects_empty_orders() {
        let err = Order::new(
            OrderId::new("order_1"),
            UserId::new("student_1"),
            vec![],
            PickupCode::new("A7F2"),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap_err();
        assert_eq!(err, OrderError::EmptyOrder);
    }

    #[test]
    fn rejects_zero_quantities() {
        let err = Order::new(
            OrderId::new("order_1"),
            UserId::new("student_1"),
            vec![OrderItem::new(ProductId::new("p1"), "Coxinha", 500, 0)],
            PickupCode::new("A7F2"),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap_err();
        assert_eq!(err, OrderError::InvalidQuantity);
    }

    #[test]
    fn computes_total_in_minor_units() {
        let order = sample_order();
        assert_eq!(order.total(), 1000);
        assert_eq!(order.status(), OrderStatus::Pending);
        assert_eq!(order.pickup_code().as_str(), "A7F2");
    }

    #[test]
    fn rejects_order_total_overflow() {
        let err = Order::new(
            OrderId::new("order_overflow"),
            UserId::new("student_1"),
            vec![OrderItem::new(ProductId::new("p1"), "Coxinha", u64::MAX, 2)],
            PickupCode::new("A7F2"),
            SystemTime::UNIX_EPOCH,
        )
        .unwrap_err();

        assert_eq!(err, OrderError::TotalOverflow);
    }

    #[test]
    fn status_exposes_active_state() {
        assert!(OrderStatus::Pending.is_active());
        assert!(OrderStatus::Ready.is_active());
        assert!(!OrderStatus::Delivered.is_active());
        assert!(!OrderStatus::Cancelled.is_active());
    }
}
