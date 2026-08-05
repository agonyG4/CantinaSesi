//! HTTP client used by the CLI against the running cantina-api.

use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Health payload returned by `GET /api/v1/health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

/// Error surfaced by the CLI.
#[derive(Debug, Error)]
pub enum CliError {
    /// The API could not be reached (DNS, connection refused, timeout).
    #[error("unable to reach the API at {url}: {source}")]
    Transport {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    /// The API responded with an unparseable body.
    #[error("invalid response from the API: {source}")]
    ResponseDecode {
        #[source]
        source: reqwest::Error,
    },

    /// The API responded with an error status.
    #[error("the API returned an error status: HTTP {status}")]
    Api { status: u16 },

    /// A command that has not been implemented yet.
    #[error("command not implemented yet: {command}")]
    NotImplemented { command: &'static str },
}

/// Minimal HTTP client bound to one API base URL.
pub struct ApiClient {
    base_url: String,
}

impl ApiClient {
    /// Creates a client for the given base URL.
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }

    /// Calls `GET /api/v1/health`.
    pub async fn health(&self) -> Result<HealthResponse, CliError> {
        let url = format!("{}/api/v1/health", self.base_url);
        let response = reqwest::get(&url)
            .await
            .map_err(|source| CliError::Transport {
                url: url.clone(),
                source,
            })?;

        let status = response.status();
        let body: HealthResponse = response
            .json()
            .await
            .map_err(|source| CliError::ResponseDecode { source })?;

        if status == StatusCode::OK {
            Ok(body)
        } else {
            Err(CliError::Api {
                status: status.as_u16(),
            })
        }
    }
}
