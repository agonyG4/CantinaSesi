//! Security adapters implementing the application security ports.
//!
//! * `passwords`: Argon2id password hashing;
//! * `tokens`: opaque session tokens and SHA-256 fingerprints.

pub mod passwords;
pub mod tokens;
