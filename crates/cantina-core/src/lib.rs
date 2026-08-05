//! CantinaSesi domain core.
//!
//! Pure business concepts for the cafeteria platform. This crate must stay
//! independent from every framework and I/O concern:
//!
//! * no HTTP (Axum, Reqwest);
//! * no async runtime (Tokio);
//! * no CLI framework (Clap);
//! * no database drivers or SQL;
//! * no JSON response structures;
//! * no environment variables or filesystem configuration;
//! * no UI concepts.
//!
//! The crate has zero external dependencies. Every adapter (HTTP, storage,
//! CLI) is defined in another crate and depends on this one, never the
//! reverse.

pub mod auth;
pub mod catalog;
pub mod orders;
pub mod users;
