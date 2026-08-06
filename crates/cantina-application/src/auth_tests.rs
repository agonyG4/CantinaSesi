//! Tests for the authentication and authorization use cases.
//!
//! Application tests use fake password and token services plus in-memory
//! repositories; they never depend on `cantina-infrastructure` and never
//! exercise real Argon2.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use cantina_core::auth::Principal;
use cantina_core::users::{User, UserId, UserRole, UserStatus};
use secrecy::{ExposeSecret, SecretString};

use crate::auth::{
    AuthService, AuthenticateUserCommand, AuthorizationService, SessionConfig, SessionConfigError,
};
use crate::error::ApplicationError;
use crate::ports::StoredPasswordHash;
use crate::test_support::{
    FakePasswordMode, FakePasswordService, FakeTokenService, ManualClock, MemCredentialRepository,
    MemSessionRepository, MemUserRepository,
};

fn fixed_time() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

fn seed_user(
    users: &MemUserRepository,
    credentials: &MemCredentialRepository,
    id: &str,
    email: &str,
    role: UserRole,
    status: UserStatus,
) {
    users.insert(User::new(UserId::new(id), "Test User", email, role, status));
    credentials.insert(
        &UserId::new(id),
        StoredPasswordHash::new(SecretString::from("$argon2id$v=19$seed-hash".to_string())),
    );
}

fn login_cmd(email: &str, password: &str) -> AuthenticateUserCommand {
    AuthenticateUserCommand {
        email: email.to_string(),
        password: SecretString::from(password.to_string()),
    }
}

struct Harness {
    users: Arc<MemUserRepository>,
    credentials: Arc<MemCredentialRepository>,
    sessions: Arc<MemSessionRepository>,
    passwords: Arc<FakePasswordService>,
    tokens: Arc<FakeTokenService>,
    clock: Arc<ManualClock>,
    service: AuthService,
}

fn harness(mode: FakePasswordMode) -> Harness {
    let users = Arc::new(MemUserRepository::new());
    let credentials = Arc::new(MemCredentialRepository::new());
    let sessions = Arc::new(MemSessionRepository::new());
    let passwords = Arc::new(FakePasswordService::new(mode));
    let tokens = Arc::new(FakeTokenService::new());
    let clock = Arc::new(ManualClock::new(fixed_time()));

    let service = AuthService::new(
        users.clone(),
        credentials.clone(),
        sessions.clone(),
        passwords.clone(),
        tokens.clone(),
        clock.clone(),
        SessionConfig::defaults(),
    );

    Harness {
        users,
        credentials,
        sessions,
        passwords,
        tokens,
        clock,
        service,
    }
}

fn seed_active_student(h: &Harness) {
    seed_user(
        &h.users,
        &h.credentials,
        "user_1",
        "ana@example.com",
        UserRole::Student,
        UserStatus::Active,
    );
}

#[tokio::test]
async fn valid_credentials_issue_a_session() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let result = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .expect("login must succeed");

    assert!(!result.access_token.expose_secret().is_empty());
    assert_eq!(result.user.email, "ana@example.com");
    assert!(result.expires_at_epoch_seconds > 1_700_000_000);

    let stored = h
        .sessions
        .session("fp-token-0")
        .expect("the issued session must be stored under its fingerprint");
    assert_eq!(stored.user_id.as_str(), "user_1");
    assert!(stored.revoked_at.is_none());
    assert_eq!(stored.last_seen_at, fixed_time());
}

#[tokio::test]
async fn email_is_normalized_before_lookup() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let result = h
        .service
        .login(login_cmd("  ANA@EXAMPLE.COM  ", "StudentDemo!2026"))
        .await
        .expect("login must normalize the e-mail");

    assert_eq!(result.user.email, "ana@example.com");
}

