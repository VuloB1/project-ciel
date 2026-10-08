use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{DatabaseError, DbPool};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub message_id: String,
    pub kind: String,
    pub path: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub hash: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct NewAttachment {
    pub message_id: String,
    pub kind: String,
    pub path: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub hash: String,
}

pub struct AttachmentsRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> AttachmentsRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: NewAttachment) -> Result<Attachment, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO attachments (id, message_id, kind, path, mime_type, size_bytes, hash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(&id)
        .bind(&input.message_id)
        .bind(&input.kind)
        .bind(&input.path)
        .bind(&input.mime_type)
        .bind(input.size_bytes)
        .bind(&input.hash)
        .bind(&now)
        .execute(self.pool)
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| DatabaseError::NotFound(format!("attachment {id} right after insert")))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Attachment>, DatabaseError> {
        let row = sqlx::query("SELECT * FROM attachments WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        Ok(row.map(row_to_attachment))
    }

    /// One query for every attachment in a conversation, joined through
    /// `messages` — used to enrich a conversation's message list without an
    /// N+1 lookup per message.
    pub async fn list_for_conversation(&self, conversation_id: &str) -> Result<Vec<Attachment>, DatabaseError> {
        let rows = sqlx::query(
            "SELECT attachments.* FROM attachments
             JOIN messages ON messages.id = attachments.message_id
             WHERE messages.conversation_id = ?1",
        )
        .bind(conversation_id)
        .fetch_all(self.pool)
        .await?;
        Ok(rows.into_iter().map(row_to_attachment).collect())
    }

    /// Attachments of `kind` created before `cutoff` (an RFC 3339
    /// timestamp) — backs Chat's audio-retention sweep
    /// (`ChatService::prune_old_audio`). The caller deletes the file on
    /// disk for each row, then this row via `delete`.
    pub async fn list_older_than(&self, kind: &str, cutoff: &str) -> Result<Vec<Attachment>, DatabaseError> {
        let rows = sqlx::query("SELECT * FROM attachments WHERE kind = ?1 AND created_at < ?2")
            .bind(kind)
            .bind(cutoff)
            .fetch_all(self.pool)
            .await?;
        Ok(rows.into_iter().map(row_to_attachment).collect())
    }

    pub async fn delete(&self, id: &str) -> Result<(), DatabaseError> {
        sqlx::query("DELETE FROM attachments WHERE id = ?1").bind(id).execute(self.pool).await?;
        Ok(())
    }
}

fn row_to_attachment(row: sqlx::sqlite::SqliteRow) -> Attachment {
    Attachment {
        id: row.get("id"),
        message_id: row.get("message_id"),
        kind: row.get("kind"),
        path: row.get("path"),
        mime_type: row.get("mime_type"),
        size_bytes: row.get("size_bytes"),
        hash: row.get("hash"),
        created_at: row.get("created_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::conversations::ConversationsRepository;
    use crate::repositories::messages::{MessageRole, MessageStatus, MessagesRepository, NewMessage};

    async fn test_pool() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        crate::migrate(&pool).await.unwrap();
        pool
    }

    async fn seed_message(pool: &DbPool) -> (String, String) {
        let profiles = crate::ProfilesRepository::new(pool);
        let profile = profiles
            .create(crate::NewProfile { name: "Ciel".to_string(), nickname: None, description: None })
            .await
            .unwrap();
        let conversation = ConversationsRepository::new(pool).create(&profile.id, "Chat", "chat").await.unwrap();
        let message = MessagesRepository::new(pool)
            .create(NewMessage {
                conversation_id: conversation.id.clone(),
                role: MessageRole::User,
                content: "mira esto".to_string(),
                status: MessageStatus::Complete,
                provider_id: None,
                model: None,
            })
            .await
            .unwrap();
        (conversation.id, message.id)
    }

    fn sample_attachment(message_id: &str) -> NewAttachment {
        NewAttachment {
            message_id: message_id.to_string(),
            kind: "image".to_string(),
            path: "attachments/ab/abcdef.png".to_string(),
            mime_type: "image/png".to_string(),
            size_bytes: 1234,
            hash: "abcdef".to_string(),
        }
    }

    #[tokio::test]
    async fn create_then_get_round_trips() {
        let pool = test_pool().await;
        let (_, message_id) = seed_message(&pool).await;
        let repo = AttachmentsRepository::new(&pool);

        let created = repo.create(sample_attachment(&message_id)).await.unwrap();
        let fetched = repo.get(&created.id).await.unwrap().unwrap();

        assert_eq!(fetched.message_id, message_id);
        assert_eq!(fetched.mime_type, "image/png");
        assert_eq!(fetched.size_bytes, 1234);
    }

    #[tokio::test]
    async fn list_for_conversation_joins_through_messages() {
        let pool = test_pool().await;
        let (conversation_id, message_id) = seed_message(&pool).await;
        let repo = AttachmentsRepository::new(&pool);
        repo.create(sample_attachment(&message_id)).await.unwrap();

        let attachments = repo.list_for_conversation(&conversation_id).await.unwrap();
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].message_id, message_id);
    }

    #[tokio::test]
    async fn list_for_conversation_is_empty_when_nothing_attached() {
        let pool = test_pool().await;
        let (conversation_id, _) = seed_message(&pool).await;
        let repo = AttachmentsRepository::new(&pool);

        assert!(repo.list_for_conversation(&conversation_id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn deleting_the_message_cascades_to_its_attachment() {
        let pool = test_pool().await;
        let (_, message_id) = seed_message(&pool).await;
        let repo = AttachmentsRepository::new(&pool);
        let created = repo.create(sample_attachment(&message_id)).await.unwrap();

        sqlx::query("DELETE FROM messages WHERE id = ?1").bind(&message_id).execute(&pool).await.unwrap();

        assert!(repo.get(&created.id).await.unwrap().is_none());
    }

    fn sample_audio_attachment(message_id: &str) -> NewAttachment {
        NewAttachment {
            message_id: message_id.to_string(),
            kind: "audio".to_string(),
            path: "attachments/ab/abcdef.mp3".to_string(),
            mime_type: "audio/mpeg".to_string(),
            size_bytes: 4321,
            hash: "abcdef".to_string(),
        }
    }

    #[tokio::test]
    async fn list_older_than_only_returns_the_matching_kind_before_the_cutoff() {
        let pool = test_pool().await;
        let (_, message_id) = seed_message(&pool).await;
        let repo = AttachmentsRepository::new(&pool);
        let old_audio = repo.create(sample_audio_attachment(&message_id)).await.unwrap();
        repo.create(sample_attachment(&message_id)).await.unwrap(); // image, same message — must not match

        let far_future_cutoff = "9999-01-01T00:00:00Z";
        let matches = repo.list_older_than("audio", far_future_cutoff).await.unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, old_audio.id);

        let far_past_cutoff = "2000-01-01T00:00:00Z";
        assert!(repo.list_older_than("audio", far_past_cutoff).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_removes_the_row() {
        let pool = test_pool().await;
        let (_, message_id) = seed_message(&pool).await;
        let repo = AttachmentsRepository::new(&pool);
        let created = repo.create(sample_audio_attachment(&message_id)).await.unwrap();

        repo.delete(&created.id).await.unwrap();

        assert!(repo.get(&created.id).await.unwrap().is_none());
    }
}
