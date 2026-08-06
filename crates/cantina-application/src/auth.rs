//! Authentication and authorization use cases.
//!
//! `AuthService` implements login, access-token authentication, refresh,
//! logout and current-user lookup against the application-owned ports. No
//! transport concept (HTTP headers, bearer schemes) appears here; the HTTP
//! adapter in `cantina-api` translates between the wire format and these use
//! cases. `AuthorizationService` owns the role-based access policy.

use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cantina_core::auth::Principal;
use cantina_core::users::UserRole;
use secrecy::SecretString;

use crate::error::ApplicationError;
use crate::ports::{
    Clock, CredentialRepository, PasswordService, SessionRecord, SessionRepository,
    SessionTokenService, UserRepository,
};
use crate::users::UserDto;

/// Command for the `login` use case.
#[derive(Debug, Clone)]
pub struct AuthenticateUserCommand {
    /// User-supplied e-mail, normalized by the use case.
    pub email: String,
    /// User-supplied plain password.
    pub password: SecretString,
}

/// Successful authentication payload.
///
/// The access token is returned exactly once (on login or refresh); it is
/// never stored server-side in plaintext.
#[derive(Debug, Clone)]
pub struct AuthenticationResult {
    /// The opaque bearer token issued to the client.
    pub access_token: SecretString,
    /// Epoch seconds at which the session absolutely expires.
    pub expires_at_epoch_seconds: u64,
    /// The authenticated user.
    pub user: UserDto,
}

/// Opaque application-safe reference to a live server-side session.
///
/// Holds only the token fingerprint; it never contains (or logs) the
/// plaintext bearer token. The application uses it to rotate or revoke the
/// session without re-authenticating the raw token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFingerprint(String);

impl SessionFingerprint {
    /// The fingerprint string keying the server-side session record.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An authenticated request context returned once by a single authentication.
///
/// Produced exactly once per request by the `authenticate_session` use case so
/// downstream operations (refresh, logout, current-user lookup) never re-parse
/// or re-authenticate the raw access token.
#[derive(Debug, Clone)]
pub struct AuthenticatedSession {
    /// The resolved principal.
    pub principal: Principal,
    /// The fingerprint of the authenticated server-side session.
    pub session_fingerprint: SessionFingerprint,
}

/// Validated server-side session timeouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionConfig {
    idle_timeout: Duration,
    absolute_timeout: Duration,
}

/// Failure while building a [`SessionConfig`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionConfigError {
    /// The idle timeout must be greater than zero.
    ZeroIdleTimeout,
    /// The absolute timeout must be greater than zero.
    ZeroAbsoluteTimeout,
    /// The idle timeout must not exceed the absolute timeout.
    IdleExceedsAbsolute,
}

impl fmt::Display for SessionConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroIdleTimeout => write!(f, "session idle timeout must be greater than zero"),
            Self::ZeroAbsoluteTimeout => {
                write!(f, "session absolute timeout must be greater than zero")
            }
            Self::IdleExceedsAbsolute => write!(
                f,
                "session idle timeout must not exceed the absolute timeout"
            ),
        }
    }
}

impl Error for SessionConfigError {}

impl SessionConfig {
    /// Default idle timeout: 30 minutes.
    pub const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
    /// Default absolute timeout: 8 hours.
    pub const DEFAULT_ABSOLUTE_TIMEOUT: Duration = Duration::from_secs(8 * 60 * 60);

    /// Validates and builds a session configuration.
    pub fn new(
        idle_timeout: Duration,
        absolute_timeout: Duration,
    ) -> Result<Self, SessionConfigError> {
        if idle_timeout.is_zero() {
            return Err(SessionConfigError::ZeroIdleTimeout);
        }
        if absolute_timeout.is_zero() {
            return Err(SessionConfigError::ZeroAbsoluteTimeout);
        }
        if idle_timeout > absolute_timeout {
            return Err(SessionConfigError::IdleExceedsAbsolute);
        }
        Ok(Self {
            idle_timeout,
            absolute_timeout,
        })
    }

    /// The documented production defaults.
    pub fn defaults() -> Self {
        Self {
            idle_timeout: Self::DEFAULT_IDLE_TIMEOUT,
            absolute_timeout: Self::DEFAULT_ABSOLUTE_TIMEOUT,
        }
    }

    /// Idle timeout after which an unused session is invalid.
    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }

    /// Absolute lifetime of a session regardless of activity.
    pub fn absolute_timeout(&self) -> Duration {
        self.absolute_timeout
    }
}

/// Coordinates authentication use cases.
pub struct AuthService {
    users: Arc<dyn UserRepository>,
    credentials: Arc<dyn CredentialRepository>,
    sessions: Arc<dyn SessionRepository>,
    passwords: Arc<dyn PasswordService>,
    tokens: Arc<dyn SessionTokenService>,
    clock: Arc<dyn Clock>,
    session_config: SessionConfig,
}