#[tokio::test]
async fn unknown_email_and_wrong_password_produce_identical_errors() {
    let unknown = {
        let h = harness(FakePasswordMode::Accept);
        seed_active_student(&h);
        h.service
            .login(login_cmd("nobody@example.com", "StudentDemo!2026"))
            .await
            .expect_err("unknown e-mail must be rejected")
    };

    let wrong_password = {
        let h = harness(FakePasswordMode::Reject);
        seed_active_student(&h);
        h.service
            .login(login_cmd("ana@example.com", "wrong-password"))
            .await
            .expect_err("wrong password must be rejected")
    };

    assert!(matches!(unknown, ApplicationError::InvalidCredentials));
    assert!(matches!(
        wrong_password,
        ApplicationError::InvalidCredentials
    ));
}

#[tokio::test]
async fn unknown_user_still_invokes_dummy_password_verification() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let _ = h
        .service
        .login(login_cmd("nobody@example.com", "anything"))
        .await;

    assert!(
        h.passwords.verify_calls().contains(&false),
        "a dummy verification with no stored hash must run for unknown users"
    );
}

#[tokio::test]
async fn inactive_account_with_correct_password_is_rejected() {
    let h = harness(FakePasswordMode::Accept);
    seed_user(
        &h.users,
        &h.credentials,
        "user_inactive",
        "inactive@example.com",
        UserRole::Student,
        UserStatus::Inactive,
    );

    let err = h
        .service
        .login(login_cmd("inactive@example.com", "InactiveDemo!2026"))
        .await
        .expect_err("inactive accounts must be rejected");

    assert!(matches!(err, ApplicationError::AccountInactive));
}

#[tokio::test]
async fn password_service_failure_is_internal() {
    let h = harness(FakePasswordMode::Error);
    seed_active_student(&h);

    let err = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .expect_err("a failing password service must surface an internal error");

    assert!(matches!(err, ApplicationError::PasswordProcessing));
}

#[tokio::test]
async fn token_generation_failure_is_internal() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);
    h.tokens.failing_generation();

    let err = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .expect_err("a failing token service must surface an internal error");

    assert!(matches!(err, ApplicationError::TokenGeneration));
}

#[tokio::test]
async fn session_storage_failure_is_internal() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);
    h.sessions.fail_saves();

    let err = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .expect_err("a failing session store must surface an internal error");

    assert!(matches!(err, ApplicationError::Storage(_)));
}

#[tokio::test]
async fn valid_token_authenticates_to_principal() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let result = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .unwrap();

    let principal = h
        .service
        .authenticate_access_token(&result.access_token)
        .await
        .expect("a freshly issued token must authenticate");

    assert_eq!(principal.user_id().as_str(), "user_1");
    assert_eq!(principal.role(), UserRole::Student);
}

#[tokio::test]
async fn unknown_token_fingerprint_fails() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let err = h
        .service
        .authenticate_access_token(&SecretString::from("never-issued".to_string()))
        .await
        .expect_err("a token with no session must fail");

    assert!(matches!(err, ApplicationError::InvalidAccessToken));
}

#[tokio::test]
async fn revoked_session_fails() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let result = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .unwrap();
    h.service.logout(&result.access_token).await.unwrap();

    let err = h
        .service
        .authenticate_access_token(&result.access_token)
        .await
        .expect_err("a revoked token must fail");

    assert!(matches!(err, ApplicationError::InvalidAccessToken));
}

#[tokio::test]
async fn idle_expired_session_fails() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let result = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .unwrap();
    h.clock
        .advance(SessionConfig::defaults().idle_timeout() + Duration::from_secs(1));

    let err = h
        .service
        .authenticate_access_token(&result.access_token)
        .await
        .expect_err("an idle-expired token must fail");

    assert!(matches!(err, ApplicationError::SessionExpired));
}

#[tokio::test]
async fn absolute_expired_session_fails() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let result = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .unwrap();
    h.clock
        .advance(SessionConfig::defaults().absolute_timeout() + Duration::from_secs(1));

    let err = h
        .service
        .authenticate_access_token(&result.access_token)
        .await
        .expect_err("an absolute-expired token must fail");

    assert!(matches!(err, ApplicationError::SessionExpired));
}

