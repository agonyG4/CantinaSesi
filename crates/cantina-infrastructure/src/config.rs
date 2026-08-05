//! Environment-driven configuration with sensible development defaults.

use std::env;

/// Runtime configuration for the backend.
#[derive(Debug, Clone)]
pub struct Config {
    bind_address: String,
    api_url: String,
    rust_log: String,
    cors_allowed_origins: Option<Vec<String>>,
}

impl Config {
    /// Loads the configuration from environment variables.
    ///
    /// Variables:
    /// * `CANTINA_BIND_ADDRESS` (default `127.0.0.1:8080`);
    /// * `CANTINA_API_URL` (default `http://127.0.0.1:8080`);
    /// * `RUST_LOG` (default `info`);
    /// * `CANTINA_CORS_ALLOWED_ORIGINS` (optional, comma-separated).
    pub fn from_env() -> Self {
        Self {
            bind_address: env_or("CANTINA_BIND_ADDRESS", "127.0.0.1:8080"),
            api_url: env_or("CANTINA_API_URL", "http://127.0.0.1:8080"),
            rust_log: env_or("RUST_LOG", "info"),
            cors_allowed_origins: parse_list(env::var("CANTINA_CORS_ALLOWED_ORIGINS").ok()),
        }
    }

    /// Development defaults, independent of the environment. Used by tests
    /// and tools that must not depend on the host environment.
    pub fn development() -> Self {
        Self {
            bind_address: "127.0.0.1:8080".to_string(),
            api_url: "http://127.0.0.1:8080".to_string(),
            rust_log: "info".to_string(),
            cors_allowed_origins: None,
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

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn development_config_uses_defaults() {
        let config = Config::development();
        assert_eq!(config.bind_address(), "127.0.0.1:8080");
        assert_eq!(config.api_url(), "http://127.0.0.1:8080");
        assert_eq!(config.rust_log(), "info");
        assert!(config.cors_allowed_origins().is_none());
    }
}
