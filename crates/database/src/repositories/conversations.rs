use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{DatabaseError, DbPool};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub profile_id: String,
    pub title: String,
    pub source: String,
    pub model: Option<String>,
    pub is_archived: bool,
    pub created_at: String,
    pub updated_at: String,
    /// How many messages have already been considered for memory extraction
    /// (Fase 4) — lets consolidation pick up only the new tail instead of
    /// reprocessing the whole conversation every time.
    pub memory_consolidated_through: i64,
    pub is_pinned: bool,
    /// Manual order among this profile's pinned conversations (lower =
    /// higher in the list) — meaningless while `is_pinned` is false. See
    /// `set_pinned`/`reorder_pinned`.
    pub pin_order: i64,
}

pub struct ConversationsRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> ConversationsRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, profile_id: &str, title: &str, source: &str) -> Result<Conversation, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO conversations (id, profile_id, title, source, model, is_archived, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, NULL, 0, ?5, ?5)",
        )
        .bind(&id)
        .bind(profile_id)
        .bind(title)
        .bind(source)
        .bind(&now)
        .execute(self.pool)
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| DatabaseError::NotFound(format!("conversation {id} right after insert")))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Conversation>, DatabaseError> {
        let row = sqlx::query("SELECT * FROM conversations WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        Ok(row.map(row_to_conversation))
    }

    /// Non-archived conversations for a profile — pinned ones first (in
    /// their own manual `pin_order`), then the rest by most recently
    /// updated, same as before pinning existed.
    pub async fn list_for_profile(&self, profile_id: &str) -> Result<Vec<Conversation>, DatabaseError> {
        let rows = sqlx::query(
            "SELECT * FROM conversations WHERE profile_id = ?1 AND is_archived = 0
             ORDER BY is_pinned DESC, pin_order ASC, updated_at DESC",
        )
        .bind(profile_id)
        .fetch_all(self.pool)
        .await?;
        Ok(rows.into_iter().map(row_to_conversation).collect())
    }

    /// Restaura una conversación desde un `.aivtprofile` importado — a
    /// diferencia de `create`, respeta el `model` y los timestamps
    /// originales (para que el historial siga ordenado por cuándo pasó de
    /// verdad, no por cuándo se importó) pero siempre genera un `id` nuevo,
    /// igual que `ProfileService::import` hace con el perfil mismo — nunca
    /// se reutiliza un id que podría chocar con datos ya existentes.
    pub async fn create_imported(
        &self,
        profile_id: &str,
        title: &str,
        source: &str,
        model: Option<&str>,
        created_at: &str,
        updated_at: &str,
    ) -> Result<Conversation, DatabaseError> {
        let id = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO conversations (id, profile_id, title, source, model, is_archived, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)",
        )
        .bind(&id)
        .bind(profile_id)
        .bind(title)
        .bind(source)
        .bind(model)
        .bind(created_at)
        .bind(updated_at)
        .execute(self.pool)
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| DatabaseError::NotFound(format!("conversation {id} right after insert")))
    }

    pub async fn rename(&self, id: &str, title: &str) -> Result<(), DatabaseError> {
        let result = sqlx::query("UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(title)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(self.pool)
            .await?;
        require_row(result.rows_affected(), id)
    }

    pub async fn set_model(&self, id: &str, model: &str) -> Result<(), DatabaseError> {
        let result = sqlx::query("UPDATE conversations SET model = ?1 WHERE id = ?2")
            .bind(model)
            .bind(id)
            .execute(self.pool)
            .await?;
        require_row(result.rows_affected(), id)
    }

    /// Bumps `updated_at` — called after a new message lands, so the
    /// conversation list stays sorted by recent activity.
    pub async fn touch(&self, id: &str) -> Result<(), DatabaseError> {
        let result = sqlx::query("UPDATE conversations SET updated_at = ?1 WHERE id = ?2")
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(self.pool)
            .await?;
        require_row(result.rows_affected(), id)
    }

    pub async fn archive(&self, id: &str) -> Result<(), DatabaseError> {
        let result = sqlx::query("UPDATE conversations SET is_archived = 1, updated_at = ?1 WHERE id = ?2")
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(self.pool)
            .await?;
        require_row(result.rows_affected(), id)
    }

    pub async fn delete(&self, id: &str) -> Result<(), DatabaseError> {
        let result = sqlx::query("DELETE FROM conversations WHERE id = ?1")
            .bind(id)
            .execute(self.pool)
            .await?;
        require_row(result.rows_affected(), id)
    }

    pub async fn set_memory_consolidated_through(&self, id: &str, message_count: i64) -> Result<(), DatabaseError> {
        let result = sqlx::query("UPDATE conversations SET memory_consolidated_through = ?1 WHERE id = ?2")
            .bind(message_count)
            .bind(id)
            .execute(self.pool)
            .await?;
        require_row(result.rows_affected(), id)
    }

    /// Pins/unpins a conversation. Pinning puts it above every other
    /// already-pinned conversation of the same profile (one below the
    /// current minimum `pin_order`) — a freshly pinned chat lands at the
    /// top, matching what "fijar" reads as to the user, rather than at the
    /// bottom of the pinned group. Unpinning doesn't bother resetting
    /// `pin_order` — it's meaningless once `is_pinned` is false and gets a
    /// fresh value the next time this same conversation is pinned again.
    pub async fn set_pinned(&self, id: &str, pinned: bool) -> Result<(), DatabaseError> {
        if pinned {
            let conversation = self.get(id).await?.ok_or_else(|| DatabaseError::NotFound(format!("conversation {id}")))?;
            let min_order: Option<i64> =
                sqlx::query("SELECT MIN(pin_order) FROM conversations WHERE profile_id = ?1 AND is_pinned = 1")
                    .bind(&conversation.profile_id)
                    .fetch_one(self.pool)
                    .await?
                    .get(0);
            let next_order = min_order.map(|order| order - 1).unwrap_or(0);
            let result = sqlx::query("UPDATE conversations SET is_pinned = 1, pin_order = ?1 WHERE id = ?2")
                .bind(next_order)
                .bind(id)
                .execute(self.pool)
                .await?;
            require_row(result.rows_affected(), id)
        } else {
            let result = sqlx::query("UPDATE conversations SET is_pinned = 0 WHERE id = ?1")
                .bind(id)
                .execute(self.pool)
                .await?;
            require_row(result.rows_affected(), id)
        }
    }

    /// Rewrites `pin_order` for every id in `ordered_ids`, front to back —
    /// the frontend computes the full desired order (drag-and-drop today,
    /// any other mechanism later) and hands it over whole rather than this
    /// repository trying to infer a delta from "moved from index A to B".
    /// Not wrapped in a transaction: each row is independent and a partial
    /// write here just means the next drag corrects it, same tolerance
    /// `MemoryManager::apply_fusion` already accepts for its own per-group
    /// writes.
    pub async fn reorder_pinned(&self, ordered_ids: &[String]) -> Result<(), DatabaseError> {
        for (index, id) in ordered_ids.iter().enumerate() {
            sqlx::query("UPDATE conversations SET pin_order = ?1 WHERE id = ?2 AND is_pinned = 1")
                .bind(index as i64)
                .bind(id)
                .execute(self.pool)
                .await?;
        }
        Ok(())
    }
}

