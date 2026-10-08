use std::collections::HashSet;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{DatabaseError, DbPool};

/// One emote as fetched from 7TV/BTTV/FFZ, not yet cached — `description`
/// isn't here because a fetch never generates one (see
/// `upsert_and_reconcile`'s doc comment).
#[derive(Debug, Clone)]
pub struct NewEmote {
    pub source: String,
    pub emote_id: String,
    pub name: String,
    pub tags: Option<String>,
    pub image_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedEmote {
    pub source: String,
    pub emote_id: String,
    pub name: String,
    pub tags: Option<String>,
    pub image_url: String,
    pub description: Option<String>,
}

pub struct TwitchEmotesRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> TwitchEmotesRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    /// Upserts every emote in `fetched`, then deletes any cached row for
    /// this channel that's no longer in `fetched` (removed from the
    /// channel since the last refresh). Deliberately never touches
    /// `description`/`described_at` on conflict — a refresh reflects what
    /// the channel's emote set looks like now, not a reason to throw away
    /// a caption already generated for an emote that's still there.
    /// Row counts per channel are tens, not thousands, so per-row deletes
    /// inside one transaction are simpler than binding a dynamic `IN (...)`
    /// list and plenty fast at this scale.
    pub async fn upsert_and_reconcile(&self, broadcaster_id: &str, fetched: &[NewEmote]) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        let mut tx = self.pool.begin().await?;

        let existing_rows = sqlx::query("SELECT source, emote_id FROM twitch_emotes WHERE broadcaster_id = ?1")
            .bind(broadcaster_id)
            .fetch_all(&mut *tx)
            .await?;
        let existing: HashSet<(String, String)> = existing_rows
            .into_iter()
            .map(|row| (row.get::<String, _>("source"), row.get::<String, _>("emote_id")))
            .collect();
        let fresh: HashSet<(String, String)> = fetched.iter().map(|e| (e.source.clone(), e.emote_id.clone())).collect();

        for emote in fetched {
            sqlx::query(
                "INSERT INTO twitch_emotes (broadcaster_id, source, emote_id, name, tags, image_url, fetched_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(broadcaster_id, source, emote_id) DO UPDATE SET
                   name = excluded.name, tags = excluded.tags, image_url = excluded.image_url, fetched_at = excluded.fetched_at",
            )
            .bind(broadcaster_id)
            .bind(&emote.source)
            .bind(&emote.emote_id)
            .bind(&emote.name)
            .bind(&emote.tags)
            .bind(&emote.image_url)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }

        for (source, emote_id) in existing.difference(&fresh) {
            sqlx::query("DELETE FROM twitch_emotes WHERE broadcaster_id = ?1 AND source = ?2 AND emote_id = ?3")
                .bind(broadcaster_id)
                .bind(source)
                .bind(emote_id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    /// Described emotes first (more useful in a capped prompt list — see
    /// `TwitchService::build_extra_instructions`), then by name.
    pub async fn list_for_broadcaster(&self, broadcaster_id: &str) -> Result<Vec<CachedEmote>, DatabaseError> {
        let rows = sqlx::query(
            "SELECT source, emote_id, name, tags, image_url, description FROM twitch_emotes
             WHERE broadcaster_id = ?1 ORDER BY description IS NULL, name",
        )
        .bind(broadcaster_id)
        .fetch_all(self.pool)
        .await?;
        Ok(rows.into_iter().map(row_to_emote).collect())
    }

    pub async fn list_uncaptioned(&self, broadcaster_id: &str) -> Result<Vec<CachedEmote>, DatabaseError> {
        let rows = sqlx::query(
            "SELECT source, emote_id, name, tags, image_url, description FROM twitch_emotes
             WHERE broadcaster_id = ?1 AND description IS NULL ORDER BY name",
        )
        .bind(broadcaster_id)
        .fetch_all(self.pool)
        .await?;
        Ok(rows.into_iter().map(row_to_emote).collect())
    }

    pub async fn set_description(&self, broadcaster_id: &str, source: &str, emote_id: &str, description: &str) -> Result<(), DatabaseError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE twitch_emotes SET description = ?1, described_at = ?2
             WHERE broadcaster_id = ?3 AND source = ?4 AND emote_id = ?5",
        )
        .bind(description)
        .bind(now)
        .bind(broadcaster_id)
        .bind(source)
        .bind(emote_id)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn count_status(&self, broadcaster_id: &str) -> Result<(i64, i64), DatabaseError> {
        let row = sqlx::query(
            "SELECT COUNT(*) as total, COUNT(description) as described FROM twitch_emotes WHERE broadcaster_id = ?1",
        )
        .bind(broadcaster_id)
        .fetch_one(self.pool)
        .await?;
        Ok((row.get::<i64, _>("total"), row.get::<i64, _>("described")))
    }
}

fn row_to_emote(row: sqlx::sqlite::SqliteRow) -> CachedEmote {
    CachedEmote {
        source: row.get("source"),
        emote_id: row.get("emote_id"),
        name: row.get("name"),
        tags: row.get("tags"),
        image_url: row.get("image_url"),
        description: row.get("description"),
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

    fn emote(source: &str, id: &str, name: &str) -> NewEmote {
        NewEmote { source: source.to_string(), emote_id: id.to_string(), name: name.to_string(), tags: None, image_url: format!("https://example.com/{id}") }
    }

    #[tokio::test]
    async fn refresh_then_list_round_trips() {
        let pool = test_pool().await;
        let repo = TwitchEmotesRepository::new(&pool);

        repo.upsert_and_reconcile("123", &[emote("7tv", "a", "GAMBA")]).await.unwrap();
        let listed = repo.list_for_broadcaster("123").await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "GAMBA");
        assert_eq!(listed[0].description, None);
    }

    #[tokio::test]
    async fn refresh_preserves_existing_descriptions() {
        let pool = test_pool().await;
        let repo = TwitchEmotesRepository::new(&pool);

        repo.upsert_and_reconcile("123", &[emote("7tv", "a", "GAMBA")]).await.unwrap();
        repo.set_description("123", "7tv", "a", "casino, apuestas").await.unwrap();

        // Refreshing again (e.g. the channel's set changed slightly) must not wipe the caption.
        repo.upsert_and_reconcile("123", &[emote("7tv", "a", "GAMBA")]).await.unwrap();
        let listed = repo.list_for_broadcaster("123").await.unwrap();
        assert_eq!(listed[0].description.as_deref(), Some("casino, apuestas"));
    }

    #[tokio::test]
    async fn refresh_removes_emotes_no_longer_present() {
        let pool = test_pool().await;
        let repo = TwitchEmotesRepository::new(&pool);

        repo.upsert_and_reconcile("123", &[emote("7tv", "a", "GAMBA"), emote("7tv", "b", "WideHard")]).await.unwrap();
        repo.upsert_and_reconcile("123", &[emote("7tv", "a", "GAMBA")]).await.unwrap();

        let listed = repo.list_for_broadcaster("123").await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "GAMBA");
    }

    #[tokio::test]
    async fn list_uncaptioned_excludes_described_emotes() {
        let pool = test_pool().await;
        let repo = TwitchEmotesRepository::new(&pool);

        repo.upsert_and_reconcile("123", &[emote("7tv", "a", "GAMBA"), emote("7tv", "b", "WideHard")]).await.unwrap();
        repo.set_description("123", "7tv", "a", "casino").await.unwrap();

        let uncaptioned = repo.list_uncaptioned("123").await.unwrap();
        assert_eq!(uncaptioned.len(), 1);
        assert_eq!(uncaptioned[0].name, "WideHard");
    }

    #[tokio::test]
    async fn count_status_reflects_described_count() {
        let pool = test_pool().await;
        let repo = TwitchEmotesRepository::new(&pool);

        repo.upsert_and_reconcile("123", &[emote("7tv", "a", "GAMBA"), emote("7tv", "b", "WideHard")]).await.unwrap();
        repo.set_description("123", "7tv", "a", "casino").await.unwrap();

        assert_eq!(repo.count_status("123").await.unwrap(), (2, 1));
    }
}