#[tokio::test]
async fn inactive_user_invalidates_an_existing_session() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let result = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .unwrap();

    h.users.insert(User::new(
        UserId::new("user_1"),
        "Test User",
        "ana@example.com",
        UserRole::Student,
        UserStatus::Inactive,
    ));

    let err = h
        .service
        .authenticate_access_token(&result.access_token)
        .await
        .expect_err("an existing session must fail once its user is inactive");

    assert!(matches!(err, ApplicationError::AccountInactive));
}

#[tokio::test]
async fn authenticated_activity_updates_last_seen_at() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let result = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .unwrap();
    h.clock.advance(Duration::from_secs(60));
    let _ = h
        .service
        .authenticate_access_token(&result.access_token)
        .await
        .unwrap();

    let stored = h.sessions.session("fp-token-0").unwrap();
    assert_eq!(
        stored.last_seen_at,
        fixed_time() + Duration::from_secs(60),
        "authenticated activity must refresh last_seen_at"
    );
}

#[tokio::test]
async fn logout_revokes_the_current_session() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let result = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .unwrap();
    h.service.logout(&result.access_token).await.unwrap();

    let stored = h.sessions.session("fp-token-0").unwrap();
    assert!(
        stored.revoked_at.is_some(),
        "logout must revoke the session"
    );
}

#[tokio::test]
async fn refresh_atomically_rotates_the_session() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let first = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .unwrap();
    let second = h
        .service
        .refresh_access_token(&first.access_token)
        .await
        .expect("refresh must succeed");

    assert_ne!(
        first.access_token.expose_secret(),
        second.access_token.expose_secret(),
        "refresh must issue a completely new token"
    );

    let old = h.sessions.session("fp-token-0").unwrap();
    assert!(
        old.revoked_at.is_some(),
        "the old session must be revoked by the rotation"
    );
    let new = h.sessions.session("fp-token-1").unwrap();
    assert!(new.revoked_at.is_none(), "the new session must be active");
    assert_eq!(new.user_id.as_str(), "user_1");
}

#[tokio::test]
async fn old_token_fails_after_refresh() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let first = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .unwrap();
    let _second = h
        .service
        .refresh_access_token(&first.access_token)
        .await
        .unwrap();

    let err = h
        .service
        .authenticate_access_token(&first.access_token)
        .await
        .expect_err("the old token must fail immediately after refresh");

    assert!(matches!(err, ApplicationError::InvalidAccessToken));
}

#[tokio::test]
async fn new_token_succeeds_after_refresh() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let first = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .unwrap();
    let second = h
        .service
        .refresh_access_token(&first.access_token)
        .await
        .unwrap();

    let principal = h
        .service
        .authenticate_access_token(&second.access_token)
        .await
        .expect("the refreshed token must authenticate");
    assert_eq!(principal.user_id().as_str(), "user_1");
}

#[tokio::test]
async fn refresh_reissues_fresh_expirations() {
    let h = harness(FakePasswordMode::Accept);
    seed_active_student(&h);

    let first = h
        .service
        .login(login_cmd("ana@example.com", "StudentDemo!2026"))
        .await
        .unwrap();
    h.clock.advance(Duration::from_secs(1200));
    let second = h
        .service
        .refresh_access_token(&first.access_token)
        .await
        .unwrap();

    let expected =
        fixed_time() + Duration::from_secs(1200) + SessionConfig::defaults().absolute_timeout();
    let expected_epoch = expected.duration_since(UNIX_EPOCH).unwrap().as_secs();
    assert_eq!(
        second.expires_at_epoch_seconds, expected_epoch,
        "the refreshed session must receive a fresh absolute expiration"
    );
}

