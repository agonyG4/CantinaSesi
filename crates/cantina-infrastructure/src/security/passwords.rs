//! Argon2id password hashing adapter.
//!
//! Implements the application `PasswordService` port with Argon2id (v19),
//! production parameters: at least 19 MiB of memory, 2 iterations and
//! parallelism 1. Hashing and verification are CPU-bound so they always run on
//! the blocking pool through `tokio::task::spawn_blocking`.

use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::{Algorithm, Argon2, Params, Version};
use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use tokio::task::spawn_blocking;

use cantina_application::ports::{PasswordError, PasswordService, StoredPasswordHash};

/// Argon2id password hasher matching the documented production parameters.
#[derive(Debug, Clone)]
pub struct Argon2PasswordService {
    params: Params,
}

impl Argon2PasswordService {
    /// Creates the service with the production Argon2id parameters.
    pub fn new() -> Self {
        Self {
            params: Params::default(),
        }
    }

    /// Creates the service with reduced parameters, intended exclusively for
    /// tests that must not spend time on production-grade hashing.
    #[cfg(test)]
    pub fn low_cost() -> Self {
        Self {
            params: Params::new(8 * 1024, 1, 1, Some(32)).expect("valid low-cost parameters"),
        }
    }
}

impl Default for Argon2PasswordService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PasswordService for Argon2PasswordService {
    async fn hash_password(
        &self,
        password: SecretString,
    ) -> Result<StoredPasswordHash, PasswordError> {
        let params = self.params.clone();
        spawn_blocking(move || {
            let salt = SaltString::generate(&mut OsRng);
            let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
            let hash = argon2
                .hash_password(password.expose_secret().as_bytes(), &salt)
                .map_err(|_| PasswordError::HashingFailed)?;
            Ok(StoredPasswordHash::new(SecretString::from(hash.to_string())))
        })
        .await
        .map_err(|_| PasswordError::HashingFailed)?
    }

    async fn verify_password(
        &self,
        password: SecretString,
        expected_hash: Option<&StoredPasswordHash>,
    ) -> Result<bool, PasswordError> {
        match expected_hash {
            Some(expected) => {
                let phc = expected.as_phc().to_string();
                spawn_blocking(move || {
                    let parsed =
                        PasswordHash::new(&phc).map_err(|_| PasswordError::MalformedHash)?;
                    let argon2 = Argon2::default();
                    Ok(argon2
                        .verify_password(password.expose_secret().as_bytes(), &parsed)
                        .is_ok())
                })
                .await
                .map_err(|_| PasswordError::VerificationFailed)?
            }
            None => {
                // Dummy verification: hash a throwaway password and verify the
                // supplied one against it. The work performed matches a real
                // verification so unknown users are not distinguishable by
                // timing, and the result is always false.
                spawn_blocking(move || {
                    let salt = SaltString::generate(&mut OsRng);
                    let argon2 = Argon2::default();
                    let dummy = argon2
                        .hash_password(b"cantina-dummy-verification", &salt)
                        .map_err(|_| PasswordError::VerificationFailed)?;
                    let _ = argon2.verify_password(password.expose_secret().as_bytes(), &dummy);
                    Ok(false)
                })
                .await
                .map_err(|_| PasswordError::VerificationFailed)?
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use argon2::password_hash::PasswordHash;
    use argon2::Algorithm;
    use secrecy::SecretString;

    use cantina_application::ports::{PasswordError, PasswordService};

    use super::Argon2PasswordService;

    async fn hash_roundtrip(password: &str) {
        let service = Argon2PasswordService::low_cost();
        let stored = service
            .hash_password(SecretString::from(password.to_string()))
            .await
            .expect("hash must succeed");
        let ok = service
            .verify_password(SecretString::from(password.to_string()), Some(&stored))
            .await
            .expect("verify must succeed");
        assert!(ok, "the correct password must verify");
    }

    #[tokio::test]
    async fn hashes_and_verifies_a_password() {
        hash_roundtrip("StudentDemo!2026").await;
    }

    #[tokio::test]
    async fn rejects_the_wrong_password() {
        let service = Argon2PasswordService::low_cost();
        let stored = service
            .hash_password(SecretString::from("StudentDemo!2026".to_string()))
            .await
            .expect("hash must succeed");
        let ok = service
            .verify_password(SecretString::from("wrong-password".to_string()), Some(&stored))
            .await
            .expect("verify must succeed");
        assert!(!ok, "the wrong password must not verify");
    }

    #[tokio::test]
    async fn malformed_phc_surfaces_a_typed_error() {
        let service = Argon2PasswordService::low_cost();
        let stored = cantina_application::ports::StoredPasswordHash::new(SecretString::from(
            "not-a-phc-string".to_string(),
        ));
        let err = service
            .verify_password(SecretString::from("StudentDemo!2026".to_string()), Some(&stored))
            .await
            .expect_err("a malformed hash must be rejected");
        assert!(matches!(err, PasswordError::MalformedHash));
    }

    #[tokio::test]
    async fn generated_hashes_use_argon2id_with_a_salt() {
        let service = Argon2PasswordService::low_cost();
        let stored = service
            .hash_password(SecretString::from("StudentDemo!2026".to_string()))
            .await
            .expect("hash must succeed");
        let parsed =
            PasswordHash::new(stored.as_phc()).expect("the PHC string must be parseable");
        assert_eq!(
            Algorithm::try_from(parsed.algorithm).expect("known algorithm"),
            Algorithm::Argon2id
        );
        assert!(
            parsed.salt.is_some(),
            "the generated hash must embed a salt"
        );
    }

    #[tokio::test]
    async fn unique_salts_produce_distinct_hashes() {
        let service = Argon2PasswordService::low_cost();
        let first = service
            .hash_password(SecretString::from("StudentDemo!2026".to_string()))
            .await
            .expect("hash must succeed");
        let second = service
            .hash_password(SecretString::from("StudentDemo!2026".to_string()))
            .await
            .expect("hash must succeed");
        assert_ne!(
            first.as_phc(),
            second.as_phc(),
            "a fresh random salt must produce a different hash every time"
        );
    }

    #[tokio::test]
    async fn dummy_verification_executes_and_returns_false() {
        let service = Argon2PasswordService::low_cost();
        let ok = service
            .verify_password(SecretString::from("StudentDemo!2026".to_string()), None)
            .await
            .expect("dummy verification must succeed");
        assert!(!ok, "the dummy verification must always report a mismatch");
    }
}
