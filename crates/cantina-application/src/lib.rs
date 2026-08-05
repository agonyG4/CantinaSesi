//! Application layer: coordinates use cases.
//!
//! This crate orchestrates the domain (`cantina-core`) through use cases and
//! services. It defines:
//!
//! * application-facing commands, queries and DTOs;
//! * application-owned repository ports and service ports;
//! * typed application errors.
//!
//! It must not know about HTTP, storage, the CLI or any framework. It never
//! accesses environment variables. Storage implementations live in
//! `cantina-infrastructure` and adapt to the ports defined here.

pub mod auth;
pub mod catalog;
pub mod error;
pub mod orders;
pub mod ports;
pub mod users;

#[cfg(test)]
pub mod test_support;

#[cfg(test)]
mod order_tests;

pub use error::ApplicationError;
