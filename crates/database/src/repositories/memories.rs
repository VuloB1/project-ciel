use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

use crate::repositories::memory_embeddings::decode_embedding;
use crate::{DatabaseError, DbPool};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    pub id: String,
    pub profile_id: String,
    /// Short label ("Bebida favorita") — lets the Memoria tab read as a
    /// scannable list instead of a wall of paragraphs (pedido del usuario,
    /// Fase 4 en vivo).
    pub title: String,
    /// The detailed version — organizational context for the user's own
    /// benefit in the Memoria tab, never sent to the model (see
    /// `AIOrchestrator`'s memory section). Still capped in the extraction
    /// prompt (`extraction::system_prompt`) so it stays "denso, no pesado"
    /// rather than an unbounded essay, even though it never reaches a
    /// prompt — a tidy note is still more useful than a wall of text.
    pub content: String,
    /// The compact "resumen" — the *only* part of a memory that ever
    /// reaches the chat system prompt (see `AIOrchestrator`'s memory
    /// section). An earlier design also sent `content` as a secondary
    /// "use only if the user asks for more" line, but that asked a plain
    /// prompt to do conditional attention with no real tool-calling behind
    /// it, and it didn't hold up in live use — reverted, pedido del
    /// usuario.
    pub summary: Option<String>,
    pub category: String,
    pub importance: i64,
    pub confidence: i64,
    pub source: String,
    pub source_id: Option<String>,
    pub is_pinned: bool,
    /// `MEMORY_SCOPE_PRIVATE` (solo canales del dueño: Chat y Llamada) o
    /// `MEMORY_SCOPE_PUBLIC` (también Twitch/Discord). Toda memoria nace
    /// privada; hacerla pública es una decisión explícita del usuario.
    pub scope: String,
    pub created_at: String,
    pub updated_at: String,
    pub last_accessed_at: Option<String>,
    pub access_count: i64,
}

pub const MEMORY_SCOPE_PRIVATE: &str = "private";
pub const MEMORY_SCOPE_PUBLIC: &str = "public";

#[derive(Debug, Clone)]
pub struct NewMemory {
    pub profile_id: String,
    pub title: String,
    pub content: String,
    pub summary: Option<String>,
    pub category: String,
    pub importance: i64,
    pub confidence: i64,
    pub source: String,
    pub source_id: Option<String>,
}

/// Everything the user can change about an existing memory (Fase 4:
/// edición pedida tras la primera prueba en vivo, extendida para incluir
/// `title`/`summary` en el rediseño posterior). Never touches
/// `source`/`source_id`/`confidence` — those describe provenance, which
/// editing the text shouldn't retroactively rewrite.
#[derive(Debug, Clone)]
pub struct MemoryUpdate {
    pub title: String,
    pub content: String,
    pub summary: Option<String>,
    pub category: String,
    pub importance: i64,
}

/// Floor for `search_by_embedding` — a candidate below this cosine
/// similarity is treated as "not actually related" rather than "the least
/// unrelated thing available". `paraphrase-multilingual-MiniLM-L12-v2`
/// (the embedding model this app ships, ADR-005) tends to put genuinely
/// related Spanish sentences in the 0.4–0.7+ range and unrelated ones
/// noticeably lower; 0.35 starts conservative (letting borderline matches
/// through) rather than aggressive, since a memory search staying silent
/// too often is a worse user experience than an occasional loosely-related
/// one — the reverse of a completely unrelated memory getting force-fed
/// into every single turn, which is the bug this constant fixes. A
/// starting point meant to be tuned from real usage, not a value derived
/// from a benchmark.
const MIN_SEMANTIC_SIMILARITY: f32 = 0.35;

pub struct MemoriesRepository<'a> {
    pool: &'a DbPool,
}

impl<'a> MemoriesRepository<'a> {
    pub fn new(pool: &'a DbPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, input: NewMemory) -> Result<Memory, DatabaseError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO memories
                (id, profile_id, title, content, summary, category, importance, confidence, source, source_id, is_pinned, created_at, updated_at, last_accessed_at, access_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?11, NULL, 0)",
        )
        .bind(&id)
        .bind(&input.profile_id)
        .bind(&input.title)
        .bind(&input.content)
        .bind(&input.summary)
        .bind(&input.category)
        .bind(input.importance)
        .bind(input.confidence)
        .bind(&input.source)
        .bind(&input.source_id)
        .bind(&now)
        .execute(self.pool)
        .await?;

