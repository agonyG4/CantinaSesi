//! Authentication endpoints.
//!
//! `POST /login` and `POST /refresh` are public; `POST /logout` and
//! `GET /me` require a valid bearer access token. The `AuthenticatedPrincipal`
//! extractor translates the `Authorization: Bearer <token>` header into an
//! application `Principal`, enforcing every server-side session rule.
//!
//! Secrets are handled carefully here:
//! * the plaintext bearer token exists only transiently inside the extractor
//!   and is never logged or stored;
//! * the password is deserialized into a `SecretString` and never echoed;
//! * access tokens are returned exactly once and never appear in error bodies
//!   or logs.

use axum::Json;
use axum::Router;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequestParts, State};
use axum::http::header;
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use cantina_application::auth::AuthenticatedSession as AppSession;
use cantina_application::auth::{AuthenticateUserCommand, AuthenticationResult};
use cantina_application::users::UserDto;
use cantina_core::auth::Principal;
use cantina_core::users::UserRole;

use crate::error::{ApiError, ErrorResponse, extract_request_id_from_parts};
use crate::state::AppState;

/// Login credentials.
#[derive(Debug, Deserialize, ToSchema)]
pub struct LoginRequest {
    /// E-mail address of the account.
    pub email: String,
    /// Plaintext password. Never logged or echoed.
    #[schema(value_type = String)]
    pub password: SecretString,
}

/// Successful authentication payload returned by login and refresh.
///
/// The plaintext access token is serialized in the HTTP/JSON response exactly
/// once, but `Debug` never reveals it.
#[derive(Serialize, ToSchema)]
pub struct AuthenticationResponse {
    /// The opaque bearer token. Returned exactly once.
    pub access_token: String,
    /// Token type, always `Bearer`.
    pub token_type: &'static str,
    /// Epoch seconds at which the session absolutely expires.
    pub expires_at_epoch_seconds: u64,
    /// The authenticated user.
    pub user: UserResponse,
}

impl std::fmt::Debug for AuthenticationResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthenticationResponse")
            .field("access_token", &"[REDACTED]")
            .field("token_type", &self.token_type)
            .field("expires_at_epoch_seconds", &self.expires_at_epoch_seconds)
            .field("user", &self.user)
            .finish()
    }
}

impl From<AuthenticationResult> for AuthenticationResponse {
    fn from(result: AuthenticationResult) -> Self {
        Self {
            access_token: result.access_token.expose_secret().to_string(),
            token_type: "Bearer",
            expires_at_epoch_seconds: result.expires_at_epoch_seconds,
            user: result.user.into(),
        }
    }
}

/// HTTP representation of the authenticated user.
#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    id: String,
    name: String,
    email: String,
    role: String,
    status: String,
}

impl From<UserDto> for UserResponse {
    fn from(dto: UserDto) -> Self {
        Self {
            id: dto.id.to_string(),
            name: dto.name,
            email: dto.email,
            role: dto.role.to_string(),
            status: dto.status.to_string(),
        }
    }
}

/// Authenticated request context extracted from the `Authorization` header.
///
/// Authenticates the bearer token exactly once and exposes the resolved
/// [`AppSession`] (principal plus opaque session fingerprint) so downstream
/// handlers never re-parse or re-authenticate the raw token.
///
/// Rejections are stable `ApiError`s: `AUTHENTICATION_REQUIRED` when no valid
/// bearer token is presented, `INVALID_ACCESS_TOKEN` / `SESSION_EXPIRED` for
/// unknown/revoked/expired sessions and `ACCOUNT_INACTIVE` for disabled
/// accounts. Authentication failures always carry `WWW-Authenticate: Bearer`.
pub struct AuthenticatedSession(pub AppSession);