#[tokio::test]
async fn current_user_lookup_resolves_principal() {
    let h = harness(FakePasswordMode::Accept);
    seed_user(
        &h.users,
        &h.credentials,
        "user_1",
        "ana@example.com",
        UserRole::Student,
        UserStatus::Active,
    );

    let principal = Principal::new(UserId::new("user_1"), UserRole::Student);
    let dto = h
        .service
        .get_current_user(&principal)
        .await
        .expect("current user must resolve");

    assert_eq!(dto.id.as_str(), "user_1");
    assert_eq!(dto.email, "ana@example.com");
    assert_eq!(dto.status, UserStatus::Active);
}

#[test]
fn require_authenticated_accepts_any_principal() {
    let principal = Principal::new(UserId::new("user_1"), UserRole::Student);
    assert!(AuthorizationService::require_authenticated(&principal).is_ok());
}

#[test]
fn require_any_role_accepts_matching_roles() {
    let student = Principal::new(UserId::new("user_1"), UserRole::Student);
    assert!(AuthorizationService::require_any_role(&student, &[UserRole::Student]).is_ok());
    assert!(
        AuthorizationService::require_any_role(&student, &[UserRole::Student, UserRole::Canteen])
            .is_ok()
    );
}

#[test]
fn student_rejected_from_canteen_or_admin_guard() {
    let student = Principal::new(UserId::new("user_1"), UserRole::Student);
    let err =
        AuthorizationService::require_any_role(&student, &[UserRole::Canteen, UserRole::Admin])
            .expect_err("a student must not satisfy the canteen/admin guard");
    assert!(matches!(err, ApplicationError::Forbidden));
}

#[test]
fn canteen_accepted_by_canteen_or_admin_guard() {
    let canteen = Principal::new(UserId::new("user_2"), UserRole::Canteen);
    assert!(
        AuthorizationService::require_any_role(&canteen, &[UserRole::Canteen, UserRole::Admin])
            .is_ok()
    );
}

#[test]
fn admin_accepted_by_canteen_or_admin_guard() {
    let admin = Principal::new(UserId::new("user_3"), UserRole::Admin);
    assert!(
        AuthorizationService::require_any_role(&admin, &[UserRole::Canteen, UserRole::Admin])
            .is_ok()
    );
}

#[test]
fn canteen_rejected_from_admin_only_guard() {
    let canteen = Principal::new(UserId::new("user_2"), UserRole::Canteen);
    let err = AuthorizationService::require_any_role(&canteen, &[UserRole::Admin])
        .expect_err("a canteen account must not satisfy an admin-only guard");
    assert!(matches!(err, ApplicationError::Forbidden));
}

#[test]
fn admin_accepted_by_admin_only_guard() {
    let admin = Principal::new(UserId::new("user_3"), UserRole::Admin);
    assert!(AuthorizationService::require_any_role(&admin, &[UserRole::Admin]).is_ok());
}

#[test]
fn session_config_rejects_zero_idle_timeout() {
    assert_eq!(
        SessionConfig::new(Duration::ZERO, Duration::from_secs(3600)),
        Err(SessionConfigError::ZeroIdleTimeout)
    );
}

#[test]
fn session_config_rejects_zero_absolute_timeout() {
    assert_eq!(
        SessionConfig::new(Duration::from_secs(600), Duration::ZERO),
        Err(SessionConfigError::ZeroAbsoluteTimeout)
    );
}

#[test]
fn session_config_rejects_idle_above_absolute() {
    assert_eq!(
        SessionConfig::new(Duration::from_secs(7200), Duration::from_secs(3600)),
        Err(SessionConfigError::IdleExceedsAbsolute)
    );
}

#[test]
fn session_config_defaults_are_valid() {
    let config = SessionConfig::defaults();
    assert_eq!(config.idle_timeout(), Duration::from_secs(30 * 60));
    assert_eq!(config.absolute_timeout(), Duration::from_secs(8 * 60 * 60));
}

#[test]
fn normalize_email_trims_and_lowercases_ascii() {
    assert_eq!(
        crate::auth::normalize_email("  ANA@Example.COM  "),
        "ana@example.com"
    );
    assert_eq!(
        crate::auth::normalize_email("test@Example.com"),
        "test@example.com"
    );
}
