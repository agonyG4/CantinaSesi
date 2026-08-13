//! Order use cases: creation, ownership-aware reads, cafeteria queue access,
//! and domain-validated status changes.

use std::collections::HashMap;
use std::sync::Arc;

use cantina_core::auth::Principal;
use cantina_core::catalog::ProductId;
use cantina_core::orders::{
    MAX_ORDER_LINES, MAX_QUANTITY_PER_PRODUCT, MAX_TOTAL_QUANTITY, Order, OrderId, OrderItem,
    OrderStatus, PickupCode,
};
use cantina_core::users::{UserId, UserRole};

use crate::auth::AuthorizationService;
use crate::error::ApplicationError;
use crate::ports::{
    Clock, IdGenerator, OrderRepository, OrderStatusTransitionError, OrderWriteError,
    PickupCodeGenerator, ProductRepository,
};

/// Maximum number of pickup-code candidates attempted for one order.
pub const MAX_PICKUP_CODE_ATTEMPTS: usize = 5;

/// Input line of a [`CreateOrderCommand`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderItemInput {
    pub product_id: ProductId,
    pub quantity: u32,
}

/// Command for the `CreateOrder` use case.
///
/// Ownership is deliberately absent. The application derives it from the
/// authenticated principal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateOrderCommand {
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

    /// Creates an order for the authenticated student.
    pub async fn create_order(
        &self,
        principal: &Principal,
        command: CreateOrderCommand,
    ) -> Result<OrderDto, ApplicationError> {
        AuthorizationService::require_any_role(principal, &[UserRole::Student])?;
        let normalized = normalize_items(command.items)?;
        let product_ids: Vec<ProductId> = normalized
            .iter()
            .map(|(product_id, _)| product_id.clone())
            .collect();
        let products = self.products.find_products_by_ids(&product_ids).await?;
        let products_by_id: HashMap<ProductId, _> = products
            .into_iter()
            .map(|product| (product.id().clone(), product))
            .collect();

        let mut items = Vec::with_capacity(normalized.len());
        for (product_id, quantity) in normalized {
            let product = products_by_id
                .get(&product_id)
                .filter(|product| product.is_available())
                .ok_or_else(|| ApplicationError::ProductUnavailable {
                    product_id: product_id.to_string(),
                })?;
            items.push(OrderItem::new(
                product.id().clone(),
                product.name().to_string(),
                product.price(),
                quantity,
            ));
        }

        let order_id = self.ids.new_order_id();
        for attempt in 0..MAX_PICKUP_CODE_ATTEMPTS {
            let order = Order::new(
                order_id.clone(),
                principal.user_id().clone(),
                items.clone(),
                self.pickup_codes.generate()?,
                self.clock.now(),
            )?;
            match self.orders.save_new_order(&order).await {
                Ok(()) => return Ok(order.into()),
                Err(OrderWriteError::PickupCodeConflict)
                    if attempt + 1 < MAX_PICKUP_CODE_ATTEMPTS => {}
                Err(OrderWriteError::PickupCodeConflict) => {
                    return Err(ApplicationError::PickupCodeGenerationExhausted);
                }
                Err(err) => return Err(err.into()),
            }
        }

        Err(ApplicationError::PickupCodeGenerationExhausted)
    }

    /// Retrieves an order after applying role and ownership policy.
    pub async fn get_order(
        &self,
        principal: &Principal,
        id: OrderId,
    ) -> Result<OrderDto, ApplicationError> {
        AuthorizationService::require_authenticated(principal)?;
        let order = self
            .orders
            .find_order_by_id(&id)
            .await?
            .ok_or_else(|| order_not_found(&id))?;
        if principal.has_role(UserRole::Student) && order.student_id() != principal.user_id() {
            return Err(order_not_found(&id));
        }
        AuthorizationService::require_any_role(
            principal,
            &[UserRole::Student, UserRole::Canteen, UserRole::Admin],
        )?;
        Ok(order.into())
    }

    /// Retrieves an order through the student-facing access policy.
    pub async fn get_student_order(
        &self,
        principal: &Principal,
        id: OrderId,
    ) -> Result<OrderDto, ApplicationError> {
        AuthorizationService::require_any_role(principal, &[UserRole::Student])?;
        self.get_order(principal, id).await
    }

    /// Retrieves an order through the cafeteria operations access policy.
    pub async fn get_canteen_order(
        &self,
        principal: &Principal,
        id: OrderId,
    ) -> Result<OrderDto, ApplicationError> {
        AuthorizationService::require_any_role(principal, &[UserRole::Canteen, UserRole::Admin])?;
        self.get_order(principal, id).await
    }

    /// Lists only the authenticated student's orders.
    pub async fn list_student_orders(
        &self,
        principal: &Principal,
    ) -> Result<Vec<OrderDto>, ApplicationError> {
        AuthorizationService::require_any_role(principal, &[UserRole::Student])?;
        let orders = self
            .orders
            .list_orders_for_student(principal.user_id())
            .await?;
        Ok(orders.into_iter().map(Into::into).collect())
    }

    /// Lists active orders for cafeteria operations.
    pub async fn list_canteen_orders(
        &self,
        principal: &Principal,
    ) -> Result<Vec<OrderDto>, ApplicationError> {
        AuthorizationService::require_any_role(principal, &[UserRole::Canteen, UserRole::Admin])?;
        let orders = self.orders.list_active_canteen_orders().await?;
        Ok(orders.into_iter().map(Into::into).collect())
    }

    /// Changes an order status through the domain transition rules.
    pub async fn change_status(
        &self,
        principal: &Principal,
        id: OrderId,
        target: OrderStatus,
    ) -> Result<OrderDto, ApplicationError> {
        AuthorizationService::require_any_role(principal, &[UserRole::Canteen, UserRole::Admin])?;
        let order = self
            .orders
            .transition_order_status(&id, target)
            .await
            .map_err(|err| map_status_transition_error(&id, err))?;
        Ok(order.into())
    }
}

