//! Output abstraction.
//!
//! Kept minimal on purpose: a single formatter for human-readable and JSON
//! output. Richer table rendering and JSON streams can be added later.

use serde::Serialize;

use crate::api::CliError;

/// Supported output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text.
    Human,
    /// Structured JSON.
    Json,
}

/// Serializes a payload in the requested format.
///
/// JSON serialization can fail, so rendering returns a typed error instead of
/// silently falling back to an empty payload.
pub fn render<T: Serialize>(
    payload: &T,
    human: impl FnOnce() -> String,
    format: OutputFormat,
) -> Result<String, CliError> {
    match format {
        OutputFormat::Human => Ok(human()),
        OutputFormat::Json => serde_json::to_string_pretty(payload)
            .map_err(|source| CliError::OutputSerialization { source }),
    }
}

/// Formats an integer amount in minor units as human-readable Brazilian Real.
///
/// Examples: `500` → `R$ 5,00`, `750` → `R$ 7,50`, `1250` → `R$ 12,50`.
pub fn format_brl(minor_units: u64) -> String {
    let reais = minor_units / 100;
    let centavos = minor_units % 100;
    format!("R$ {reais},{centavos:02}")
}

#[cfg(test)]
mod tests {
    use super::format_brl;

    #[test]
    fn formats_whole_reais() {
        assert_eq!(format_brl(500), "R$ 5,00");
    }

    #[test]
    fn formats_with_centavos() {
        assert_eq!(format_brl(750), "R$ 7,50");
    }

    #[test]
    fn formats_multi_digit_values() {
        assert_eq!(format_brl(1250), "R$ 12,50");
    }

    #[test]
    fn formats_zero() {
        assert_eq!(format_brl(0), "R$ 0,00");
    }
}
