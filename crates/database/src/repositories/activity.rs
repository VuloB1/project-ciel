use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{DatabaseError, DbPool};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ActivityLevel {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl ActivityLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActivityLevel::Debug => "DEBUG",
            ActivityLevel::Info => "INFO",
            ActivityLevel::Warning => "WARNING",
            ActivityLevel::Error => "ERROR",
            ActivityLevel::Critical => "CRITICAL",
        }
    }
}

/// What a caller provides to log one activity event; `id` and `timestamp`
/// are assigned by the repository so every writer stays consistent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewActivityEvent {
    pub level: ActivityLevel,
    pub category: String,
    pub module: String,
    pub action: String,
    pub profile_id: Option<String>,
    pub conversation_id: Option<String>,
    pub request_id: Option<String>,
    pub duration_ms: Option<i64>,
    pub status: String,
    pub summary: String,
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEventRow {
    pub id: String,
    pub timestamp: String,
    pub level: String,
    pub category: String,
    pub module: String,
    pub action: String,
    pub profile_id: Option<String>,
    pub conversation_id: Option<String>,
    pub request_id: Option<String>,
    pub duration_ms: Option<i64>,
    pub status: String,
    pub summary: String,
    pub details_json: Option<String>,
}

pub struct ActivityRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> ActivityRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn record(&self, event: NewActivityEvent) -> Result<String, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let details_json = event.details.map(|d| d.to_string());

        sqlx::query(
            "INSERT INTO activity_events
                (id, timestamp, level, category, module, action, profile_id,
                 conversation_id, request_id, duration_ms, status, summary, details_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        )
        .bind(&id)
        .bind(now)
        .bind(event.level.as_str())
        .bind(event.category)
        .bind(event.module)
        .bind(event.action)
        .bind(event.profile_id)
        .bind(event.conversation_id)
        .bind(event.request_id)
        .bind(event.duration_ms)
        .bind(event.status)
        .bind(event.summary)
        .bind(details_json)
        .execute(self.pool)
        .await?;

        Ok(id)
    }

    pub async fn recent(&self, limit: i64) -> Result<Vec<ActivityEventRow>, DatabaseError> {
        let rows = sqlx::query_as::<_, ActivityEventRow>(
            "SELECT id, timestamp, level, category, module, action, profile_id,
                    conversation_id, request_id, duration_ms, status, summary, details_json
             FROM activity_events
             ORDER BY timestamp DESC
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    /// Wipes the whole feed — the Actividad tab's "Limpiar eventos"
    /// button. No `WHERE`: this is an explicit, user-confirmed "clear
    /// everything", not a scoped delete.
    pub async fn clear(&self) -> Result<(), DatabaseError> {
        sqlx::query("DELETE FROM activity_events").execute(self.pool).await?;
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

    fn sample_event() -> NewActivityEvent {
        NewActivityEvent {
            level: ActivityLevel::Info,
            category: "SYSTEM".into(),
            module: "service_manager".into(),
            action: "status_changed".into(),
            profile_id: None,
            conversation_id: None,
            request_id: None,
            duration_ms: None,
            status: "success".into(),
            summary: "Test event".into(),
            details: None,
        }
    }

    #[tokio::test]
    async fn record_then_recent_round_trips() {
        let pool = test_pool().await;
        let repo = ActivityRepository::new(&pool);

        let id = repo.record(sample_event()).await.unwrap();

        let recent = repo.recent(10).await.unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, id);
        assert_eq!(recent[0].level, "INFO");
        assert_eq!(recent[0].summary, "Test event");
    }

    #[tokio::test]
    async fn recent_respects_limit_and_order() {
        let pool = test_pool().await;
        let repo = ActivityRepository::new(&pool);

        for i in 0..5 {
            let mut event = sample_event();
            event.summary = format!("event-{i}");
            repo.record(event).await.unwrap();
        }

        let recent = repo.recent(2).await.unwrap();
        assert_eq!(recent.len(), 2);
        // Most recent first.
        assert_eq!(recent[0].summary, "event-4");
        assert_eq!(recent[1].summary, "event-3");
    }

    #[tokio::test]
    async fn clear_removes_all_events() {
        let pool = test_pool().await;
        let repo = ActivityRepository::new(&pool);
        repo.record(sample_event()).await.unwrap();
        repo.record(sample_event()).await.unwrap();

        repo.clear().await.unwrap();

        assert_eq!(repo.recent(10).await.unwrap().len(), 0);
    }
}
