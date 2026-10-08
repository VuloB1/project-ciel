use chrono::Utc;
use sqlx::Row;

use crate::{DatabaseError, DbPool};

/// Backs `app_settings`: a flat key/JSON-value store for anything under
/// Configuración that doesn't yet warrant its own table.
pub struct AppSettingsRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> AppSettingsRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, key: &str) -> Result<Option<String>, DatabaseError> {
        let row = sqlx::query("SELECT value FROM app_settings WHERE key = ?1")
            .bind(key)
            .fetch_optional(self.pool)
            .await?;
        Ok(row.map(|r| r.get::<String, _>("value")))
    }

    pub async fn set(&self, key: &str, value: &str) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .bind(now)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_all(&self) -> Result<Vec<(String, String)>, DatabaseError> {
        let rows = sqlx::query("SELECT key, value FROM app_settings ORDER BY key")
            .fetch_all(self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|r| (r.get::<String, _>("key"), r.get::<String, _>("value")))
            .collect())
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

    #[tokio::test]
    async fn set_then_get_round_trips() {
        let pool = test_pool().await;
        let repo = AppSettingsRepository::new(&pool);

        assert_eq!(repo.get("language").await.unwrap(), None);

        repo.set("language", "\"es\"").await.unwrap();
        assert_eq!(repo.get("language").await.unwrap(), Some("\"es\"".to_string()));

        // Upsert overwrites rather than erroring.
        repo.set("language", "\"en\"").await.unwrap();
        assert_eq!(repo.get("language").await.unwrap(), Some("\"en\"".to_string()));
    }

    #[tokio::test]
    async fn get_all_returns_every_key_sorted() {
        let pool = test_pool().await;
        let repo = AppSettingsRepository::new(&pool);
        repo.set("start_with_system", "true").await.unwrap();
        repo.set("language", "\"es\"").await.unwrap();

        let all = repo.get_all().await.unwrap();
        assert_eq!(
            all,
            vec![
                ("language".to_string(), "\"es\"".to_string()),
                ("start_with_system".to_string(), "true".to_string()),
            ]
        );
    }
}
