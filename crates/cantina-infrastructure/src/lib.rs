//! Replaceable technical implementations.
//!
//! This crate holds everything that is easy to swap out without touching the
//! domain or the application layer:
//!
//! * `config`: environment-driven configuration with development defaults;
//! * `persistence`: repository adapters that implement the application
//!   ports (in-memory for now; a real database comes later);
//! * `security`: placeholder password/token implementations;
//! * `time`: a system clock implementing the application `Clock` port;
//! * `identifiers`: id and pickup-code generators.
//!
//! No business rules live here. This crate only adapts the application
//! ports to concrete technical choices.

pub mod config;
pub mod identifiers;
pub mod persistence;
pub mod security;
pub mod time;
