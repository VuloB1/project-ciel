use chrono::Utc;

use crate::{DatabaseError, DbPool};

pub struct MemoryEmbeddingsRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> MemoryEmbeddingsRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    /// Inserts or overwrites the embedding for one memory — a memory
    /// re-embedded after an edit (or a future model swap) just replaces
    /// its single row rather than accumulating stale ones.
    pub async fn upsert(&self, memory_id: &str, embedding: &[f32], model_version: &str) -> Result<(), DatabaseError> {
        sqlx::query(
            "INSERT INTO memory_embeddings (memory_id, embedding, model_version, updated_at) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(memory_id) DO UPDATE SET embedding = excluded.embedding, model_version = excluded.model_version, updated_at = excluded.updated_at",
        )
        .bind(memory_id)
        .bind(encode_embedding(embedding))
        .bind(model_version)
        .bind(Utc::now().to_rfc3339())
        .execute(self.pool)
        .await?;
        Ok(())
    }
}

/// Raw little-endian `f32` bytes, no header — the simplest encoding that
/// needs no new dependency (`sqlx` already binds `Vec<u8>` to `BLOB`
/// natively), paired with `decode_embedding` below.
pub(crate) fn encode_embedding(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub(crate) fn decode_embedding(bytes: &[u8]) -> Vec<f32> {
    bytes.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::memories::{MemoriesRepository, NewMemory};

    async fn test_pool() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        crate::migrate(&pool).await.unwrap();
        pool
    }

    async fn seed_memory(pool: &DbPool) -> String {
        let profiles = crate::ProfilesRepository::new(pool);
        let profile = profiles
            .create(crate::NewProfile { name: "Ciel".to_string(), nickname: None, description: None })
            .await
            .unwrap();
        let memories = MemoriesRepository::new(pool);
        let memory = memories
            .create(NewMemory {
                profile_id: profile.id,
                title: "Senderismo".to_string(),
                content: "Le gusta el senderismo".to_string(),
                summary: None,
                category: "preference".to_string(),
                importance: 70,
                confidence: 100,
                source: "manual".to_string(),
                source_id: None,
            })
            .await
            .unwrap();
        memory.id
    }

    #[test]
    fn embedding_round_trips_through_encode_and_decode() {
        let vector = vec![0.5_f32, -0.25, 1.0, 0.0];
        assert_eq!(decode_embedding(&encode_embedding(&vector)), vector);
    }

    #[tokio::test]
    async fn upsert_then_get_round_trips_through_the_database() {
        let pool = test_pool().await;
        let memory_id = seed_memory(&pool).await;
        let repo = MemoryEmbeddingsRepository::new(&pool);
        let vector = vec![0.1_f32, 0.2, 0.3];

        repo.upsert(&memory_id, &vector, "test-model-v1").await.unwrap();

        let row = sqlx::query("SELECT embedding, model_version FROM memory_embeddings WHERE memory_id = ?1")
            .bind(&memory_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let blob: Vec<u8> = sqlx::Row::get(&row, "embedding");
        let model_version: String = sqlx::Row::get(&row, "model_version");
        assert_eq!(decode_embedding(&blob), vector);
        assert_eq!(model_version, "test-model-v1");
    }

    #[tokio::test]
    async fn upserting_the_same_memory_again_overwrites_the_previous_embedding() {
        let pool = test_pool().await;
        let memory_id = seed_memory(&pool).await;
        let repo = MemoryEmbeddingsRepository::new(&pool);

        repo.upsert(&memory_id, &[1.0, 0.0], "v1").await.unwrap();
        repo.upsert(&memory_id, &[0.0, 1.0], "v2").await.unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_embeddings WHERE memory_id = ?1")
            .bind(&memory_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "el upsert debe pisar la fila, no acumular otra");

        let row = sqlx::query("SELECT embedding, model_version FROM memory_embeddings WHERE memory_id = ?1")
            .bind(&memory_id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let blob: Vec<u8> = sqlx::Row::get(&row, "embedding");
        let model_version: String = sqlx::Row::get(&row, "model_version");
        assert_eq!(decode_embedding(&blob), vec![0.0, 1.0]);
        assert_eq!(model_version, "v2");
    }

    #[tokio::test]
    async fn deleting_the_parent_memory_cascades_to_its_embedding() {
        let pool = test_pool().await;
        let memory_id = seed_memory(&pool).await;
        MemoryEmbeddingsRepository::new(&pool).upsert(&memory_id, &[1.0], "v1").await.unwrap();

        MemoriesRepository::new(&pool).delete(&memory_id).await.unwrap();

        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM memory_embeddings").fetch_one(&pool).await.unwrap();
        assert_eq!(count, 0);
    }
}
