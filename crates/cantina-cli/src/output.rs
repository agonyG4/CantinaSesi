//! Output abstraction.
//!
//! Kept minimal on purpose: a single formatter for human-readable and JSON
//! output. Richer table rendering and JSON streams can be added later.

use serde::Serialize;

/// Supported output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text.
    Human,
    /// Structured JSON.
    Json,
}

/// Serializes a payload in the requested format.
pub fn render<T: Serialize>(
    payload: &T,
    human: impl FnOnce() -> String,
    format: OutputFormat,
) -> String {
    match format {
        OutputFormat::Human => human(),
        OutputFormat::Json => {
            serde_json::to_string_pretty(payload).unwrap_or_else(|_| "{}".to_string())
        }
    }
}