impl AuthService {
    /// Creates the service with every dependency it owns.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        users: Arc<dyn UserRepository>,
        credentials: Arc<dyn CredentialRepository>,
        sessions: Arc<dyn SessionRepository>,
        passwords: Arc<dyn PasswordService>,
        tokens: Arc<dyn SessionTokenService>,
        clock: Arc<dyn Clock>,
        session_config: SessionConfig,
    ) -> Self {
        Self {
            users,
            credentials,
            sessions,
            passwords,
            tokens,
            clock,
            session_config,
        }
    }

    /// `login` use case.
    ///
    /// Normalizes the e-mail, verifies the password without revealing whether
    /// the account exists, rejects inactive accounts and issues a session.
    pub async fn login(
        &self,
        command: AuthenticateUserCommand,
    ) -> Result<AuthenticationResult, ApplicationError> {
        let email = normalize_email(&command.email);
        if email.is_empty() {
            return Err(ApplicationError::InvalidCredentials);
        }

        let user = self.users.find_user_by_email(&email).await?;

        let password_matches = match &user {
            Some(user) => {
                let stored = self
                    .credentials
                    .find_password_hash_by_user_id(user.id())
                    .await?;
                self.passwords
                    .verify_password(command.password.clone(), stored.as_ref())
                    .await
                    .map_err(|_| ApplicationError::PasswordProcessing)?
            }
            None => self
                .passwords
                .verify_password(command.password.clone(), None)
                .await
                .map_err(|_| ApplicationError::PasswordProcessing)?,
        };

        if !password_matches {
            return Err(ApplicationError::InvalidCredentials);
        }

        // Unknown e-mail and wrong password produce the identical public
        // error; a matching password alone does not authenticate an
        // unknown account.
        let user = user.ok_or(ApplicationError::InvalidCredentials)?;
        if !user.is_active() {
            return Err(ApplicationError::AccountInactive);
        }

        let access_token = self
            .tokens
            .generate_token()
            .map_err(|_| ApplicationError::TokenGeneration)?;
        let fingerprint = self
            .tokens
            .fingerprint(&access_token)
            .map_err(|_| ApplicationError::TokenGeneration)?;

        let now = self.clock.now();
        let absolute_expires_at = now
            .checked_add(self.session_config.absolute_timeout())
            .ok_or_else(|| ApplicationError::Storage("session timeout overflow".to_string()))?;

        let session = SessionRecord {
            token_fingerprint: fingerprint,
            user_id: user.id().clone(),
            created_at: now,
            last_seen_at: now,
            absolute_expires_at,
            revoked_at: None,
        };
        self.sessions.save_session(session).await?;

        Ok(AuthenticationResult {
            access_token,
            expires_at_epoch_seconds: epoch_seconds(absolute_expires_at),
            user: user.into(),
        })
    }

    /// Resolves an opaque bearer token to an authenticated [`Principal`].
    ///
    /// Enforces every server-side invalidation rule: unknown fingerprint,
    /// revoked session, absolute expiration, idle expiration, and a user that
    /// no longer exists or has become inactive. Successful activity refreshes
    /// `last_seen_at`.
    pub async fn authenticate_access_token(
        &self,
        token: &SecretString,
    ) -> Result<Principal, ApplicationError> {
        let fingerprint = self
            .tokens
            .fingerprint(token)
            .map_err(|_| ApplicationError::TokenGeneration)?;
        self.authenticate_with_fingerprint(&fingerprint).await
    }

    /// Authenticates an opaque bearer token exactly once and returns the
    /// resolved [`AuthenticatedSession`].
    ///
    /// This is the single authentication performed by the HTTP adapter per
    /// request. Downstream use cases (`refresh_authenticated_session`,
    /// `logout_authenticated_session`, `get_current_user`) consume the
    /// returned context and never re-authenticate the raw token.
    pub async fn authenticate_session(
        &self,
        token: &SecretString,
    ) -> Result<AuthenticatedSession, ApplicationError> {
        let fingerprint = self
            .tokens
            .fingerprint(token)
            .map_err(|_| ApplicationError::TokenGeneration)?;
        let principal = self.authenticate_with_fingerprint(&fingerprint).await?;
        Ok(AuthenticatedSession {
            principal,
            session_fingerprint: SessionFingerprint(fingerprint),
        })
    }

    /// Core authentication against an already-fingerprinted session.
    ///
    /// Enforces every server-side invalidation rule and refreshes
    /// `last_seen_at` on success.
    async fn authenticate_with_fingerprint(
        &self,
        fingerprint: &str,
    ) -> Result<Principal, ApplicationError> {
        let session = self
            .sessions
            .find_session_by_fingerprint(fingerprint)
            .await?
            .ok_or(ApplicationError::InvalidAccessToken)?;

        if session.revoked_at.is_some() {
            return Err(ApplicationError::InvalidAccessToken);
        }

        let now = self.clock.now();
        if session.absolute_expires_at <= now {
            return Err(ApplicationError::SessionExpired);
        }

        let idle_deadline = session
            .last_seen_at
            .checked_add(self.session_config.idle_timeout())
            .unwrap_or(session.absolute_expires_at);
        if idle_deadline <= now {
            return Err(ApplicationError::SessionExpired);
        }

        let user = self
            .users
            .find_user_by_id(&session.user_id)
            .await?
            .ok_or(ApplicationError::InvalidAccessToken)?;
        if !user.is_active() {
            return Err(ApplicationError::AccountInactive);
        }

        self.sessions.update_last_seen_at(fingerprint, now).await?;

        Ok(Principal::new(session.user_id, user.role()))
    }

    /// `refresh` use case: rotates the already-authenticated session into a
    /// brand new one.
    ///
    /// The old session is atomically revoked and the new session receives
    /// fresh idle and absolute expirations. The context was produced by
    /// [`Self::authenticate_session`], so this performs no duplicate
    /// authentication. A stale or already-used session fails with the stable
    /// public authentication error.
    pub async fn refresh_authenticated_session(
        &self,
        context: &AuthenticatedSession,
    ) -> Result<AuthenticationResult, ApplicationError> {
        let new_token = self
            .tokens
            .generate_token()
            .map_err(|_| ApplicationError::TokenGeneration)?;
        let new_fingerprint = self
            .tokens
            .fingerprint(&new_token)
            .map_err(|_| ApplicationError::TokenGeneration)?;

        let now = self.clock.now();
        let absolute_expires_at = now
            .checked_add(self.session_config.absolute_timeout())
            .ok_or_else(|| ApplicationError::Storage("session timeout overflow".to_string()))?;

        let new_session = SessionRecord {
            token_fingerprint: new_fingerprint,
            user_id: context.principal.user_id().clone(),
            created_at: now,
            last_seen_at: now,
            absolute_expires_at,
            revoked_at: None,
        };
        self.sessions
            .rotate_session(context.session_fingerprint.as_str(), new_session, now)
            .await?;

        let user = self
            .users
            .find_user_by_id(context.principal.user_id())
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "user",
                id: context.principal.user_id().to_string(),
            })?;

        Ok(AuthenticationResult {
            access_token: new_token,
            expires_at_epoch_seconds: epoch_seconds(absolute_expires_at),
            user: user.into(),
        })
    }

    /// `refresh` use case from a raw token, for callers without an
    /// already-authenticated context. Authenticates the token once and then
    /// rotates the resulting session.
    pub async fn refresh_access_token(
        &self,
        token: &SecretString,
    ) -> Result<AuthenticationResult, ApplicationError> {
        let context = self.authenticate_session(token).await?;
        self.refresh_authenticated_session(&context).await
    }

    /// `logout` use case: revokes the already-authenticated session.
    ///
    /// The context was produced by [`Self::authenticate_session`], so this
    /// performs no duplicate authentication.
    pub async fn logout_authenticated_session(
        &self,
        context: &AuthenticatedSession,
    ) -> Result<(), ApplicationError> {
        let now = self.clock.now();
        self.sessions
            .revoke_session(context.session_fingerprint.as_str(), now)
            .await?;
        Ok(())
    }

    /// `logout` use case from a raw token, for callers without an
    /// already-authenticated context. Authenticates the token once and then
    /// revokes the resulting session.
    pub async fn logout(&self, token: &SecretString) -> Result<(), ApplicationError> {
        let context = self.authenticate_session(token).await?;
        self.logout_authenticated_session(&context).await
    }

    /// `GetCurrentUser` use case: resolves a [`Principal`] to its [`User`].
    pub async fn get_current_user(
        &self,
        principal: &Principal,
    ) -> Result<UserDto, ApplicationError> {
        let user = self
            .users
            .find_user_by_id(principal.user_id())
            .await?
            .ok_or_else(|| ApplicationError::NotFound {
                resource: "user",
                id: principal.user_id().to_string(),
            })?;
        Ok(user.into())
    }
}

/// Application-level authorization policy.
///
/// Future application use cases remain the final authorization authority; the
/// HTTP layer only mirrors these decisions as a transport convenience.
pub struct AuthorizationService;

impl AuthorizationService {
    /// Requires an authenticated principal. A `Principal` is by construction
    /// authenticated, so this is a policy marker for use cases that must be
    /// reached only through the authenticated path.
    pub fn require_authenticated(principal: &Principal) -> Result<(), ApplicationError> {
        let _ = principal;
        Ok(())
    }

    /// Requires the principal to carry one of the allowed roles.
    pub fn require_any_role(
        principal: &Principal,
        allowed: &[UserRole],
    ) -> Result<(), ApplicationError> {
        if principal.has_any_role(allowed) {
            Ok(())
        } else {
            Err(ApplicationError::Forbidden)
        }
    }
}

/// Normalizes a login e-mail: trims surrounding whitespace and lowercases
/// ASCII characters.
pub fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn epoch_seconds(at: SystemTime) -> u64 {
    at.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}