        self.get(&id)
            .await?
            .ok_or_else(|| DatabaseError::NotFound(format!("memory {id} right after insert")))
    }

    pub async fn get(&self, id: &str) -> Result<Option<Memory>, DatabaseError> {
        let row = sqlx::query("SELECT * FROM memories WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        Ok(row.map(row_to_memory))
    }

    /// Every memory for a profile, pinned first and then newest-created
    /// first — backs the Memoria tab's default (unfiltered) list.
    /// `importance` is still shown per-memory but no longer drives list
    /// order: a chronological feed is what the user asked for after using
    /// the tab live (pedido explícito: "de más antigua a más nueva, donde
    /// las más nuevas se ven arriba"). Pinning still wins over date — a
    /// memory the user deliberately pinned should stay visible regardless
    /// of how much newer other memories get.
    pub async fn list_for_profile(&self, profile_id: &str) -> Result<Vec<Memory>, DatabaseError> {
        let rows = sqlx::query("SELECT * FROM memories WHERE profile_id = ?1 ORDER BY is_pinned DESC, created_at DESC")
            .bind(profile_id)
            .fetch_all(self.pool)
            .await?;
        Ok(rows.into_iter().map(row_to_memory).collect())
    }

    /// FTS5 search scoped to one profile, ranked by relevance — used both by
    /// the Memoria tab's search box and by the orchestrator to pull context
    /// relevant to the current message before responding. Empty/whitespace
    /// queries return no results rather than erroring (FTS5 rejects an
    /// empty `MATCH` expression).
    ///
    /// Candidates sharing fewer than `MIN_FTS_TERM_OVERLAP` content terms
    /// with a multi-word query are dropped — BM25 relevance degenerates
    /// with a small memory store (a profile with only one or two memories
    /// has no real corpus for IDF to weigh "distinctive" against "common"
    /// word), so `to_fts_query`'s OR-of-all-terms used to let a single
    /// generic shared word (e.g. "sistema", "video") from a long automatic
    /// retrieval query resurface an unrelated memory on nearly every turn.
    /// Pedido del usuario tras verlo en vivo: una sola memoria guardada
    /// terminaba repitiéndose casi textual en cada respuesta. A query with
    /// only one content term is unaffected (nothing to require "at least 2
    /// of" from), so a manual one-word search in la pestaña Memoria still
    /// behaves exactly as before.
    pub async fn search(&self, profile_id: &str, query: &str, limit: i64) -> Result<Vec<Memory>, DatabaseError> {
        self.search_scoped(profile_id, query, limit, false).await
    }

    /// Igual que `search`, pero con `public_only` solo devuelve memorias de
    /// alcance público — lo que puede usar un canal abierto a terceros
    /// (Twitch/Discord). El filtro va en la propia consulta, no después: así
    /// las memorias privadas no desplazan a las públicas del tope del ranking.
    pub async fn search_scoped(&self, profile_id: &str, query: &str, limit: i64, public_only: bool) -> Result<Vec<Memory>, DatabaseError> {
        let Some(terms) = content_terms(query) else { return Ok(Vec::new()) };
        let fts_query = terms.iter().map(|term| format!("\"{}\"", term.replace('"', "\"\""))).collect::<Vec<_>>().join(" OR ");

        let rows = sqlx::query(
            "SELECT memories.* FROM memories
             JOIN memories_fts ON memories_fts.rowid = memories.rowid
             WHERE memories.profile_id = ?1 AND memories_fts MATCH ?2
               AND (?4 = 0 OR memories.scope = 'public')
             ORDER BY memories_fts.rank
             LIMIT ?3",
        )
        .bind(profile_id)
        .bind(&fts_query)
        .bind(limit)
        .bind(public_only)
        .fetch_all(self.pool)
        .await?;

        let min_overlap = terms.len().min(MIN_FTS_TERM_OVERLAP);
        Ok(rows
            .into_iter()
            .map(row_to_memory)
            .filter(|memory| matching_term_count(memory, &terms) >= min_overlap)
            .take(limit.max(0) as usize)
            .collect())
    }

    pub async fn set_pinned(&self, id: &str, pinned: bool) -> Result<(), DatabaseError> {
        let result = sqlx::query("UPDATE memories SET is_pinned = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(pinned)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(self.pool)
            .await?;
        require_row(result.rows_affected(), id)
    }

    /// Cambia quién puede usar la memoria (`MEMORY_SCOPE_PRIVATE`/`_PUBLIC`).
    /// Un valor desconocido se rechaza: un alcance mal escrito no debe
    /// quedar guardado como "algo que no es privado" por accidente.
    pub async fn set_scope(&self, id: &str, scope: &str) -> Result<(), DatabaseError> {
        if scope != MEMORY_SCOPE_PRIVATE && scope != MEMORY_SCOPE_PUBLIC {
            return Err(DatabaseError::NotFound(format!("unknown memory scope '{scope}' (expected 'private' or 'public')")));
        }
        let result = sqlx::query("UPDATE memories SET scope = ?1, updated_at = ?2 WHERE id = ?3")
            .bind(scope)
            .bind(Utc::now().to_rfc3339())
            .bind(id)
            .execute(self.pool)
            .await?;
        require_row(result.rows_affected(), id)
    }

    /// Edits a memory's own content — lets the user correct something the
    /// extraction got wrong (sección "Fase 4" del estado de desarrollo:
    /// pedido explícito tras la primera prueba en vivo). Doesn't touch
    /// `source`/`source_id`/`confidence`: those describe where the memory
    /// *came from*, which editing the text shouldn't retroactively change.
    pub async fn update(&self, id: &str, update: MemoryUpdate) -> Result<Memory, DatabaseError> {
        let result = sqlx::query(
            "UPDATE memories SET title = ?1, content = ?2, summary = ?3, category = ?4, importance = ?5, updated_at = ?6 WHERE id = ?7",
        )
        .bind(&update.title)
        .bind(&update.content)
        .bind(&update.summary)
        .bind(&update.category)
        .bind(update.importance)
        .bind(Utc::now().to_rfc3339())
        .bind(id)
        .execute(self.pool)
        .await?;
        require_row(result.rows_affected(), id)?;
        self.get(id).await?.ok_or_else(|| DatabaseError::NotFound(format!("memory {id}")))
    }

    pub async fn delete(&self, id: &str) -> Result<(), DatabaseError> {
        let result = sqlx::query("DELETE FROM memories WHERE id = ?1").bind(id).execute(self.pool).await?;
        require_row(result.rows_affected(), id)
    }

    /// Best-effort bookkeeping — called when a memory is actually surfaced
    /// to a generation, not on every read. Never fails the caller: losing an
    /// access-count increment isn't worth failing a chat response over.
    pub async fn touch_access(&self, id: &str) {
        let _ = sqlx::query(
            "UPDATE memories SET access_count = access_count + 1, last_accessed_at = ?1 WHERE id = ?2",
        )
        .bind(Utc::now().to_rfc3339())
        .bind(id)
        .execute(self.pool)
        .await;
    }

    /// Brute-force cosine similarity (a plain dot product — vectors are
    /// L2-normalized at embed time, see
    /// `aivt_provider_embedding::LocalEmbeddingProvider`) over every
    /// embedded memory for one profile. At this app's scale (hundreds to
    /// low thousands of memories per profile, never millions) this is
    /// comfortably fast enough with no ANN index — see
    /// docs/ADR/005-local-embedding-search.md.
    ///
    /// Only ever returns candidates at or above `MIN_SEMANTIC_SIMILARITY` —
    /// without a floor, this was always returning the `limit` *closest*
    /// vectors even when none of them were actually related to the query,
    /// because "nearest neighbor" always finds *something*. Reciprocal Rank
    /// Fusion (`MemoryManager::search`) only looks at each list's rank, not
    /// its score, so those weak matches rode into the fused result — and
    /// from there into the model's prompt on every single turn — exactly as
    /// confidently as a real match. Pedido del usuario tras notar que la IA
    /// se volvía redundante y terminaba ignorando el mensaje actual a favor
    /// de "recuerdos" que en realidad no venían al caso.
    pub async fn search_by_embedding(&self, profile_id: &str, query_vector: &[f32], limit: i64) -> Result<Vec<Memory>, DatabaseError> {
        self.search_by_embedding_scoped(profile_id, query_vector, limit, false).await
    }

    /// Ver `search_scoped`: `public_only` limita a memorias de alcance público.
    pub async fn search_by_embedding_scoped(
        &self,
        profile_id: &str,
        query_vector: &[f32],
        limit: i64,
        public_only: bool,
    ) -> Result<Vec<Memory>, DatabaseError> {
        let rows = sqlx::query(
            "SELECT memories.*, memory_embeddings.embedding AS embedding_blob
             FROM memories JOIN memory_embeddings ON memory_embeddings.memory_id = memories.id
             WHERE memories.profile_id = ?1 AND (?2 = 0 OR memories.scope = 'public')",
        )
        .bind(profile_id)
        .bind(public_only)
        .fetch_all(self.pool)
        .await?;

        let mut scored: Vec<(f32, Memory)> = rows
            .into_iter()
            .filter_map(|row| {
                let blob: Vec<u8> = row.get("embedding_blob");
                let embedding = decode_embedding(&blob);
                // Guards against a future model swap with a different
                // dimension: a mismatched row is skipped, not a crash.
                (embedding.len() == query_vector.len()).then(|| (dot_product(query_vector, &embedding), row_to_memory(row)))
            })
            .filter(|(score, _)| *score >= MIN_SEMANTIC_SIMILARITY)
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit.max(0) as usize);
        Ok(scored.into_iter().map(|(_, m)| m).collect())
    }

    /// Backs the embedding backfill command — memories that predate the
    /// embedding model being downloaded, or were created while it was
    /// unavailable.
    pub async fn missing_embeddings_for_profile(&self, profile_id: &str) -> Result<Vec<Memory>, DatabaseError> {
        let rows = sqlx::query(
            "SELECT memories.* FROM memories
             LEFT JOIN memory_embeddings ON memory_embeddings.memory_id = memories.id
             WHERE memories.profile_id = ?1 AND memory_embeddings.memory_id IS NULL",
        )
        .bind(profile_id)
        .fetch_all(self.pool)
        .await?;
        Ok(rows.into_iter().map(row_to_memory).collect())
    }
}