fn normalize_items(inputs: Vec<OrderItemInput>) -> Result<Vec<(ProductId, u32)>, ApplicationError> {
    if inputs.is_empty() {
        return Err(ApplicationError::EmptyOrder);
    }

    let mut normalized: Vec<(ProductId, u32)> = Vec::with_capacity(inputs.len());
    let mut total_quantity = 0_u32;
    for input in inputs {
        if input.quantity == 0 {
            return Err(ApplicationError::InvalidQuantity);
        }
        if input.quantity > MAX_QUANTITY_PER_PRODUCT {
            return Err(ApplicationError::QuantityLimitExceeded);
        }
        total_quantity = total_quantity
            .checked_add(input.quantity)
            .ok_or(ApplicationError::TotalQuantityLimitExceeded)?;
        if total_quantity > MAX_TOTAL_QUANTITY {
            return Err(ApplicationError::TotalQuantityLimitExceeded);
        }

        if let Some((_, quantity)) = normalized
            .iter_mut()
            .find(|(product_id, _)| *product_id == input.product_id)
        {
            *quantity = quantity
                .checked_add(input.quantity)
                .ok_or(ApplicationError::QuantityLimitExceeded)?;
            if *quantity > MAX_QUANTITY_PER_PRODUCT {
                return Err(ApplicationError::QuantityLimitExceeded);
            }
        } else {
            if normalized.len() >= MAX_ORDER_LINES {
                return Err(ApplicationError::TooManyOrderLines);
            }
            normalized.push((input.product_id, input.quantity));
        }
    }
    Ok(normalized)
}

fn order_not_found(id: &OrderId) -> ApplicationError {
    ApplicationError::NotFound {
        resource: "order",
        id: id.to_string(),
    }
}

fn map_status_transition_error(
    id: &OrderId,
    error: OrderStatusTransitionError,
) -> ApplicationError {
    match error {
        OrderStatusTransitionError::NotFound => order_not_found(id),
        OrderStatusTransitionError::InvalidStatusTransition { from, to } => {
            ApplicationError::InvalidStatusTransition {
                from: from.to_string(),
                to: to.to_string(),
            }
        }
        OrderStatusTransitionError::Internal(message) => ApplicationError::Storage(message),
    }
}
