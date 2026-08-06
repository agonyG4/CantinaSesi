//! HTTP client used by the CLI against the running cantina-api.
//!
//! The client is deliberately thin: it performs requests against a single
//! base URL and converts both network failures and API error responses into
//! typed [`CliError`] values so commands can render them consistently.

use reqwest::StatusCode;
use reqwest::Url;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Request timeout for every API call.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Health payload returned by `GET /api/v1/health`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
}

/// HTTP representation of a product, mirroring the API DTO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductDto {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Price in minor units.
    pub price: u64,
    pub category_id: String,
    pub available: bool,
}

/// Envelope for product collection responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductListResponse {
    pub items: Vec<ProductDto>,
}

/// HTTP representation of a category, mirroring the API DTO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryDto {
    pub id: String,
    pub name: String,
}

/// Envelope for category collection responses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CategoryListResponse {
    pub items: Vec<CategoryDto>,
}

/// HTTP representation of an authenticated user, mirroring the API DTO.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserDto {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub status: String,
}

/// Authentication payload returned by `login` and `refresh`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthenticationResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_at_epoch_seconds: u64,
    pub user: UserDto,
}

/// Query filters sent to `GET /api/v1/products`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductsQuery {
    pub category_id: Option<String>,
    pub search: Option<String>,
    pub include_unavailable: bool,
}

impl ProductsQuery {
    fn pairs(&self) -> Vec<(&'static str, String)> {
        let mut pairs = Vec::new();
        if let Some(category_id) = &self.category_id {
            pairs.push(("category_id", category_id.clone()));
        }
        if let Some(search) = &self.search {
            pairs.push(("search", search.clone()));
        }
        if self.include_unavailable {
            pairs.push(("include_unavailable", "true".to_string()));
        }
        pairs
    }
}

/// Error envelope returned by the API on non-success responses.
#[derive(Debug, Deserialize)]
struct ApiErrorEnvelope {
    error: ApiErrorBody,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    code: String,
    message: String,
    #[serde(default)]
    request_id: Option<String>,
}

/// Error surfaced by the CLI.
#[derive(Debug, Error)]
pub enum CliError {
    /// The configured base URL is not a valid HTTP(S) URL.
    #[error("invalid API URL '{url}': {message}")]
    InvalidApiUrl { url: String, message: String },