impl FromRequestParts<AppState> for AuthenticatedSession {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let request_id = extract_request_id_from_parts(parts);
        let token =
            bearer_token(&parts.headers).map_err(|err| err.with_request_id(request_id.clone()))?;
        let session = state
            .auth
            .authenticate_session(&token)
            .await
            .map_err(|err| ApiError::from(err).with_request_id(request_id))?;
        Ok(Self(session))
    }
}

/// Parses the bearer access token from the `Authorization` header.
///
/// The scheme is matched case-insensitively. A missing header, malformed
/// header or blank token is an `AUTHENTICATION_REQUIRED` challenge; the token
/// itself is never logged or stored.
fn bearer_token(headers: &HeaderMap) -> Result<SecretString, ApiError> {
    let header = headers.get(header::AUTHORIZATION).ok_or_else(|| {
        ApiError::from(cantina_application::ApplicationError::AuthenticationRequired)
    })?;
    let value = header.to_str().map_err(|_| {
        ApiError::from(cantina_application::ApplicationError::AuthenticationRequired)
    })?;

    let (scheme, token) = value.split_once(' ').ok_or_else(|| {
        ApiError::from(cantina_application::ApplicationError::AuthenticationRequired)
    })?;

    if !scheme.eq_ignore_ascii_case("bearer") {
        return Err(ApiError::from(
            cantina_application::ApplicationError::AuthenticationRequired,
        ));
    }
    let token = token.trim();
    if token.is_empty() {
        return Err(ApiError::from(
            cantina_application::ApplicationError::AuthenticationRequired,
        ));
    }
    Ok(SecretString::from(token.to_string()))
}

/// Request identifier extracted from the extensions set by the middleware.
pub struct RequestId(Option<String>);

impl FromRequestParts<AppState> for RequestId {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self(extract_request_id_from_parts(parts)))
    }
}

/// `POST /api/v1/auth/login`
#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Authentication succeeded", body = AuthenticationResponse),
        (status = 400, description = "Invalid request body", body = ErrorResponse),
        (status = 401, description = "Invalid credentials", body = ErrorResponse),
        (status = 403, description = "Account inactive", body = ErrorResponse),
    ),
)]
async fn login(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    payload: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(payload) = payload
        .map_err(|err| {
            tracing::debug!(detail = %err, "login body rejected");
            ApiError::invalid_request_body()
        })
        .map_err(|err| err.with_request_id(request_id.clone()))?;

    if payload.email.trim().is_empty() || payload.password.expose_secret().is_empty() {
        return Err(ApiError::invalid_request_body().with_request_id(request_id));
    }

    let result = state
        .auth
        .login(AuthenticateUserCommand {
            email: payload.email,
            password: payload.password,
        })
        .await
        .map_err(|err| ApiError::from(err).with_request_id(request_id))?;

    Ok(authentication_response(result))
}

/// `POST /api/v1/auth/refresh`
#[utoipa::path(
    post,
    path = "/api/v1/auth/refresh",
    security(("auth_bearer" = [])),
    responses(
        (status = 200, description = "A new session was issued", body = AuthenticationResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "Account inactive", body = ErrorResponse),
    ),
)]
async fn refresh(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
) -> Result<Response, ApiError> {
    let result = state
        .auth
        .refresh_authenticated_session(&context)
        .await
        .map_err(|err| ApiError::from(err).with_request_id(request_id))?;
    Ok(authentication_response(result))
}

