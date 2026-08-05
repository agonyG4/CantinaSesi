//! Identifier and pickup-code generators implementing the application ports.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use cantina_application::ports::{IdGenerator, PickupCodeGenerator};
use cantina_core::catalog::{CategoryId, ProductId};
use cantina_core::orders::{OrderId, PickupCode};
use cantina_core::users::UserId;

/// Generates opaque string identifiers like `order_1710000000000_3`.
///
/// The format is not part of the public contract and may change. Consumers
/// must treat identifiers as opaque.
pub struct Generator {
    epoch_ms: u64,
    counter: AtomicU64,
}

impl Generator {
    /// Creates a generator seeded from the current wall clock.
    pub fn new() -> Self {
        let epoch_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis() as u64);
        Self {
            epoch_ms,
            counter: AtomicU64::new(0),
        }
    }

    fn next(&self, prefix: &str) -> String {
        let sequence = self.counter.fetch_add(1, Ordering::Relaxed);
        format!("{prefix}_{}_{sequence}", self.epoch_ms)
    }
}

impl Default for Generator {
    fn default() -> Self {
        Self::new()
    }
}

impl IdGenerator for Generator {
    fn new_user_id(&self) -> UserId {
        UserId::new(self.next("user"))
    }

    fn new_product_id(&self) -> ProductId {
        ProductId::new(self.next("product"))
    }

    fn new_category_id(&self) -> CategoryId {
        CategoryId::new(self.next("category"))
    }

    fn new_order_id(&self) -> OrderId {
        OrderId::new(self.next("order"))
    }
}

/// Alphabet that avoids ambiguous characters (no `I`, `O`, `0`, `1`).
const PICKUP_ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
const PICKUP_LENGTH: usize = 4;

/// Generates deterministic 4-character pickup codes like `A7F2`.
pub struct DefaultPickupCodeGenerator {
    counter: AtomicU64,
}

impl DefaultPickupCodeGenerator {
    /// Creates a pickup-code generator.
    pub fn new() -> Self {
        Self {
            counter: AtomicU64::new(0),
        }
    }

    fn generate_code(&self) -> String {
        let mut value = self.counter.fetch_add(1, Ordering::Relaxed);
        let mut code = String::with_capacity(PICKUP_LENGTH);
        for _ in 0..PICKUP_LENGTH {
            let index = (value % PICKUP_ALPHABET.len() as u64) as usize;
            code.push(char::from(PICKUP_ALPHABET[index]));
            value /= PICKUP_ALPHABET.len() as u64;
        }
        code
    }
}

impl Default for DefaultPickupCodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl PickupCodeGenerator for DefaultPickupCodeGenerator {
    fn generate(&self) -> PickupCode {
        PickupCode::new(self.generate_code())
    }
}

#[cfg(test)]
mod tests {
    use cantina_application::ports::{IdGenerator, PickupCodeGenerator};

    use super::{DefaultPickupCodeGenerator, Generator};

    #[test]
    fn generates_prefixed_order_ids() {
        let generator = Generator::new();
        let id = generator.new_order_id();
        assert!(id.as_str().starts_with("order_"));
        assert_ne!(generator.new_order_id(), generator.new_order_id());
    }

    #[test]
    fn pickup_codes_are_four_characters() {
        let generator = DefaultPickupCodeGenerator::new();
        for _ in 0..100 {
            let code = generator.generate();
            assert_eq!(code.as_str().len(), 4);
        }
    }
}
