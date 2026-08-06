//! Environment-driven configuration with sensible development defaults.

use std::env;
use std::time::Duration;

use cantina_application::auth::{SessionConfig, SessionConfigError};

/// Failure while parsing runtime configuration.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// A session-timeout variable was not a valid non-negative integer.
    #[error("invalid value for {var}: expected a non-negative integer, got {value:?}")]
    InvalidTimeout {
        /// The offending environment variable.
        var: String,
        /// The value that could not be parsed.
        value: String,
    },
    /// The session timeouts violated the documented constraints.
    #[error("invalid session timeout configuration: {0}")]
    InvalidSessionConfig(#[from] SessionConfigError),
}

/// Runtime configuration for the backend.
#[derive(Debug, Clone)]
pub struct Config {
    bind_address: String,
    api_url: String,
    rust_log: String,
    cors_allowed_origins: Option<Vec<String>>,
    session_config: SessionConfig,
}

impl Config {
    /// Loads the configuration from environment variables.
    ///
    /// Variables:
    /// * `CANTINA_BIND_ADDRESS` (default `127.0.0.1:8080`);
    /// * `CANTINA_API_URL` (default `http://127.0.0.1:8080`);
    /// * `RUST_LOG` (default `info`);
    /// * `CANTINA_CORS_ALLOWED_ORIGINS` (optional, comma-separated);
    /// * `CANTINA_SESSION_IDLE_TTL_SECONDS` (default `1800`);
    /// * `CANTINA_SESSION_ABSOLUTE_TTL_SECONDS` (default `28800`).
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            bind_address: env_or("CANTINA_BIND_ADDRESS", "127.0.0.1:8080"),
            api_url: env_or("CANTINA_API_URL", "http://127.0.0.1:8080"),
            rust_log: env_or("RUST_LOG", "info"),
            cors_allowed_origins: parse_list(env::var("CANTINA_CORS_ALLOWED_ORIGINS").ok()),
            session_config: session_config_from_env()?,
        })
    }

    /// Development defaults, independent of the environment. Used by tests
    /// and tools that must not depend on the host environment.
    pub fn development() -> Self {
        Self {
            bind_address: "127.0.0.1:8080".to_string(),
            api_url: "http://127.0.0.1:8080".to_string(),
            rust_log: "info".to_string(),
            cors_allowed_origins: None,
            session_config: SessionConfig::defaults(),
        }
    }

    /// Address the HTTP API binds to.
    pub fn bind_address(&self) -> &str {
        &self.bind_address
    }

    /// Base URL clients use to reach the API.
    pub fn api_url(&self) -> &str {
        &self.api_url
    }

    /// Log filter for the tracing subscriber.
    pub fn rust_log(&self) -> &str {
        &self.rust_log
    }

    /// Optional allow-list of CORS origins.
    pub fn cors_allowed_origins(&self) -> Option<&[String]> {
        self.cors_allowed_origins.as_deref()
    }

    /// Server-side session timeout configuration.
    pub fn session_config(&self) -> SessionConfig {
        self.session_config
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn parse_list(value: Option<String>) -> Option<Vec<String>> {
    let items = value?
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();
    if items.is_empty() { None } else { Some(items) }
}

fn session_config_from_env() -> Result<SessionConfig, ConfigError> {
    let idle = parse_seconds("CANTINA_SESSION_IDLE_TTL_SECONDS", SessionConfig::DEFAULT_IDLE_TIMEOUT)?;
    let absolute = parse_seconds(
        "CANTINA_SESSION_ABSOLUTE_TTL_SECONDS",
        SessionConfig::DEFAULT_ABSOLUTE_TIMEOUT,
    )?;
    build_session_config(idle, absolute)
}

fn build_session_config(idle: Duration, absolute: Duration) -> Result<SessionConfig, ConfigError> {
    SessionConfig::new(idle, absolute).map_err(ConfigError::InvalidSessionConfig)
}

fn parse_seconds(var: &str, default: Duration) -> Result<Duration, ConfigError> {
    match env::var(var) {
        Ok(value) => parse_seconds_value(var, &value),
        Err(_) => Ok(default),
    }
}

fn parse_seconds_value(var: &str, value: &str) -> Result<Duration, ConfigError> {
    let seconds: u64 = value
        .trim()
        .parse()
        .map_err(|_| ConfigError::InvalidTimeout {
            var: var.to_string(),
            value: value.to_string(),
        })?;
    Ok(Duration::from_secs(seconds))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use cantina_application::auth::{SessionConfig, SessionConfigError};

    use super::{build_session_config, session_config_from_env, Config, ConfigError};

    #[test]
    fn development_config_uses_defaults() {
        let config = Config::development();
        assert_eq!(config.bind_address(), "127.0.0.1:8080");
        assert_eq!(config.api_url(), "http://127.0.0.1:8080");
        assert_eq!(config.rust_log(), "info");
        assert!(config.cors_allowed_origins().is_none());
        assert_eq!(config.session_config(), SessionConfig::defaults());
    }

    #[test]
    fn session_config_defaults_are_applied_without_environment() {
        let config = session_config_from_env().expect("defaults must be valid");
        assert_eq!(config.idle_timeout(), Duration::from_secs(1800));
        assert_eq!(config.absolute_timeout(), Duration::from_secs(28800));
    }

    #[test]
    fn zero_idle_timeout_is_rejected() {
        assert_eq!(
            build_session_config(Duration::ZERO, Duration::from_secs(28800)),
            Err(ConfigError::InvalidSessionConfig(
                SessionConfigError::ZeroIdleTimeout
            ))
        );
    }

    #[test]
    fn zero_absolute_timeout_is_rejected() {
        assert_eq!(
            build_session_config(Duration::from_secs(1800), Duration::ZERO),
            Err(ConfigError::InvalidSessionConfig(
                SessionConfigError::ZeroAbsoluteTimeout
            ))
        );
    }

    #[test]
    fn idle_above_absolute_is_rejected() {
        assert_eq!(
            build_session_config(Duration::from_secs(28800), Duration::from_secs(1800)),
            Err(ConfigError::InvalidSessionConfig(
                SessionConfigError::IdleExceedsAbsolute
            ))
        );
    }

    #[test]
    fn valid_session_ttls_are_accepted() {
        let config = build_session_config(Duration::from_secs(1800), Duration::from_secs(28800))
            .expect("valid timeouts must be accepted");
        assert_eq!(config.idle_timeout(), Duration::from_secs(1800));
        assert_eq!(config.absolute_timeout(), Duration::from_secs(28800));
    }

    #[test]
    fn parse_seconds_value_rejects_non_numeric_values() {
        let result =
            super::parse_seconds_value("CANTINA_SESSION_IDLE_TTL_SECONDS", "not-a-number")
                .err()
                .expect("a non-numeric value must be rejected");
        assert!(matches!(
            result,
            ConfigError::InvalidTimeout { var, .. } if var == "CANTINA_SESSION_IDLE_TTL_SECONDS"
        ));
    }

    #[test]
    fn parse_seconds_value_accepts_a_valid_number() {
        let parsed = super::parse_seconds_value("CANTINA_SESSION_ABSOLUTE_TTL_SECONDS", "7200")
            .expect("a numeric value must parse");
        assert_eq!(parsed, Duration::from_secs(7200));
    }
}