    /// The API could not be reached (DNS, connection refused, TLS).
    #[error("unable to reach the API at {url}: {source}")]
    Transport {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    /// The request exceeded the configured timeout.
    #[error("the API at {url} did not respond within {timeout} seconds")]
    Timeout { url: String, timeout: u64 },

    /// The API responded with a structured error.
    #[error("the API returned HTTP {status}: {message}")]
    ApiErrorResponse {
        status: u16,
        code: String,
        message: String,
        request_id: Option<String>,
    },

    /// The API responded with a non-success status whose body was not a
    /// structured error envelope.
    #[error("the API returned HTTP {status}: {reason}")]
    MalformedErrorResponse { status: u16, reason: String },

    /// The error response body could not be read from the wire.
    #[error("failed to read the API response body from {url}: {source}")]
    ResponseBodyRead {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    /// The API returned a successful status but an unparseable body.
    #[error("invalid response from the API: {source}")]
    ResponseDecode {
        #[source]
        source: serde_json::Error,
    },

    /// The API returned a successful status without any content.
    #[error("the API returned HTTP {status} without a response body")]
    MalformedSuccess { status: u16 },

    /// A payload could not be rendered in the requested output format.
    #[error("failed to render output: {source}")]
    OutputSerialization {
        #[source]
        source: serde_json::Error,
    },

    /// An authenticated command was run without a token.
    #[error("authentication required: pass --token or set CANTINA_ACCESS_TOKEN")]
    MissingAccessToken,

    /// The password prompt or stdin read failed.
    #[error("failed to read the password: {source}")]
    PasswordRead {
        #[source]
        source: std::io::Error,
    },

    /// The password was empty.
    #[error("a password is required")]
    EmptyPassword,

    /// A command that has not been implemented yet.
    #[error("command not implemented yet: {command}")]
    NotImplemented { command: &'static str },
}

/// HTTP client bound to one API base URL.
pub struct ApiClient {
    base_url: Url,
    client: reqwest::Client,
}

impl ApiClient {
    /// Creates a client for the given base URL, failing on invalid URLs.
    pub fn new(base_url: &str) -> Result<Self, CliError> {
        let parsed = Url::parse(base_url).map_err(|err| CliError::InvalidApiUrl {
            url: base_url.to_string(),
            message: err.to_string(),
        })?;
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|source| CliError::Transport {
                url: base_url.to_string(),
                source,
            })?;
        Ok(Self {
            base_url: parsed,
            client,
        })
    }

    /// Calls `GET /api/v1/health`.
    pub async fn health(&self) -> Result<HealthResponse, CliError> {
        self.get_json("/api/v1/health", &[]).await
    }

    /// Calls `GET /api/v1/products` with the given filters.
    pub async fn list_products(
        &self,
        query: &ProductsQuery,
    ) -> Result<ProductListResponse, CliError> {
        self.get_json("/api/v1/products", &query.pairs()).await
    }

    /// Calls `GET /api/v1/products/{id}`.
    pub async fn get_product(&self, id: &str) -> Result<ProductDto, CliError> {
        self.get_json(&format!("/api/v1/products/{id}"), &[]).await
    }

    /// Calls `GET /api/v1/categories`.
    pub async fn list_categories(&self) -> Result<CategoryListResponse, CliError> {
        self.get_json("/api/v1/categories", &[]).await
    }

    /// Calls `POST /api/v1/auth/login` with the given credentials.
    pub async fn login(
        &self,
        email: &str,
        password: &str,
    ) -> Result<AuthenticationResponse, CliError> {
        let body = serde_json::json!({ "email": email, "password": password });
        self.post_json("/api/v1/auth/login", &body, None).await
    }

    /// Calls `GET /api/v1/auth/me` with the given access token.
    pub async fn me(&self, token: &str) -> Result<UserDto, CliError> {
        self.get_json_with_token("/api/v1/auth/me", &[], Some(token))
            .await
    }

    /// Calls `POST /api/v1/auth/refresh` with the given access token.
    pub async fn refresh(&self, token: &str) -> Result<AuthenticationResponse, CliError> {
        self.post_json("/api/v1/auth/refresh", &serde_json::json!({}), Some(token))
            .await
    }

    /// Calls `POST /api/v1/auth/logout` with the given access token.
    pub async fn logout(&self, token: &str) -> Result<(), CliError> {
        let url = self
            .base_url
            .join("/api/v1/auth/logout")
            .map_err(|err| CliError::InvalidApiUrl {
                url: self.base_url.to_string(),
                message: err.to_string(),
            })?;
        let url_string = url.to_string();

        let response = self
            .client
            .post(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|source| {
                if source.is_timeout() {
                    CliError::Timeout {
                        url: url_string.clone(),
                        timeout: REQUEST_TIMEOUT.as_secs(),
                    }
                } else {
                    CliError::Transport {
                        url: url_string.clone(),
                        source,
                    }
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.error_from_response(status, response).await?);
        }
        Ok(())
    }

    /// Performs a GET request, joins the path onto the base URL and decodes
    /// the response into `T`, translating API errors into [`CliError`].
    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T, CliError> {
        self.get_json_with_token(path, query, None).await
    }

    /// Like [`Self::get_json`] but attaches a bearer token.
    async fn get_json_with_token<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
        token: Option<&str>,
    ) -> Result<T, CliError> {
        let url_string = self
            .build_url(path, query)?
            .to_string();
        let mut request = self.client.get(&url_string);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        self.send_json(request, url_string).await
    }

    /// Performs a POST request with an optional JSON body and bearer token.
    async fn post_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
        token: Option<&str>,
    ) -> Result<T, CliError> {
        let url_string = self
            .build_url(path, &[])?
            .to_string();
        let mut request = self.client.post(&url_string).json(body);
        if let Some(token) = token {
            request = request.bearer_auth(token);
        }
        self.send_json(request, url_string).await
    }

    /// Joins a path onto the base URL and appends the query pairs.
    fn build_url(&self, path: &str, query: &[(&str, String)]) -> Result<Url, CliError> {
        let mut url = self
            .base_url
            .join(path)
            .map_err(|err| CliError::InvalidApiUrl {
                url: self.base_url.to_string(),
                message: err.to_string(),
            })?;
        if !query.is_empty() {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                pairs.append_pair(key, value);
            }
            drop(pairs);
        }
        Ok(url)
    }

    /// Sends a prepared request and decodes the JSON body.
    async fn send_json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        url_string: String,
    ) -> Result<T, CliError> {
        let response = request.send().await.map_err(|source| {
            if source.is_timeout() {
                CliError::Timeout {
                    url: url_string.clone(),
                    timeout: REQUEST_TIMEOUT.as_secs(),
                }
            } else {
                CliError::Transport {
                    url: url_string.clone(),
                    source,
                }
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            return Err(self.error_from_response(status, response).await?);
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|source| CliError::Transport {
                url: url_string,
                source,
            })?;
        if bytes.is_empty() {
            return Err(CliError::MalformedSuccess {
                status: status.as_u16(),
            });
        }
        serde_json::from_slice(&bytes).map_err(|source| CliError::ResponseDecode { source })
    }

    async fn error_from_response(
        &self,
        status: StatusCode,
        response: reqwest::Response,
    ) -> Result<CliError, CliError> {
        let status_code = status.as_u16();
        let bytes = response
            .bytes()
            .await
            .map_err(|source| CliError::ResponseBodyRead {
                url: self.base_url.to_string(),
                source,
            })?;
        if let Ok(envelope) = serde_json::from_slice::<ApiErrorEnvelope>(&bytes) {
            Ok(CliError::ApiErrorResponse {
                status: status_code,
                code: envelope.error.code,
                message: envelope.error.message,
                request_id: envelope.error.request_id,
            })
        } else {
            Ok(CliError::MalformedErrorResponse {
                status: status_code,
                reason: status
                    .canonical_reason()
                    .unwrap_or("the API returned an error")
                    .to_string(),
            })
        }
    }
}