fn require_row(rows_affected: u64, id: &str) -> Result<(), DatabaseError> {
    if rows_affected == 0 {
        Err(DatabaseError::NotFound(format!("conversation {id}")))
    } else {
        Ok(())
    }
}

fn row_to_conversation(row: sqlx::sqlite::SqliteRow) -> Conversation {
    Conversation {
        id: row.get("id"),
        profile_id: row.get("profile_id"),
        title: row.get("title"),
        source: row.get("source"),
        model: row.get("model"),
        is_archived: row.get("is_archived"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        memory_consolidated_through: row.get("memory_consolidated_through"),
        is_pinned: row.get("is_pinned"),
        pin_order: row.get("pin_order"),
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
    async fn create_then_get_round_trips() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = ConversationsRepository::new(&pool);

        let created = repo.create(&profile_id, "Nueva conversación", "chat").await.unwrap();
        assert_eq!(created.profile_id, profile_id);
        assert_eq!(created.source, "chat");
        assert!(!created.is_archived);

        let fetched = repo.get(&created.id).await.unwrap().unwrap();
        assert_eq!(fetched.title, "Nueva conversación");
    }

    #[tokio::test]
    async fn list_for_profile_orders_by_recently_updated() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = ConversationsRepository::new(&pool);

        let first = repo.create(&profile_id, "Primera", "chat").await.unwrap();
        let second = repo.create(&profile_id, "Segunda", "chat").await.unwrap();
        repo.touch(&first.id).await.unwrap();

        let list = repo.list_for_profile(&profile_id).await.unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, first.id);
        assert_eq!(list[1].id, second.id);
    }

    #[tokio::test]
    async fn archive_hides_conversation_from_list() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = ConversationsRepository::new(&pool);
        let created = repo.create(&profile_id, "Temp", "chat").await.unwrap();

        repo.archive(&created.id).await.unwrap();
        assert!(repo.list_for_profile(&profile_id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn rename_updates_title() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = ConversationsRepository::new(&pool);
        let created = repo.create(&profile_id, "Vieja", "chat").await.unwrap();

        repo.rename(&created.id, "Nueva").await.unwrap();
        assert_eq!(repo.get(&created.id).await.unwrap().unwrap().title, "Nueva");
    }

    #[tokio::test]
    async fn delete_removes_conversation() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = ConversationsRepository::new(&pool);
        let created = repo.create(&profile_id, "Temp", "chat").await.unwrap();

        repo.delete(&created.id).await.unwrap();
        assert!(repo.get(&created.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn operations_on_missing_conversation_error() {
        let pool = test_pool().await;
        let repo = ConversationsRepository::new(&pool);
        assert!(matches!(repo.rename("missing", "x").await, Err(DatabaseError::NotFound(_))));
        assert!(matches!(repo.delete("missing").await, Err(DatabaseError::NotFound(_))));
    }

    #[tokio::test]
    async fn memory_consolidation_marker_defaults_to_zero_and_round_trips() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = ConversationsRepository::new(&pool);
        let created = repo.create(&profile_id, "Temp", "chat").await.unwrap();
        assert_eq!(created.memory_consolidated_through, 0);

        repo.set_memory_consolidated_through(&created.id, 6).await.unwrap();
        assert_eq!(repo.get(&created.id).await.unwrap().unwrap().memory_consolidated_through, 6);
    }

    #[tokio::test]
    async fn pinning_puts_a_conversation_above_unpinned_ones_regardless_of_recency() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = ConversationsRepository::new(&pool);

        let older = repo.create(&profile_id, "Vieja", "chat").await.unwrap();
        let newer = repo.create(&profile_id, "Nueva", "chat").await.unwrap();
        repo.touch(&newer.id).await.unwrap();

        repo.set_pinned(&older.id, true).await.unwrap();

        let list = repo.list_for_profile(&profile_id).await.unwrap();
        assert_eq!(list[0].id, older.id);
        assert!(list[0].is_pinned);
        assert_eq!(list[1].id, newer.id);
        assert!(!list[1].is_pinned);
    }

    #[tokio::test]
    async fn pinning_a_second_conversation_puts_it_above_the_first() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = ConversationsRepository::new(&pool);

        let first = repo.create(&profile_id, "Primera fijada", "chat").await.unwrap();
        let second = repo.create(&profile_id, "Segunda fijada", "chat").await.unwrap();
        repo.set_pinned(&first.id, true).await.unwrap();
        repo.set_pinned(&second.id, true).await.unwrap();

        let list = repo.list_for_profile(&profile_id).await.unwrap();
        assert_eq!(list[0].id, second.id, "el fijado más reciente va arriba del todo");
        assert_eq!(list[1].id, first.id);
    }

    #[tokio::test]
    async fn reorder_pinned_applies_the_given_order() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = ConversationsRepository::new(&pool);

        let a = repo.create(&profile_id, "A", "chat").await.unwrap();
        let b = repo.create(&profile_id, "B", "chat").await.unwrap();
        let c = repo.create(&profile_id, "C", "chat").await.unwrap();
        for conversation in [&a, &b, &c] {
            repo.set_pinned(&conversation.id, true).await.unwrap();
        }

        repo.reorder_pinned(&[c.id.clone(), a.id.clone(), b.id.clone()]).await.unwrap();

        let list = repo.list_for_profile(&profile_id).await.unwrap();
        assert_eq!(list.iter().map(|c| c.id.clone()).collect::<Vec<_>>(), vec![c.id, a.id, b.id]);
    }

    #[tokio::test]
    async fn unpinning_removes_it_from_the_pinned_group() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = ConversationsRepository::new(&pool);

        let pinned = repo.create(&profile_id, "Fijada", "chat").await.unwrap();
        let other = repo.create(&profile_id, "Otra", "chat").await.unwrap();
        repo.set_pinned(&pinned.id, true).await.unwrap();
        repo.touch(&other.id).await.unwrap();

        repo.set_pinned(&pinned.id, false).await.unwrap();

        let list = repo.list_for_profile(&profile_id).await.unwrap();
        assert!(!list.iter().find(|c| c.id == pinned.id).unwrap().is_pinned);
        // Back to plain recency order — `other` was touched most recently.
        assert_eq!(list[0].id, other.id);
    }
}