/// `POST /api/v1/auth/logout`
#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    security(("auth_bearer" = [])),
    responses(
        (status = 204, description = "The session was revoked"),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "Account inactive", body = ErrorResponse),
    ),
)]
async fn logout(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
) -> Result<StatusCode, ApiError> {
    state
        .auth
        .logout_authenticated_session(&context)
        .await
        .map_err(|err| ApiError::from(err).with_request_id(request_id))?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /api/v1/auth/me`
#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    security(("auth_bearer" = [])),
    responses(
        (status = 200, description = "The authenticated user", body = UserResponse),
        (status = 401, description = "Authentication required or invalid token", body = ErrorResponse),
        (status = 403, description = "Account inactive", body = ErrorResponse),
    ),
)]
async fn me(
    State(state): State<AppState>,
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
) -> Result<Json<UserResponse>, ApiError> {
    let user = state
        .auth
        .get_current_user(&context.principal)
        .await
        .map_err(|err| ApiError::from(err).with_request_id(request_id))?;
    Ok(Json(user.into()))
}

/// Builds a login/refresh response with cache-busting headers.
fn authentication_response(result: AuthenticationResult) -> Response {
    let headers = [
        (header::CACHE_CONTROL, "no-store"),
        (header::PRAGMA, "no-cache"),
    ];
    (headers, Json(AuthenticationResponse::from(result))).into_response()
}

/// Prepares the `/api/v1/auth` router.
///
/// Request bodies are limited to 16 KiB; everything above is rejected with the
/// stable `INVALID_REQUEST_BODY` error.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024))
}

/// Test-only RBAC guard mirroring the application policy for HTTP handlers.
///
/// Used exclusively by integration tests to prove role enforcement over the
/// wire; production endpoints never rely on it.
pub fn require_any_role(principal: &Principal, allowed: &[UserRole]) -> Result<(), ApiError> {
    match cantina_application::auth::AuthorizationService::require_any_role(principal, allowed) {
        Ok(()) => Ok(()),
        Err(err) => Err(ApiError::from(err).with_bearer_challenge()),
    }
}

/// Test-only router exercising the role guards over the wire.
///
/// Kept outside the production router: only integration tests mount it.
pub fn test_only_router() -> Router<AppState> {
    Router::new()
        .route("/roles/canteen-admin", get(require_canteen_or_admin))
        .route("/roles/admin-only", get(require_admin_only))
}

async fn require_canteen_or_admin(
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
) -> Result<&'static str, ApiError> {
    require_any_role(&context.principal, &[UserRole::Canteen, UserRole::Admin])
        .map_err(|err| err.with_request_id(request_id))?;
    Ok("allowed")
}

async fn require_admin_only(
    RequestId(request_id): RequestId,
    AuthenticatedSession(context): AuthenticatedSession,
) -> Result<&'static str, ApiError> {
    require_any_role(&context.principal, &[UserRole::Admin])
        .map_err(|err| err.with_request_id(request_id))?;
    Ok("allowed")
}

#[cfg(test)]
mod tests {
    use super::{AuthenticationResponse, UserResponse, bearer_token};
    use secrecy::ExposeSecret;

    #[test]
    fn authentication_response_debug_redacts_the_token() {
        let response = AuthenticationResponse {
            access_token: "known-secret-token-789".to_string(),
            token_type: "Bearer",
            expires_at_epoch_seconds: 1893456000,
            user: UserResponse {
                id: "user_student_1".to_string(),
                name: "Ana Silva".to_string(),
                email: "ana@example.com".to_string(),
                role: "student".to_string(),
                status: "active".to_string(),
            },
        };
        let debug = format!("{response:?}");
        assert!(
            !debug.contains("known-secret-token-789"),
            "Debug must never leak the token: {debug}"
        );
        assert!(
            debug.contains("[REDACTED]"),
            "Debug must redact the token: {debug}"
        );
        let json = serde_json::to_string(&response).expect("serialize");
        assert!(
            json.contains("known-secret-token-789"),
            "serialization must still include the token"
        );
    }

    #[test]
    fn bearer_scheme_is_case_insensitive() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("BEARER abc"),
        );
        let token = bearer_token(&headers).expect("a mixed-case scheme must be accepted");
        assert_eq!(token.expose_secret(), "abc");
    }

    #[test]
    fn missing_bearer_header_is_a_challenge() {
        let headers = axum::http::HeaderMap::new();
        assert!(bearer_token(&headers).is_err());
    }

    #[test]
    fn blank_token_is_rejected() {
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer   "),
        );
        assert!(bearer_token(&headers).is_err());
    }
}