fn require_row(rows_affected: u64, id: &str) -> Result<(), DatabaseError> {
    if rows_affected == 0 {
        Err(DatabaseError::NotFound(format!("memory {id}")))
    } else {
        Ok(())
    }
}

fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Common Spanish/English function words — filtered out of retrieval
/// queries (see `to_fts_query`) so they can't drown out the words that
/// actually distinguish one memory from another. Chat's automatic
/// retrieval passes the user's raw message as the query (sección
/// "Problemas encontrados y corregidos", Fase 4: a natural question like
/// "¿qué me dijiste sobre...?" was diluting BM25 ranking with "qué"/"me"/
/// "sobre" and letting the actually relevant memory fall out of the top
/// results) — not exhaustive, just enough to cover ordinary conversational
/// phrasing.
const STOPWORDS: &[&str] = &[
    "el", "la", "los", "las", "un", "una", "unos", "unas", "de", "del", "al", "y", "o", "u",
    "que", "qué", "quien", "quién", "como", "cómo", "cuando", "cuándo", "donde", "dónde",
    "por", "para", "con", "sin", "sobre", "entre", "hacia", "hasta", "es", "son", "fue",
    "era", "ser", "estar", "esta", "está", "este", "esa", "eso", "yo", "tú", "tu", "él",
    "ella", "nosotros", "ustedes", "ellos", "ellas", "me", "te", "se", "nos", "les", "le",
    "lo", "mi", "mis", "tus", "su", "sus", "no", "sí", "si", "muy", "más", "mas", "también",
    "pero", "porque", "cual", "cuál", "en", "a", "ya", "dije", "dijiste", "dijo", "sabes",
    "sabías", "acuerdas", "recuerdas",
    "the", "an", "of", "and", "or", "to", "in", "on", "for", "with", "is", "are", "was",
    "were", "be", "you", "your", "i", "my", "what", "did", "do", "does", "that", "this", "it",
    "know", "remember", "about",
];

