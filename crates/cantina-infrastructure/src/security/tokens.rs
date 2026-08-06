//! Opaque session token adapter.
//!
//! Tokens are exactly 32 cryptographically random bytes encoded as URL-safe
//! Base64 without padding. The server stores only a SHA-256 fingerprint of the
//! token; the plaintext token is returned exactly once and never persisted.

use base64::Engine;
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};

use cantina_application::ports::{SessionTokenService, TokenError};

/// Token bytes length: 32 bytes of OS-CSPRNG entropy.
pub const TOKEN_BYTES: usize = 32;

/// Generates 32-byte OS-CSPRNG session tokens and their SHA-256 fingerprints.
#[derive(Debug, Default, Clone, Copy)]
pub struct OsRngSessionTokenService;

impl SessionTokenService for OsRngSessionTokenService {
    fn generate_token(&self) -> Result<SecretString, TokenError> {
        let mut bytes = [0u8; TOKEN_BYTES];
        getrandom::getrandom(&mut bytes).map_err(|_| TokenError::Generation)?;
        let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        Ok(SecretString::from(token))
    }

    fn fingerprint(&self, token: &SecretString) -> Result<String, TokenError> {
        let digest = Sha256::digest(token.expose_secret().as_bytes());
        Ok(hex_encode(&digest))
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use secrecy::ExposeSecret;

    use cantina_application::ports::SessionTokenService;

    use super::OsRngSessionTokenService;

    #[test]
    fn generated_tokens_are_at_least_256_bits() {
        let service = OsRngSessionTokenService;
        for _ in 0..8 {
            let token = service
                .generate_token()
                .expect("token generation must succeed");
            assert_eq!(
                token.expose_secret().len(),
                43,
                "32 random bytes URL-safe base64 without padding is 43 characters"
            );
        }
    }

    #[test]
    fn generated_tokens_are_url_safe_and_unpadded() {
        let service = OsRngSessionTokenService;
        for _ in 0..16 {
            let token = service
                .generate_token()
                .expect("token generation must succeed")
                .expose_secret()
                .to_string();
            assert!(
                token
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
                "token must only use URL-safe alphabet"
            );
            assert!(!token.ends_with('='), "token must not be base64 padded");
        }
    }

    #[test]
    fn generated_tokens_are_distinct() {
        let service = OsRngSessionTokenService;
        let mut seen = std::collections::HashSet::new();
        for _ in 0..32 {
            let token = service
                .generate_token()
                .expect("token generation must succeed")
                .expose_secret()
                .to_string();
            assert!(seen.insert(token), "tokens must be unique");
        }
    }

    #[test]
    fn fingerprints_are_deterministic_and_distinct() {
        let service = OsRngSessionTokenService;
        let first = service
            .generate_token()
            .expect("token generation must succeed");
        let second = service
            .generate_token()
            .expect("token generation must succeed");

        let fp_first = service.fingerprint(&first).expect("fingerprint");
        let fp_first_again = service.fingerprint(&first).expect("fingerprint");
        let fp_second = service.fingerprint(&second).expect("fingerprint");

        assert_eq!(
            fp_first, fp_first_again,
            "fingerprint must be deterministic"
        );
        assert_ne!(fp_first, fp_second, "different tokens must differ");
    }

    #[test]
    fn fingerprint_is_not_the_plaintext_token() {
        let service = OsRngSessionTokenService;
        let token = service
            .generate_token()
            .expect("token generation must succeed");
        let fp = service.fingerprint(&token).expect("fingerprint");
        assert_ne!(
            fp,
            token.expose_secret().to_string(),
            "the fingerprint must never equal the plaintext token"
        );
    }

    #[test]
    fn fingerprint_has_sha256_length() {
        let service = OsRngSessionTokenService;
        let token = service
            .generate_token()
            .expect("token generation must succeed");
        let fp = service.fingerprint(&token).expect("fingerprint");
        assert_eq!(fp.len(), 64, "SHA-256 hex fingerprints are 64 characters");
    }
}
