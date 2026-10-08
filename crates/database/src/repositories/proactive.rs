use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{DatabaseError, DbPool};

/// Whether a character is allowed to initiate a conversation on its own —
/// distinct from `ProfileMood`/`ProfilePersonality`, which describe the
/// character, not whether it may act unprompted. A missing row means
/// "never configured", treated as disabled — same "absence = safe
/// default" convention as `profile_moods`/`memory_embeddings`, so no row
/// needs to be seeded when a profile is created. See
/// docs/ADR/007-proactive-mode.md.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProactiveSettings {
    pub profile_id: String,
    pub enabled: bool,
    pub last_triggered_at: Option<String>,
}

impl ProactiveSettings {
    pub fn disabled(profile_id: &str) -> Self {
        Self { profile_id: profile_id.to_string(), enabled: false, last_triggered_at: None }
    }
}

pub struct ProactiveSettingsRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> ProactiveSettingsRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, profile_id: &str) -> Result<ProactiveSettings, DatabaseError> {
        let row = sqlx::query("SELECT profile_id, enabled, last_triggered_at FROM profile_proactive WHERE profile_id = ?1")
            .bind(profile_id)
            .fetch_optional(self.pool)
            .await?;
        Ok(match row {
            Some(row) => ProactiveSettings {
                profile_id: row.get("profile_id"),
                enabled: row.get("enabled"),
                last_triggered_at: row.get("last_triggered_at"),
            },
            None => ProactiveSettings::disabled(profile_id),
        })
    }

    /// Upserts just the toggle — preserves whatever `last_triggered_at`
    /// was already there (`NULL` for a brand-new row) rather than
    /// resetting the cooldown every time the user flips the switch.
    pub async fn set_enabled(&self, profile_id: &str, enabled: bool) -> Result<ProactiveSettings, DatabaseError> {
        sqlx::query(
            "INSERT INTO profile_proactive (profile_id, enabled, last_triggered_at) VALUES (?1, ?2, NULL)
             ON CONFLICT(profile_id) DO UPDATE SET enabled = excluded.enabled",
        )
        .bind(profile_id)
        .bind(enabled)
        .execute(self.pool)
        .await?;

        self.get(profile_id).await
    }

    /// Called after a proactive message actually goes out — feeds the
    /// cooldown check in `ProactiveService`. Upserts `enabled = 0` for a
    /// row that doesn't exist yet purely so the `NOT NULL` column has a
    /// value; in practice this is only ever called right after `get`
    /// confirmed `enabled` was already `true`, so the row already exists.
    pub async fn record_triggered(&self, profile_id: &str) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO profile_proactive (profile_id, enabled, last_triggered_at) VALUES (?1, 0, ?2)
             ON CONFLICT(profile_id) DO UPDATE SET last_triggered_at = excluded.last_triggered_at",
        )
        .bind(profile_id)
        .bind(&now)
        .execute(self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        crate::migrate(&pool).await.unwrap();
        pool
    }

    async fn seed_profile(pool: &DbPool) -> String {
        let profiles = crate::ProfilesRepository::new(pool);
        let profile = profiles
            .create(crate::NewProfile { name: "Ciel".to_string(), nickname: None, description: None })
            .await
            .unwrap();
        profile.id
    }

    #[tokio::test]
    async fn get_defaults_to_disabled_with_no_row() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;

        let settings = ProactiveSettingsRepository::new(&pool).get(&profile_id).await.unwrap();

        assert!(!settings.enabled);
        assert_eq!(settings.last_triggered_at, None);
    }

    #[tokio::test]
    async fn set_enabled_toggles_and_persists() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = ProactiveSettingsRepository::new(&pool);

        let settings = repo.set_enabled(&profile_id, true).await.unwrap();
        assert!(settings.enabled);

        let settings = repo.set_enabled(&profile_id, false).await.unwrap();
        assert!(!settings.enabled);
    }

    #[tokio::test]
    async fn record_triggered_sets_the_timestamp_without_touching_enabled() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = ProactiveSettingsRepository::new(&pool);
        repo.set_enabled(&profile_id, true).await.unwrap();

        repo.record_triggered(&profile_id).await.unwrap();

        let settings = repo.get(&profile_id).await.unwrap();
        assert!(settings.enabled);
        assert!(settings.last_triggered_at.is_some());
    }

    #[tokio::test]
    async fn record_triggered_overwrites_an_older_timestamp() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = ProactiveSettingsRepository::new(&pool);
        repo.set_enabled(&profile_id, true).await.unwrap();

        // Seed an old timestamp by hand — full control over "before", no
        // need to depend on real wall-clock delay between two calls.
        let old_timestamp = (Utc::now() - chrono::Duration::hours(10)).to_rfc3339();
        sqlx::query("UPDATE profile_proactive SET last_triggered_at = ?1 WHERE profile_id = ?2")
            .bind(&old_timestamp)
            .bind(&profile_id)
            .execute(&pool)
            .await
            .unwrap();

        repo.record_triggered(&profile_id).await.unwrap();
        let updated = repo.get(&profile_id).await.unwrap().last_triggered_at.unwrap();

        assert!(updated > old_timestamp);
    }
}
