use std::sync::Arc;

use async_trait::async_trait;

use aivt_database::{DbPool, NewPerson, PeopleRepository, Person, MAX_ALIASES_PER_PERSON};
use aivt_secrets::SecretStore;
use aivt_service_settings::SettingsService;

use crate::MemoryError;

/// The "registro de personas" feature — separate from `MemoryManager`
/// (this is a flat name registry, not free-text recollections), but living
/// in the same crate per ADR-004's tier-4 "Person memory" placement, and
/// sharing its DB pool so extraction (`MemoryService::extract_and_store`)
/// can populate it without a cross-crate call.
#[async_trait]
pub trait PeopleManager: Send + Sync {
    async fn list_all(&self) -> Result<Vec<Person>, MemoryError>;
    async fn create_manual(&self, input: NewPerson) -> Result<Person, MemoryError>;
    async fn delete(&self, id: &str) -> Result<(), MemoryError>;
    /// The curated, capped list `CallService::transcription_prompt` folds
    /// into Whisper's `initial_prompt` — see
    /// `PeopleRepository::top_for_prompt` for the ranking rules.
    async fn top_for_prompt(&self, limit: i64) -> Result<Vec<Person>, MemoryError>;
    /// Called by `TwitchService` for every real chat message. A no-op
    /// (returns `Ok(())` without touching the database) when
    /// `PeoplePolicy::twitch_auto_learn_enabled` is off — the check lives
    /// entirely here so Twitch's own logic never needs to know this
    /// setting exists, it just always calls this unconditionally.
    async fn observe_twitch_chatter(&self, user_id: &str, display_name: &str) -> Result<(), MemoryError>;
    /// Called by `DiscordService` for every real message from a human
    /// author. Mirrors `observe_twitch_chatter` exactly, gated by its own
    /// `PeoplePolicy::discord_auto_learn_enabled` toggle instead.
    async fn observe_discord_author(&self, author_id: &str, display_name: &str) -> Result<(), MemoryError>;
}

pub struct PeopleService {
    pool: DbPool,
    secrets: Arc<dyn SecretStore>,
}

impl PeopleService {
    pub fn new(pool: DbPool, secrets: Arc<dyn SecretStore>) -> Self {
        Self { pool, secrets }
    }

    fn repo(&self) -> PeopleRepository<'_> {
        PeopleRepository::new(&self.pool)
    }

    fn settings(&self) -> SettingsService {
        SettingsService::new(self.pool.clone(), self.secrets.clone())
    }
}

#[async_trait]
impl PeopleManager for PeopleService {
    async fn list_all(&self) -> Result<Vec<Person>, MemoryError> {
        Ok(self.repo().list_all().await?)
    }

    async fn create_manual(&self, input: NewPerson) -> Result<Person, MemoryError> {
        Ok(self.repo().create(input).await?)
    }

    async fn delete(&self, id: &str) -> Result<(), MemoryError> {
        Ok(self.repo().delete(id).await?)
    }

    async fn top_for_prompt(&self, limit: i64) -> Result<Vec<Person>, MemoryError> {
        Ok(self.repo().top_for_prompt(limit).await?)
    }

    async fn observe_twitch_chatter(&self, user_id: &str, display_name: &str) -> Result<(), MemoryError> {
        let enabled = match self.settings().people_policy().await {
            Ok(policy) => policy.twitch_auto_learn_enabled,
            Err(err) => {
                tracing::warn!(target: "system", "people: no se pudo leer la política, se omite el registro: {err}");
                return Ok(());
            }
        };
        if !enabled {
            return Ok(());
        }
        Ok(self.repo().observe_twitch_chatter(user_id, display_name, MAX_ALIASES_PER_PERSON).await?)
    }

    async fn observe_discord_author(&self, author_id: &str, display_name: &str) -> Result<(), MemoryError> {
        let enabled = match self.settings().people_policy().await {
            Ok(policy) => policy.discord_auto_learn_enabled,
            Err(err) => {
                tracing::warn!(target: "system", "people: no se pudo leer la política, se omite el registro: {err}");
                return Ok(());
            }
        };
        if !enabled {
            return Ok(());
        }
        Ok(self.repo().observe_discord_author(author_id, display_name, MAX_ALIASES_PER_PERSON).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aivt_testing::MockSecretStore;

    async fn test_service() -> PeopleService {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        PeopleService::new(pool, Arc::new(MockSecretStore::new()))
    }

    #[tokio::test]
    async fn create_list_and_delete_round_trip() {
        let service = test_service().await;
        let created = service.create_manual(NewPerson { canonical_name: "Kenji".to_string(), role: "friend".to_string() }).await.unwrap();

        let all = service.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, created.id);

        service.delete(&created.id).await.unwrap();
        assert!(service.list_all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn observe_twitch_chatter_registers_by_default() {
        let service = test_service().await;
        service.observe_twitch_chatter("123", "Viewer").await.unwrap();
        service.observe_twitch_chatter("123", "Viewer").await.unwrap();

        let all = service.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].mention_count, 2);
    }

    #[tokio::test]
    async fn observe_twitch_chatter_is_a_no_op_when_auto_learn_is_disabled() {
        let service = test_service().await;
        service.settings().set_people_policy(aivt_service_settings::PeoplePolicy { twitch_auto_learn_enabled: false, ..Default::default() }).await.unwrap();

        service.observe_twitch_chatter("123", "Viewer").await.unwrap();

        assert!(service.list_all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn observe_discord_author_registers_by_default() {
        let service = test_service().await;
        service.observe_discord_author("456", "Member").await.unwrap();
        service.observe_discord_author("456", "Member").await.unwrap();

        let all = service.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].mention_count, 2);
    }

    #[tokio::test]
    async fn observe_discord_author_is_a_no_op_when_auto_learn_is_disabled() {
        let service = test_service().await;
        service.settings().set_people_policy(aivt_service_settings::PeoplePolicy { discord_auto_learn_enabled: false, ..Default::default() }).await.unwrap();

        service.observe_discord_author("456", "Member").await.unwrap();

        assert!(service.list_all().await.unwrap().is_empty());
    }
}
