//! Placeholder security implementations.
//!
//! Real authentication is out of scope for the foundation. These types exist
//! so the application ports have adapters to point at once the credential
//! scheme is designed.

use cantina_application::ports::{PasswordVerifier, TokenError, TokenIssuer};
use cantina_core::auth::Principal;

/// Password verifier that always rejects.
///
/// Replace with a real password-hashing adapter when authentication lands.
pub struct NoopPasswordVerifier;

impl Default for NoopPasswordVerifier {
    fn default() -> Self {
        Self
    }
}

impl PasswordVerifier for NoopPasswordVerifier {
    fn verify(&self, _password: &str, _expected_hash: &str) -> bool {
        false
    }
}

/// Token issuer that refuses to issue tokens.
///
/// Replace with a real token adapter when authentication lands.
pub struct NoopTokenIssuer;

impl Default for NoopTokenIssuer {
    fn default() -> Self {
        Self
    }
}

impl TokenIssuer for NoopTokenIssuer {
    fn issue(&self, _principal: &Principal) -> Result<String, TokenError> {
        Err(TokenError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::{NoopPasswordVerifier, NoopTokenIssuer};
    use cantina_application::ports::{PasswordVerifier, TokenError, TokenIssuer};
    use cantina_core::auth::Principal;
    use cantina_core::users::{UserId, UserRole};

    #[test]
    fn noop_verifier_rejects() {
        let verifier = NoopPasswordVerifier;
        assert!(!verifier.verify("password", "hash"));
    }

    #[test]
    fn noop_issuer_returns_typed_error() {
        let issuer = NoopTokenIssuer;
        let principal = Principal::new(UserId::new("u1"), UserRole::Admin);
        assert_eq!(
            issuer.issue(&principal).unwrap_err(),
            TokenError::NotImplemented
        );
    }
}
