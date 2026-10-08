use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::{DatabaseError, DbPool};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl MessageRole {
    fn as_str(self) -> &'static str {
        match self {
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::System => "system",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "assistant" => MessageRole::Assistant,
            "system" => MessageRole::System,
            _ => MessageRole::User,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Streaming,
    Complete,
    Error,
    Cancelled,
}

impl MessageStatus {
    fn as_str(self) -> &'static str {
        match self {
            MessageStatus::Streaming => "streaming",
            MessageStatus::Complete => "complete",
            MessageStatus::Error => "error",
            MessageStatus::Cancelled => "cancelled",
        }
    }

    fn parse(value: &str) -> Self {
        match value {
            "streaming" => MessageStatus::Streaming,
            "error" => MessageStatus::Error,
            "cancelled" => MessageStatus::Cancelled,
            _ => MessageStatus::Complete,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: MessageRole,
    pub content: String,
    pub status: MessageStatus,
    pub provider_id: Option<String>,
    pub model: Option<String>,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct NewMessage {
    pub conversation_id: String,
    pub role: MessageRole,
    pub content: String,
    pub status: MessageStatus,
    pub provider_id: Option<String>,
    pub model: Option<String>,
}

pub struct MessagesRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> MessagesRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: NewMessage) -> Result<Message, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO messages
                (id, conversation_id, role, content, status, provider_id, model, input_tokens, output_tokens, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, ?8)",
        )
        .bind(&id)
        .bind(&input.conversation_id)
        .bind(input.role.as_str())
        .bind(&input.content)
        .bind(input.status.as_str())
        .bind(&input.provider_id)
        .bind(&input.model)
        .bind(&now)
        .execute(self.pool)
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| DatabaseError::NotFound(format!("message {id} right after insert")))
    }

    /// Restaura un mensaje desde un `.aivtprofile` importado — mismo
    /// espíritu que `ConversationsRepository::create_imported`: respeta el
    /// `created_at` original (el orden de `list_for_conversation` depende
    /// de esto) pero siempre con un `id` nuevo.
    #[allow(clippy::too_many_arguments)]
    pub async fn create_imported(
        &self,
        conversation_id: &str,
        role: MessageRole,
        content: &str,
        status: MessageStatus,
        provider_id: Option<&str>,
        model: Option<&str>,
        created_at: &str,
    ) -> Result<Message, DatabaseError> {
        let id = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO messages
                (id, conversation_id, role, content, status, provider_id, model, input_tokens, output_tokens, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, ?8)",
        )
        .bind(&id)
        .bind(conversation_id)
        .bind(role.as_str())
        .bind(content)
        .bind(status.as_str())
        .bind(provider_id)
        .bind(model)
        .bind(created_at)
        .execute(self.pool)
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| DatabaseError::NotFound(format!("message {id} right after insert")))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Message>, DatabaseError> {
        let row = sqlx::query("SELECT * FROM messages WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        Ok(row.map(row_to_message))
    }

    pub async fn list_for_conversation(&self, conversation_id: &str) -> Result<Vec<Message>, DatabaseError> {
        let rows = sqlx::query("SELECT * FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC")
            .bind(conversation_id)
            .fetch_all(self.pool)
            .await?;
        Ok(rows.into_iter().map(row_to_message).collect())
    }

    /// Called once a stream finishes (success, error, or cancellation) to
    /// write the final content and status — see docs/DEVELOPMENT_STATUS.md
    /// for why this isn't written on every token.
    pub async fn finalize(
        &self,
        id: &str,
        content: &str,
        status: MessageStatus,
        output_tokens: Option<i64>,
    ) -> Result<(), DatabaseError> {
        let result = sqlx::query("UPDATE messages SET content = ?1, status = ?2, output_tokens = ?3 WHERE id = ?4")
            .bind(content)
            .bind(status.as_str())
            .bind(output_tokens)
            .bind(id)
            .execute(self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!("message {id}")));
        }
        Ok(())
    }
}

fn row_to_message(row: sqlx::sqlite::SqliteRow) -> Message {
    Message {
        id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        role: MessageRole::parse(&row.get::<String, _>("role")),
        content: row.get("content"),
        status: MessageStatus::parse(&row.get::<String, _>("status")),
        provider_id: row.get("provider_id"),
        model: row.get("model"),
        input_tokens: row.get("input_tokens"),
        output_tokens: row.get("output_tokens"),
        created_at: row.get("created_at"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::conversations::ConversationsRepository;

    async fn test_pool() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        crate::migrate(&pool).await.unwrap();
        pool
    }

    async fn seed_conversation(pool: &DbPool) -> String {
        let profiles = crate::ProfilesRepository::new(pool);
        let profile = profiles
            .create(crate::NewProfile { name: "Ciel".to_string(), nickname: None, description: None })
            .await
            .unwrap();
        let conversation = ConversationsRepository::new(pool).create(&profile.id, "Chat", "chat").await.unwrap();
        conversation.id
    }

    fn user_message(conversation_id: &str, content: &str) -> NewMessage {
        NewMessage {
            conversation_id: conversation_id.to_string(),
            role: MessageRole::User,
            content: content.to_string(),
            status: MessageStatus::Complete,
            provider_id: None,
            model: None,
        }
    }

    #[tokio::test]
    async fn create_then_list_round_trips_in_order() {
        let pool = test_pool().await;
        let conversation_id = seed_conversation(&pool).await;
        let repo = MessagesRepository::new(&pool);

        repo.create(user_message(&conversation_id, "Hola")).await.unwrap();
        repo.create(NewMessage {
            role: MessageRole::Assistant,
            status: MessageStatus::Streaming,
            ..user_message(&conversation_id, "")
        })
        .await
        .unwrap();

        let messages = repo.list_for_conversation(&conversation_id).await.unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[0].content, "Hola");
        assert_eq!(messages[1].role, MessageRole::Assistant);
        assert_eq!(messages[1].status, MessageStatus::Streaming);
    }

    #[tokio::test]
    async fn finalize_updates_content_status_and_tokens() {
        let pool = test_pool().await;
        let conversation_id = seed_conversation(&pool).await;
        let repo = MessagesRepository::new(&pool);
        let created = repo
            .create(NewMessage { role: MessageRole::Assistant, status: MessageStatus::Streaming, ..user_message(&conversation_id, "") })
            .await
            .unwrap();

        repo.finalize(&created.id, "Respuesta completa", MessageStatus::Complete, Some(42)).await.unwrap();

        let fetched = repo.get(&created.id).await.unwrap().unwrap();
        assert_eq!(fetched.content, "Respuesta completa");
        assert_eq!(fetched.status, MessageStatus::Complete);
        assert_eq!(fetched.output_tokens, Some(42));
    }

    #[tokio::test]
    async fn finalize_missing_message_errors() {
        let pool = test_pool().await;
        let repo = MessagesRepository::new(&pool);
        assert!(matches!(
            repo.finalize("missing", "x", MessageStatus::Error, None).await,
            Err(DatabaseError::NotFound(_))
        ));
    }
}
