//! Repository adapters implementing the application ports.

pub mod in_memory;

pub use in_memory::{
    InMemoryCategoryRepository, InMemoryOrderRepository, InMemoryProductRepository,
    InMemoryRepositories, InMemoryUserRepository,
};