/// A candidate below this many shared content terms is dropped by
/// `search` when the query has enough terms to meaningfully ask for more
/// than one — see that method's doc comment for why.
const MIN_FTS_TERM_OVERLAP: usize = 2;

/// Extracts the content words `search` actually matches on: lowercased,
/// stripped of surrounding punctuation, stopwords dropped first (see
/// `STOPWORDS`) unless the whole query turns out to be nothing but
/// stopwords, in which case keeping all of them beats returning nothing at
/// all. `None` only for a query with literally no words in it.
fn content_terms(query: &str) -> Option<Vec<String>> {
    let all_terms: Vec<String> = query
        .split_whitespace()
        .map(|term| term.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
        .filter(|term| !term.is_empty())
        .collect();

    let filtered: Vec<String> = all_terms.iter().filter(|term| !STOPWORDS.contains(&term.as_str())).cloned().collect();
    let chosen = if filtered.is_empty() { all_terms } else { filtered };

    (!chosen.is_empty()).then_some(chosen)
}

/// How many of `terms` appear as a whole word somewhere in `memory`'s
/// title, content, or summary — case/accent-insensitive (`aivt_core::
/// contains_word` folds both), same matching rule Twitch's mention
/// detection already uses for the same "not a substring, a real word"
/// reason.
fn matching_term_count(memory: &Memory, terms: &[String]) -> usize {
    let haystack = format!("{} {} {}", memory.title, memory.content, memory.summary.as_deref().unwrap_or(""));
    terms.iter().filter(|term| aivt_core::contains_word(&haystack, term)).count()
}

fn row_to_memory(row: sqlx::sqlite::SqliteRow) -> Memory {
    Memory {
        id: row.get("id"),
        profile_id: row.get("profile_id"),
        title: row.get("title"),
        content: row.get("content"),
        summary: row.get("summary"),
        category: row.get("category"),
        importance: row.get("importance"),
        confidence: row.get("confidence"),
        source: row.get("source"),
        source_id: row.get("source_id"),
        is_pinned: row.get("is_pinned"),
        scope: row.get("scope"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        last_accessed_at: row.get("last_accessed_at"),
        access_count: row.get("access_count"),
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

    fn sample(profile_id: &str, content: &str) -> NewMemory {
        NewMemory {
            profile_id: profile_id.to_string(),
            title: "Título de prueba".to_string(),
            content: content.to_string(),
            summary: None,
            category: "fact".to_string(),
            importance: 50,
            confidence: 80,
            source: "chat".to_string(),
            source_id: None,
        }
    }

    #[tokio::test]
    async fn create_then_get_round_trips() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);

        let created = repo.create(sample(&profile_id, "Le encanta el café")).await.unwrap();
        assert_eq!(created.content, "Le encanta el café");
        assert!(!created.is_pinned);
        assert_eq!(created.access_count, 0);

        let fetched = repo.get(&created.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, created.id);
    }

    #[tokio::test]
    async fn a_new_memory_is_private_by_default() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);

        let created = repo.create(sample(&profile_id, "Algo dicho en una charla privada")).await.unwrap();
        assert_eq!(created.scope, MEMORY_SCOPE_PRIVATE);
    }

    #[tokio::test]
    async fn public_only_search_never_returns_private_memories() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);

        let private = repo.create(sample(&profile_id, "Le encanta el café, secreto del dueño")).await.unwrap();
        let public = repo.create(sample(&profile_id, "Le encanta el café, dato para el público")).await.unwrap();
        repo.set_scope(&public.id, MEMORY_SCOPE_PUBLIC).await.unwrap();

        let owner_view = repo.search_scoped(&profile_id, "café", 10, false).await.unwrap();
        assert_eq!(owner_view.len(), 2, "los canales del dueño ven todo");

        let public_view = repo.search_scoped(&profile_id, "café", 10, true).await.unwrap();
        assert_eq!(public_view.len(), 1);
        assert_eq!(public_view[0].id, public.id);
        assert!(public_view.iter().all(|m| m.id != private.id));
    }

    #[tokio::test]
    async fn public_only_search_is_not_crowded_out_by_better_ranked_private_memories() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);

        // Con un `limit` de 1, si el filtro se aplicara después de limitar,
        // la memoria privada (mejor ranking) ocuparía el único lugar y la
        // pública no aparecería nunca.
        for _ in 0..3 {
            repo.create(sample(&profile_id, "café café café café")).await.unwrap();
        }
        let public = repo.create(sample(&profile_id, "un café")).await.unwrap();
        repo.set_scope(&public.id, MEMORY_SCOPE_PUBLIC).await.unwrap();

        let public_view = repo.search_scoped(&profile_id, "café", 1, true).await.unwrap();
        assert_eq!(public_view.len(), 1);
        assert_eq!(public_view[0].id, public.id);
    }

    #[tokio::test]
    async fn set_scope_can_make_a_memory_private_again_and_rejects_unknown_values() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);
        let memory = repo.create(sample(&profile_id, "Dato cualquiera")).await.unwrap();

        repo.set_scope(&memory.id, MEMORY_SCOPE_PUBLIC).await.unwrap();
        assert_eq!(repo.get(&memory.id).await.unwrap().unwrap().scope, MEMORY_SCOPE_PUBLIC);

        repo.set_scope(&memory.id, MEMORY_SCOPE_PRIVATE).await.unwrap();
        assert_eq!(repo.get(&memory.id).await.unwrap().unwrap().scope, MEMORY_SCOPE_PRIVATE);

        assert!(repo.set_scope(&memory.id, "todos").await.is_err(), "un alcance desconocido no se guarda");
        assert_eq!(repo.get(&memory.id).await.unwrap().unwrap().scope, MEMORY_SCOPE_PRIVATE);
        assert!(repo.set_scope("no-existe", MEMORY_SCOPE_PUBLIC).await.is_err());
    }

    #[tokio::test]
    async fn list_for_profile_orders_pinned_first_regardless_of_importance() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);

        let low = repo
            .create(NewMemory { importance: 10, ..sample(&profile_id, "Detalle menor") })
            .await
            .unwrap();
        let high = repo
            .create(NewMemory { importance: 90, ..sample(&profile_id, "Detalle importante") })
            .await
            .unwrap();
        repo.set_pinned(&low.id, true).await.unwrap();

        let list = repo.list_for_profile(&profile_id).await.unwrap();
        assert_eq!(list[0].id, low.id, "pinned comes first even with lower importance and older creation");
        assert_eq!(list[1].id, high.id);
    }

    #[tokio::test]
    async fn list_for_profile_otherwise_orders_newest_first() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);

        // Importance is deliberately the *opposite* of creation order here —
        // if importance still drove sorting this test would fail, proving
        // the list is chronological now, not importance-first.
        let first = repo
            .create(NewMemory { importance: 90, ..sample(&profile_id, "Se creó primero") })
            .await
            .unwrap();
        let second = repo
            .create(NewMemory { importance: 10, ..sample(&profile_id, "Se creó después") })
            .await
            .unwrap();

        let list = repo.list_for_profile(&profile_id).await.unwrap();
        assert_eq!(list[0].id, second.id, "newest first, even though it has lower importance");
        assert_eq!(list[1].id, first.id);
    }

    #[tokio::test]
    async fn search_finds_matching_memories_scoped_to_the_profile() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let other_profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);

        repo.create(sample(&profile_id, "Le encanta el café por las mañanas")).await.unwrap();
        repo.create(sample(&profile_id, "Trabaja como diseñadora gráfica")).await.unwrap();
        repo.create(sample(&other_profile_id, "Le encanta el café también")).await.unwrap();

        let results = repo.search(&profile_id, "café", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("café"));
    }

    #[tokio::test]
    async fn search_matches_on_title_too() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);
        repo.create(NewMemory { title: "Bebida favorita".to_string(), ..sample(&profile_id, "Detalle sin esa palabra") })
            .await
            .unwrap();

        let results = repo.search(&profile_id, "bebida", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Bebida favorita");
    }

    #[tokio::test]
    async fn search_with_an_empty_query_returns_nothing() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);
        repo.create(sample(&profile_id, "algo")).await.unwrap();

        assert!(repo.search(&profile_id, "   ", 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn search_query_with_quotes_does_not_error() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);
        repo.create(sample(&profile_id, "Dijo \"hola\" ayer")).await.unwrap();

        let results = repo.search(&profile_id, "\"hola\" OR malicious", 10).await;
        assert!(results.is_ok());
    }

    #[test]
    fn content_terms_drops_stopwords_but_keeps_content_words() {
        // Regression test: chat's automatic retrieval passes the user's raw
        // message as the query — a natural question like "¿qué me dijiste
        // sobre X?" used to weigh "qué"/"me"/"sobre" just as heavily as the
        // actual content word in BM25 ranking, letting unrelated memories
        // that happened to repeat common words outrank the relevant one.
        let terms = content_terms("¿qué le gusta hacer los fines de semana?").unwrap();
        assert!(terms.contains(&"gusta".to_string()), "terms were: {terms:?}");
        assert!(terms.contains(&"fines".to_string()), "terms were: {terms:?}");
        assert!(terms.contains(&"semana".to_string()), "terms were: {terms:?}");
        assert!(!terms.contains(&"qué".to_string()), "terms were: {terms:?}");
        assert!(!terms.contains(&"le".to_string()), "terms were: {terms:?}");
        assert!(!terms.contains(&"los".to_string()), "terms were: {terms:?}");
    }

    #[test]
    fn content_terms_falls_back_to_every_word_when_the_whole_query_is_stopwords() {
        // Better to over-match than return nothing for a query with no
        // content words at all (an edge case, not the common path).
        let terms = content_terms("qué te dije").unwrap();
        assert!(terms.contains(&"qué".to_string()));
        assert!(terms.contains(&"dije".to_string()));
    }

    #[test]
    fn content_terms_is_case_insensitive_against_the_stopword_list() {
        let terms = content_terms("¿Qué SABES sobre el café?").unwrap();
        assert!(terms.contains(&"café".to_string()));
        assert!(!terms.contains(&"qué".to_string()));
        assert!(!terms.contains(&"sabes".to_string()));
    }

    #[test]
    fn search_requires_at_least_two_shared_terms_for_a_multi_word_query() {
        let memory = Memory {
            id: "m1".to_string(),
            profile_id: "p1".to_string(),
            title: "Configuración de video".to_string(),
            content: "Quiere armar un sistema local para generar videos.".to_string(),
            summary: None,
            category: "preference".to_string(),
            importance: 50,
            confidence: 80,
            source: "manual".to_string(),
            source_id: None,
            is_pinned: false,
            scope: MEMORY_SCOPE_PRIVATE.to_string(),
            created_at: String::new(),
            updated_at: String::new(),
            last_accessed_at: None,
            access_count: 0,
        };

        // Shares only "sistema" with the memory — a single generic word out
        // of an otherwise unrelated sentence must not count as a match.
        let weak_query = content_terms("tengo un problema con mi sistema operativo, ayuda").unwrap();
        assert!(matching_term_count(&memory, &weak_query) < MIN_FTS_TERM_OVERLAP);

        // Shares "video"/"videos" and "sistema" — genuinely on-topic.
        let strong_query = content_terms("quiero armar un sistema para hacer videos").unwrap();
        assert!(matching_term_count(&memory, &strong_query) >= MIN_FTS_TERM_OVERLAP);
    }

    #[tokio::test]
    async fn delete_removes_the_memory_and_it_no_longer_matches_search() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);
        let created = repo.create(sample(&profile_id, "Algo temporal")).await.unwrap();

        repo.delete(&created.id).await.unwrap();

        assert!(repo.get(&created.id).await.unwrap().is_none());
        assert!(repo.search(&profile_id, "temporal", 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn touch_access_increments_count_and_sets_last_accessed() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);
        let created = repo.create(sample(&profile_id, "algo")).await.unwrap();

        repo.touch_access(&created.id).await;

        let fetched = repo.get(&created.id).await.unwrap().unwrap();
        assert_eq!(fetched.access_count, 1);
        assert!(fetched.last_accessed_at.is_some());
    }

    #[tokio::test]
    async fn operations_on_missing_memory_error() {
        let pool = test_pool().await;
        let repo = MemoriesRepository::new(&pool);
        assert!(matches!(repo.set_pinned("missing", true).await, Err(DatabaseError::NotFound(_))));
        assert!(matches!(repo.delete("missing").await, Err(DatabaseError::NotFound(_))));
        assert!(matches!(
            repo.update("missing", sample_update("x", "fact", 50)).await,
            Err(DatabaseError::NotFound(_))
        ));
    }

    fn sample_update(content: &str, category: &str, importance: i64) -> MemoryUpdate {
        MemoryUpdate {
            title: "Título editado".to_string(),
            content: content.to_string(),
            summary: None,
            category: category.to_string(),
            importance,
        }
    }

    #[tokio::test]
    async fn update_edits_title_content_category_and_importance_but_not_provenance() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);
        let created = repo.create(sample(&profile_id, "Texto original")).await.unwrap();

        let updated = repo.update(&created.id, sample_update("Texto corregido", "preference", 90)).await.unwrap();

        assert_eq!(updated.title, "Título editado");
        assert_eq!(updated.content, "Texto corregido");
        assert_eq!(updated.category, "preference");
        assert_eq!(updated.importance, 90);
        assert_eq!(updated.source, created.source);
        assert_eq!(updated.confidence, created.confidence);
    }

    #[tokio::test]
    async fn update_is_reflected_in_search() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);
        let created = repo.create(sample(&profile_id, "palabra original")).await.unwrap();

        repo.update(&created.id, sample_update("palabra corregida", "fact", 50)).await.unwrap();

        assert!(repo.search(&profile_id, "original", 10).await.unwrap().is_empty());
        assert_eq!(repo.search(&profile_id, "corregida", 10).await.unwrap().len(), 1);
    }

    async fn embed(pool: &DbPool, memory_id: &str, vector: &[f32]) {
        crate::repositories::memory_embeddings::MemoryEmbeddingsRepository::new(pool)
            .upsert(memory_id, vector, "test-model")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn search_by_embedding_ranks_the_closer_vector_first() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);

        let close = repo.create(sample(&profile_id, "cercana")).await.unwrap();
        // Farther than `close` but still above `MIN_SEMANTIC_SIMILARITY` —
        // this test is about ranking order, not the floor (see
        // `search_by_embedding_filters_out_weakly_related_memories` for
        // that), so it must stay above the cutoff on its own.
        let far = repo.create(sample(&profile_id, "lejana")).await.unwrap();
        embed(&pool, &close.id, &[0.98, 0.2]).await;
        embed(&pool, &far.id, &[0.6, 0.8]).await;

        let results = repo.search_by_embedding(&profile_id, &[1.0, 0.0], 10).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, close.id, "el vector más cercano a la consulta debe rankear primero");
        assert_eq!(results[1].id, far.id);
    }

    #[tokio::test]
    async fn public_only_embedding_search_never_returns_private_memories() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);

        // La privada es la más cercana a la consulta: aun así no debe salir.
        let private = repo.create(sample(&profile_id, "privada")).await.unwrap();
        let public = repo.create(sample(&profile_id, "pública")).await.unwrap();
        repo.set_scope(&public.id, MEMORY_SCOPE_PUBLIC).await.unwrap();
        embed(&pool, &private.id, &[0.98, 0.2]).await;
        embed(&pool, &public.id, &[0.6, 0.8]).await;

        let results = repo.search_by_embedding_scoped(&profile_id, &[1.0, 0.0], 10, true).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, public.id);

        let owner_view = repo.search_by_embedding_scoped(&profile_id, &[1.0, 0.0], 10, false).await.unwrap();
        assert_eq!(owner_view.len(), 2);
    }

    #[tokio::test]
    async fn search_by_embedding_filters_out_weakly_related_memories() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);

        let related = repo.create(sample(&profile_id, "relacionada")).await.unwrap();
        let unrelated = repo.create(sample(&profile_id, "no relacionada")).await.unwrap();
        embed(&pool, &related.id, &[0.9, 0.44]).await;
        // Near-orthogonal to the query — the "nearest" thing left in an
        // unrelated profile, exactly the case that used to get force-fed
        // into the prompt just for being the least-far candidate.
        embed(&pool, &unrelated.id, &[0.05, 1.0]).await;

        let results = repo.search_by_embedding(&profile_id, &[1.0, 0.0], 10).await.unwrap();

        assert_eq!(results.len(), 1, "una memoria bajo el umbral de similitud no debe aparecer, aunque sea la más cercana disponible");
        assert_eq!(results[0].id, related.id);
    }

    #[tokio::test]
    async fn search_by_embedding_returns_nothing_when_no_memory_clears_the_threshold() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);

        let memory = repo.create(sample(&profile_id, "sin relación")).await.unwrap();
        embed(&pool, &memory.id, &[0.0, 1.0]).await;

        let results = repo.search_by_embedding(&profile_id, &[1.0, 0.0], 10).await.unwrap();

        assert!(results.is_empty(), "sin ninguna memoria realmente relacionada, la búsqueda semántica no debe forzar un resultado");
    }

    #[tokio::test]
    async fn search_by_embedding_is_scoped_to_the_profile() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let other_profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);

        let other = repo.create(sample(&other_profile_id, "de otro perfil")).await.unwrap();
        embed(&pool, &other.id, &[1.0, 0.0]).await;

        let results = repo.search_by_embedding(&profile_id, &[1.0, 0.0], 10).await.unwrap();
        assert!(results.is_empty(), "una memoria embebida de otro perfil no debe aparecer");
    }

    #[tokio::test]
    async fn search_by_embedding_skips_memories_with_no_embedding_yet() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);
        repo.create(sample(&profile_id, "todavía sin embeber")).await.unwrap();

        let results = repo.search_by_embedding(&profile_id, &[1.0, 0.0], 10).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn missing_embeddings_for_profile_returns_only_unembedded_memories() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);

        let embedded = repo.create(sample(&profile_id, "ya embebida")).await.unwrap();
        let pending = repo.create(sample(&profile_id, "todavía pendiente")).await.unwrap();
        embed(&pool, &embedded.id, &[1.0, 0.0]).await;

        let missing = repo.missing_embeddings_for_profile(&profile_id).await.unwrap();

        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].id, pending.id);
    }

    #[tokio::test]
    async fn missing_embeddings_for_profile_is_empty_once_everything_is_embedded() {
        let pool = test_pool().await;
        let profile_id = seed_profile(&pool).await;
        let repo = MemoriesRepository::new(&pool);
        let memory = repo.create(sample(&profile_id, "una memoria")).await.unwrap();
        embed(&pool, &memory.id, &[1.0, 0.0]).await;

        assert!(repo.missing_embeddings_for_profile(&profile_id).await.unwrap().is_empty());
    }
}
