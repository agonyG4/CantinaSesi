//! In-memory adapter for the `SessionRepository` port.
//!
//! Sessions are keyed by the SHA-256 fingerprint of the bearer token. The
//! plaintext token is never stored here.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;

use async_trait::async_trait;

use cantina_application::ports::{RepositoryError, SessionRecord, SessionRepository};

use super::{Store, lock_store};

/// In-memory adapter for server-side sessions.
pub struct InMemorySessionRepository {
    sessions: Store<SessionRecord>,
}

impl InMemorySessionRepository {
    /// Creates an empty in-memory session repository.
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemorySessionRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionRepository for InMemorySessionRepository {
    async fn save_session(&self, session: SessionRecord) -> Result<(), RepositoryError> {
        lock_store(&self.sessions)?.insert(session.token_fingerprint.clone(), session);
        Ok(())
    }

    async fn find_session_by_fingerprint(
        &self,
        fingerprint: &str,
    ) -> Result<Option<SessionRecord>, RepositoryError> {
        Ok(lock_store(&self.sessions)?.get(fingerprint).cloned())
    }

    async fn update_last_seen_at(
        &self,
        fingerprint: &str,
        at: SystemTime,
    ) -> Result<(), RepositoryError> {
        if let Some(session) = lock_store(&self.sessions)?.get_mut(fingerprint) {
            session.last_seen_at = at;
        }
        Ok(())
    }

    async fn revoke_session(
        &self,
        fingerprint: &str,
        revoked_at: SystemTime,
    ) -> Result<(), RepositoryError> {
        if let Some(session) = lock_store(&self.sessions)?.get_mut(fingerprint) {
            session.revoked_at = Some(revoked_at);
        }
        Ok(())
    }

    async fn rotate_session(
        &self,
        old_fingerprint: &str,
        new_session: SessionRecord,
        revoked_at: SystemTime,
    ) -> Result<(), RepositoryError> {
        let mut sessions = lock_store(&self.sessions)?;
        if let Some(old) = sessions.get_mut(old_fingerprint) {
            old.revoked_at = Some(revoked_at);
        }
        sessions.insert(new_session.token_fingerprint.clone(), new_session);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use cantina_application::ports::{SessionRecord, SessionRepository};
    use cantina_core::users::UserId;

    use super::InMemorySessionRepository;

    fn now() -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(1_700_000_000)
    }

    fn record(fingerprint: &str, user_id: &str) -> SessionRecord {
        SessionRecord {
            token_fingerprint: fingerprint.to_string(),
            user_id: UserId::new(user_id),
            created_at: now(),
            last_seen_at: now(),
            absolute_expires_at: now() + Duration::from_secs(3600),
            revoked_at: None,
        }
    }

    #[tokio::test]
    async fn saves_and_finds_a_session_by_fingerprint() {
        let repo = InMemorySessionRepository::new();
        repo.save_session(record("fp-1", "user_1"))
            .await
            .expect("save");
        let found = repo
            .find_session_by_fingerprint("fp-1")
            .await
            .expect("find");
        assert_eq!(found.unwrap().user_id, UserId::new("user_1"));
    }

    #[tokio::test]
    async fn unknown_fingerprint_returns_none() {
        let repo = InMemorySessionRepository::new();
        let found = repo
            .find_session_by_fingerprint("nope")
            .await
            .expect("find");
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn revocation_is_recorded() {
        let repo = InMemorySessionRepository::new();
        repo.save_session(record("fp-1", "user_1"))
            .await
            .expect("save");
        repo.revoke_session("fp-1", now()).await.expect("revoke");
        let found = repo
            .find_session_by_fingerprint("fp-1")
            .await
            .expect("find")
            .unwrap();
        assert!(found.revoked_at.is_some());
    }

    #[tokio::test]
    async fn last_seen_at_is_updated() {
        let repo = InMemorySessionRepository::new();
        repo.save_session(record("fp-1", "user_1"))
            .await
            .expect("save");
        let later = now() + Duration::from_secs(120);
        repo.update_last_seen_at("fp-1", later)
            .await
            .expect("update");
        let found = repo
            .find_session_by_fingerprint("fp-1")
            .await
            .expect("find")
            .unwrap();
        assert_eq!(found.last_seen_at, later);
    }

    #[tokio::test]
    async fn rotation_revokes_old_and_stores_new() {
        let repo = InMemorySessionRepository::new();
        repo.save_session(record("fp-old", "user_1"))
            .await
            .expect("save");
        repo.rotate_session("fp-old", record("fp-new", "user_1"), now())
            .await
            .expect("rotate");

        let old = repo
            .find_session_by_fingerprint("fp-old")
            .await
            .expect("find")
            .unwrap();
        assert!(old.revoked_at.is_some(), "old session must be revoked");

        let new = repo
            .find_session_by_fingerprint("fp-new")
            .await
            .expect("find")
            .unwrap();
        assert!(new.revoked_at.is_none(), "new session must be active");
    }

    #[tokio::test]
    async fn rotating_an_unknown_old_fingerprint_still_stores_the_new_session() {
        let repo = InMemorySessionRepository::new();
        repo.rotate_session("fp-ghost", record("fp-new", "user_1"), now())
            .await
            .expect("rotate");
        let new = repo
            .find_session_by_fingerprint("fp-new")
            .await
            .expect("find")
            .unwrap();
        assert_eq!(new.user_id, UserId::new("user_1"));
    }
}
