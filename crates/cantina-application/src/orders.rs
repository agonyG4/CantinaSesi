//! Order use cases: `CreateOrder`, `GetOrder`, `ListStudentOrders`,
//! `ListCanteenOrders`, `ChangeOrderStatus`.

use std::sync::Arc;

use cantina_core::catalog::ProductId;
use cantina_core::orders::{Order, OrderId, OrderItem, OrderStatus, PickupCode};
use cantina_core::users::UserId;

use crate::error::ApplicationError;
use crate::ports::{Clock, IdGenerator, OrderRepository, PickupCodeGenerator, ProductRepository};

/// Input line of a [`CreateOrderCommand`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderItemInput {
    pub product_id: ProductId,
    pub quantity: u32,
}

/// Command for the `CreateOrder` use case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateOrderCommand {
    pub student_id: UserId,
    pub items: Vec<OrderItemInput>,
}

/// Application representation of an [`OrderItem`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderItemDto {
    pub product_id: ProductId,
    pub product_name: String,
    pub unit_price: u64,
    pub quantity: u32,
}

impl From<&OrderItem> for OrderItemDto {
    fn from(item: &OrderItem) -> Self {
        Self {
            product_id: item.product_id.clone(),
            product_name: item.product_name.clone(),
            unit_price: item.unit_price,
            quantity: item.quantity,
        }
    }
}

/// Application representation of an [`Order`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderDto {
    pub id: OrderId,
    pub student_id: UserId,
    pub status: OrderStatus,
    /// Total in minor units.
    pub total: u64,
    pub pickup_code: PickupCode,
    pub items: Vec<OrderItemDto>,
}

impl From<&Order> for OrderDto {
    fn from(order: &Order) -> Self {
        Self {
            id: order.id().clone(),
            student_id: order.student_id().clone(),
            status: order.status(),
            total: order.total(),
            pickup_code: order.pickup_code().clone(),
            items: order.items().iter().map(OrderItemDto::from).collect(),
        }
    }
}

impl From<Order> for OrderDto {
    fn from(order: Order) -> Self {
        Self::from(&order)
    }
}

/// Coordinates order use cases.
pub struct OrderService {
    orders: Arc<dyn OrderRepository>,
    products: Arc<dyn ProductRepository>,
    ids: Arc<dyn IdGenerator>,
    pickup_codes: Arc<dyn PickupCodeGenerator>,
    clock: Arc<dyn Clock>,
}

impl OrderService {
    /// Creates the service with its repositories and generators.
    pub fn new(
        orders: Arc<dyn OrderRepository>,
        products: Arc<dyn ProductRepository>,
        ids: Arc<dyn IdGenerator>,
        pickup_codes: Arc<dyn PickupCodeGenerator>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            orders,
            products,
            ids,
            pickup_codes,
            clock,
        }
    }

    /// `CreateOrder` use case.
    ///
    /// Validates the command, resolves products against the catalog and
    /// persists a new order in `Pending` status.
    pub async fn create_order(
        &self,
        command: CreateOrderCommand,
    ) -> Result<OrderDto, ApplicationError> {
        if command.items.is_empty() {
            return Err(ApplicationError::EmptyOrder);
        }

        let mut items = Vec::with_capacity(command.items.len());
        for item in command.items {
            if item.quantity == 0 {
                return Err(ApplicationError::InvalidQuantity);
            }
            let product = self
                .products
                .find_product_by_id(&item.product_id)
                .await?
                .filter(|product| product.is_available())
                .ok_or_else(|| ApplicationError::ProductUnavailable {
                    product_id: item.product_id.to_string(),
                })?;
            items.push(OrderItem::new(
                product.id().clone(),
                product.name().to_string(),
                product.price(),
                item.quantity,
            ));
        }

        let order = Order::new(
            self.ids.new_order_id(),
            command.student_id,
            items,
            self.pickup_codes.generate(),
            self.clock.now(),
        )?;
        self.orders.save_order(&order).await?;
        Ok(order.into())
    }

    /// `GetOrder` use case.
    pub async fn get_order(&self, id: OrderId) -> Result<OrderDto, ApplicationError> {
        let order =
            self.orders
                .find_order_by_id(&id)
                .await?
                .ok_or_else(|| ApplicationError::NotFound {
                    resource: "order",
                    id: id.to_string(),
                })?;
        Ok(order.into())
    }

    /// `ListStudentOrders` use case.
    pub async fn list_student_orders(
        &self,
        student_id: UserId,
    ) -> Result<Vec<OrderDto>, ApplicationError> {
        let orders = self.orders.list_orders_for_student(&student_id).await?;
        Ok(orders.into_iter().map(Into::into).collect())
    }

    /// `ListCanteenOrders` use case.
    pub async fn list_canteen_orders(&self) -> Result<Vec<OrderDto>, ApplicationError> {
        let orders = self.orders.list_active_canteen_orders().await?;
        Ok(orders.into_iter().map(Into::into).collect())
    }

    /// `ChangeOrderStatus` use case.
    pub async fn change_status(
        &self,
        id: OrderId,
        target: OrderStatus,
    ) -> Result<OrderDto, ApplicationError> {
        let mut order =
            self.orders
                .find_order_by_id(&id)
                .await?
                .ok_or_else(|| ApplicationError::NotFound {
                    resource: "order",
                    id: id.to_string(),
                })?;
        order.change_status(target)?;
        self.orders.save_order(&order).await?;
        Ok(order.into())
    }
}
