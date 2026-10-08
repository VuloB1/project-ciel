use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures::StreamExt;

use aivt_database::{
    DbPool, MemoriesRepository, Memory, MemoryEmbeddingsRepository, MemoryUpdate, Message, MoodRepository,
    NewMemory, PeopleRepository, ProactiveSettings, ProactiveSettingsRepository, ProfileMood,
    MAX_ALIASES_PER_PERSON, MEMORY_SCOPE_PUBLIC,
};
use aivt_eventbus::{AppEvent, EventBus};
use aivt_provider_embedding::{EmbeddingError, EmbeddingProvider, MODEL_VERSION};
use aivt_provider_llm::{ChatMessage, ChatRequest, LLMProvider, MessageRole};

use crate::extraction::{parse_extraction_response, system_prompt, transcript};
use crate::fusion::{memories_listing, parse_fusion_response, system_prompt as fusion_system_prompt};
use crate::{FusionGroupInput, FusionProposal, MemoryError};

/// Extraction is a one-shot analysis call, not a creative response — always
/// forced low regardless of whatever temperature the user picked for
/// casual chat, so the same conversation doesn't produce wildly different
/// memories from run to run.
const EXTRACTION_TEMPERATURE: f32 = 0.2;
const EXTRACTION_MAX_TOKENS: u32 = 768;
/// A person candidate below this confidence isn't saved — unlike a memory
/// (which just sits unused in FTS search if wrong), a bad guess here
/// pollutes a small, space-constrained Whisper prompt
/// (`CallService::transcription_prompt`), so a low-confidence guess is
/// worse than skipping it entirely.
const PEOPLE_CONFIDENCE_THRESHOLD: i64 = 50;
/// Below this many memories, asking the model to "find redundant groups"
/// isn't worth a round-trip — there's rarely anything meaningful to merge
/// in a handful of memories, and it's cheap to just skip the call rather
/// than burn a request on an answer that would come back empty anyway.
const MIN_MEMORIES_FOR_FUSION: usize = 4;
/// Same reasoning as `EXTRACTION_TEMPERATURE` — a one-shot analysis call,
/// not a creative response, forced low for consistent judgment about
/// what's actually redundant.
const FUSION_TEMPERATURE: f32 = 0.2;
/// Generous, not tight: a thinking-capable local model (Qwen3's reasoning
/// checkpoints, confirmed live) spends its budget on hidden
/// `reasoning_content` before ever emitting the JSON answer — too small a
/// cap here means `finish_reason: "length"` with empty `content`, which
/// looks identical to "no groups proposed" from the caller's side. Same
/// "not an artificially tight cap" reasoning as `TwitchService`/
/// `DiscordService` already apply to their own chat responses.
const FUSION_MAX_TOKENS: u32 = 4096;

#[async_trait]
pub trait MemoryManager: Send + Sync {
    async fn list_for_profile(&self, profile_id: &str) -> Result<Vec<Memory>, MemoryError>;
    async fn search(&self, profile_id: &str, query: &str, limit: i64) -> Result<Vec<Memory>, MemoryError>;
    /// Como `search`, pero solo con memorias de alcance público — lo único
    /// que puede usar un canal abierto a terceros (Twitch/Discord). Los
    /// canales del dueño (Chat, Llamada) usan `search`, que ve todo. Ver
    /// `Memory::scope`.
    async fn search_public(&self, profile_id: &str, query: &str, limit: i64) -> Result<Vec<Memory>, MemoryError>;
    async fn create(&self, input: NewMemory) -> Result<Memory, MemoryError>;
    async fn set_pinned(&self, id: &str, pinned: bool) -> Result<(), MemoryError>;
    /// Hace una memoria privada (solo Chat/Llamada) o pública (también
    /// Twitch/Discord) — siempre una decisión explícita del usuario; toda
    /// memoria nace privada. `scope` debe ser `MEMORY_SCOPE_PRIVATE` o
    /// `MEMORY_SCOPE_PUBLIC`.
    async fn set_scope(&self, id: &str, scope: &str) -> Result<(), MemoryError>;
    /// Lets the user correct a memory's title/text/category/importance —
    /// never touches `source`/`source_id`/`confidence` (see
    /// `MemoriesRepository::update`).
    async fn update(&self, id: &str, update: MemoryUpdate) -> Result<Memory, MemoryError>;
    async fn delete(&self, id: &str) -> Result<(), MemoryError>;

    /// Asks `provider` to analyze `messages` (the *new* tail since the last
    /// consolidation — see `Conversation::memory_consolidated_through`) and
    /// persists whatever candidates come back — memories and, best-effort,
    /// any real people mentioned (see `crates/memory/src/people.rs`).
    /// `source_id` is typically the conversation id, kept for
    /// traceability. Never partially fails: a response that doesn't parse
    /// returns an empty `Vec`, not an error — see
    /// `extraction::parse_extraction_response`. `language`
    /// (`SettingsService::language`) only localizes the `activity` log
    /// entry emitted on a parse failure — falls back to Spanish, same
    /// convention as `PromptComposer` (see its doc comment).
    async fn extract_and_store(
        &self,
        profile_id: &str,
        source_id: Option<&str>,
        messages: &[Message],
        provider: Arc<dyn LLMProvider>,
        model: &str,
        language: &str,
    ) -> Result<Vec<Memory>, MemoryError>;

    /// Asks `provider` to find groups of redundant/similar memories worth
    /// merging — a read-only proposal, never writes anything. The Memoria
    /// tab's "Fusionar memorias" button shows these to the user for review
    /// before any of them are actually applied (see `apply_fusion`).
    /// `language` follows `extract_and_store`'s own doc comment.
    async fn propose_fusion(&self, profile_id: &str, provider: Arc<dyn LLMProvider>, model: &str, language: &str) -> Result<Vec<FusionProposal>, MemoryError>;

    /// Applies exactly the groups the user accepted after reviewing
    /// `propose_fusion`'s output — for each, creates the merged memory and
    /// deletes the originals it replaces. Groups are independent of each
    /// other: one referencing an id that no longer exists doesn't affect
    /// the rest.
    async fn apply_fusion(&self, profile_id: &str, groups: Vec<FusionGroupInput>) -> Result<Vec<Memory>, MemoryError>;

    /// Recorre las memorias del perfil sin fila en `memory_embeddings` y
    /// las embebe, publicando `AppEvent::EmbeddingBackfillProgress`
    /// throttleado. Un fallo puntual se loguea y se saltea — salvo
    /// `EmbeddingError::Unavailable` (modelo no descargado), que corta de
    /// una porque reintentar por ítem nunca serviría. Devuelve la
    /// cantidad efectivamente embebida.
    async fn backfill_embeddings(&self, profile_id: &str) -> Result<usize, MemoryError>;

    /// El estado emocional actual del personaje — ver `docs/ADR/006-mood.md`.
    /// Un perfil sin fila todavía devuelve el baseline neutral (50/50), no
    /// un error.
    async fn get_mood(&self, profile_id: &str) -> Result<ProfileMood, MemoryError>;

    /// Botón de "reiniciar" manual — vuelve al estado neutral (50/50).
    async fn reset_mood(&self, profile_id: &str) -> Result<ProfileMood, MemoryError>;

    /// Si el personaje puede iniciar una conversación por su cuenta — ver
    /// docs/ADR/007-proactive-mode.md. Un perfil sin fila todavía devuelve
    /// desactivado, no un error.
    async fn proactive_settings(&self, profile_id: &str) -> Result<ProactiveSettings, MemoryError>;

    /// Prende/apaga el modo proactivo para un perfil — el toggle de
    /// Identidad > Personalidad.
    async fn set_proactive_enabled(&self, profile_id: &str, enabled: bool) -> Result<ProactiveSettings, MemoryError>;

    /// Registra que el motor de decisión efectivamente mandó un mensaje
    /// ahora mismo — alimenta el cooldown en `ProactiveService`.
    async fn mark_proactive_triggered(&self, profile_id: &str) -> Result<(), MemoryError>;
}

pub struct MemoryService {
    pool: DbPool,
    embeddings: Arc<dyn EmbeddingProvider>,
    event_bus: EventBus,
}

impl MemoryService {
    pub fn new(pool: DbPool, embeddings: Arc<dyn EmbeddingProvider>, event_bus: EventBus) -> Self {
        Self { pool, embeddings, event_bus }
    }

    fn repo(&self) -> MemoriesRepository<'_> {
        MemoriesRepository::new(&self.pool)
    }

    fn people_repo(&self) -> PeopleRepository<'_> {
        PeopleRepository::new(&self.pool)
    }

    fn embeddings_repo(&self) -> MemoryEmbeddingsRepository<'_> {
        MemoryEmbeddingsRepository::new(&self.pool)
    }

    fn mood_repo(&self) -> MoodRepository<'_> {
        MoodRepository::new(&self.pool)
    }

    fn proactive_repo(&self) -> ProactiveSettingsRepository<'_> {
        ProactiveSettingsRepository::new(&self.pool)
    }

    /// Búsqueda híbrida (FTS + semántica) compartida por `search` y
    /// `search_public`. `public_only` filtra por alcance dentro de las
    /// consultas mismas, no después, para que las memorias privadas no
    /// desplacen a las públicas del tope del ranking.
    async fn search_with_scope(&self, profile_id: &str, query: &str, limit: i64, public_only: bool) -> Result<Vec<Memory>, MemoryError> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let candidates = (limit.max(0) * 3).max(20);

        let fts_results = self.repo().search_scoped(profile_id, query, candidates, public_only).await?;

        let semantic_results = match self.embeddings.embed(query).await {
            Ok(vector) => self.repo().search_by_embedding_scoped(profile_id, &vector, candidates, public_only).await.unwrap_or_else(|err| {
                tracing::warn!(target: "system", "memory: búsqueda semántica falló, se usa solo FTS: {err}");
                Vec::new()
            }),
            Err(EmbeddingError::Unavailable(_)) => Vec::new(),
            Err(err) => {
                tracing::warn!(target: "system", "memory: no se pudo calcular el embedding de la consulta, se usa solo FTS: {err}");
                Vec::new()
            }
        };

        Ok(reciprocal_rank_fusion(&fts_results, &semantic_results, limit.max(0) as usize))
    }

    /// Fires a fully detached best-effort embed — never awaited, matching
    /// `run_generation`'s "background work must never block a request
    /// that already succeeded" stance for memory retrieval failures. A
    /// missing model or an inference failure silently no-ops: semantic
    /// search is an enhancement over FTS, never a hard dependency.
    fn spawn_embed(&self, memory: Memory) {
        let pool = self.pool.clone();
        let embeddings = self.embeddings.clone();
        tokio::spawn(async move {
            match embeddings.embed(&embedding_text(&memory)).await {
                Ok(vector) => {
                    if let Err(err) = MemoryEmbeddingsRepository::new(&pool).upsert(&memory.id, &vector, MODEL_VERSION).await {
                        tracing::warn!(target: "system", "memory: no se pudo guardar el embedding de '{}': {err}", memory.id);
                    }
                }
                Err(EmbeddingError::Unavailable(_)) => {}
                Err(err) => tracing::warn!(target: "system", "memory: no se pudo calcular el embedding de '{}': {err}", memory.id),
            }
        });
    }
}

/// Mismo texto que el que ve el modelo en el prompt de chat (ver
/// `AIOrchestrator::memory_section`) — título + resumen (o contenido si
/// no hay resumen). Que sea idéntico es lo que hace que la similitud
/// semántica tenga sentido acá.
fn embedding_text(memory: &Memory) -> String {
    let body = memory.summary.as_deref().filter(|s| !s.trim().is_empty()).unwrap_or(&memory.content);
    format!("{}\n{}", memory.title, body)
}

/// Reciprocal Rank Fusion — combina dos listas rankeadas de forma
/// independiente (BM25 de FTS5, similitud coseno de embeddings) sin
/// necesitar normalizar puntajes entre sí, ya que RRF solo le importa la
/// *posición* dentro de cada lista. `FTS_WEIGHT > SEMANTIC_WEIGHT` le da
/// a FTS una ventaja deliberada para que una coincidencia exacta de
/// término/nombre no quede enterrada bajo un resultado "parecido pero no
/// exacto".
const RRF_K: f64 = 60.0; // constante estándar de la literatura de RRF
const FTS_WEIGHT: f64 = 1.2;
const SEMANTIC_WEIGHT: f64 = 1.0;

fn reciprocal_rank_fusion(fts: &[Memory], semantic: &[Memory], limit: usize) -> Vec<Memory> {
    let mut scores: HashMap<String, f64> = HashMap::new();
    let mut by_id: HashMap<String, Memory> = HashMap::new();

    for (rank, memory) in fts.iter().enumerate() {
        *scores.entry(memory.id.clone()).or_insert(0.0) += FTS_WEIGHT / (RRF_K + rank as f64 + 1.0);
        by_id.entry(memory.id.clone()).or_insert_with(|| memory.clone());
    }
    for (rank, memory) in semantic.iter().enumerate() {
        *scores.entry(memory.id.clone()).or_insert(0.0) += SEMANTIC_WEIGHT / (RRF_K + rank as f64 + 1.0);
        by_id.entry(memory.id.clone()).or_insert_with(|| memory.clone());
    }

    let mut ranked: Vec<(String, f64)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked.into_iter().take(limit).filter_map(|(id, _)| by_id.remove(&id)).collect()
}

#[async_trait]
impl MemoryManager for MemoryService {
    async fn list_for_profile(&self, profile_id: &str) -> Result<Vec<Memory>, MemoryError> {
        Ok(self.repo().list_for_profile(profile_id).await?)
    }

    async fn search(&self, profile_id: &str, query: &str, limit: i64) -> Result<Vec<Memory>, MemoryError> {
        self.search_with_scope(profile_id, query, limit, false).await
    }

    async fn search_public(&self, profile_id: &str, query: &str, limit: i64) -> Result<Vec<Memory>, MemoryError> {
        self.search_with_scope(profile_id, query, limit, true).await
    }

    async fn set_scope(&self, id: &str, scope: &str) -> Result<(), MemoryError> {
        Ok(self.repo().set_scope(id, scope).await?)
    }

    async fn create(&self, input: NewMemory) -> Result<Memory, MemoryError> {
        let memory = self.repo().create(input).await?;
        self.spawn_embed(memory.clone());
        Ok(memory)
    }

    async fn set_pinned(&self, id: &str, pinned: bool) -> Result<(), MemoryError> {
        Ok(self.repo().set_pinned(id, pinned).await?)
    }

    async fn update(&self, id: &str, update: MemoryUpdate) -> Result<Memory, MemoryError> {
        let memory = self.repo().update(id, update).await?;
        self.spawn_embed(memory.clone());
        Ok(memory)
    }

    async fn delete(&self, id: &str) -> Result<(), MemoryError> {
        Ok(self.repo().delete(id).await?)
    }

    async fn extract_and_store(
        &self,
        profile_id: &str,
        source_id: Option<&str>,
        messages: &[Message],
        provider: Arc<dyn LLMProvider>,
        model: &str,
        language: &str,
    ) -> Result<Vec<Memory>, MemoryError> {
        if messages.is_empty() {
            return Ok(Vec::new());
        }

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage::text(MessageRole::System, system_prompt()),
                ChatMessage::text(MessageRole::User, format!("Conversación:\n{}", transcript(messages))),
            ],
            temperature: Some(EXTRACTION_TEMPERATURE),
            max_tokens: Some(EXTRACTION_MAX_TOKENS),
            tools: Vec::new(),
            context: Vec::new(),
            contains_stored_memories: false,
        };

        let mut stream = provider.stream_chat(request);
        let mut raw = String::new();
        while let Some(chunk) = stream.next().await {
            raw.push_str(&chunk?.delta);
        }

        let Some(response) = parse_extraction_response(&raw) else {
            tracing::warn!(
                target: "activity",
                category = "MEMORY",
                module = "extraction",
                action = "parse",
                status = "error",
                summary = %extraction_parse_failure_message(language),
            );
            return Ok(Vec::new());
        };

        let mut created = Vec::with_capacity(response.memories.len());
        for candidate in response.memories {
            let content = candidate.content.trim();
            if content.is_empty() {
                continue;
            }
            // Va por `self.create(...)` (el método del trait), no
            // `self.repo().create(...)` directo, para que cada memoria
            // extraída dispare el mismo embed de fondo que una creada a
            // mano — sin duplicar esa lógica acá.
            let memory = self
                .create(NewMemory {
                    profile_id: profile_id.to_string(),
                    title: candidate.resolved_title(),
                    content: content.to_string(),
                    summary: candidate.resolved_summary(),
                    category: candidate.normalized_category(),
                    importance: candidate.clamped_importance(),
                    confidence: candidate.clamped_confidence(),
                    source: "chat".to_string(),
                    source_id: source_id.map(str::to_string),
                })
                .await?;
            created.push(memory);
        }

        for person in response.people {
            let name = person.name.trim();
            if name.is_empty() || person.clamped_confidence() < PEOPLE_CONFIDENCE_THRESHOLD {
                continue;
            }
            if let Err(err) = self
                .people_repo()
                .upsert_by_name(name, person.trimmed_alias(), person.normalized_role(), "chat", MAX_ALIASES_PER_PERSON)
                .await
            {
                // Best-effort — a failure to register a name shouldn't
                // fail the whole extraction call over what's ultimately a
                // secondary benefit (better STT recognition), especially
                // since the memories above are already committed.
                tracing::warn!(target: "system", "memory: no se pudo registrar a '{name}' en Personas: {err}");
            }
        }

        // Siempre corre, aunque el modelo no haya mandado "mood" — así la
        // decadencia por tiempo transcurrido (ver `MoodRepository::apply_delta`)
        // pasa en cada consolidación, no solo cuando el modelo se acuerda
        // de incluir el campo.
        let (valence_delta, energy_delta) = response
            .mood
            .as_ref()
            .map(|m| (m.clamped_valence_delta(), m.clamped_energy_delta()))
            .unwrap_or((0, 0));
        match self.mood_repo().apply_delta(profile_id, valence_delta, energy_delta).await {
            Ok(mood) => self.event_bus.publish(AppEvent::MoodUpdated {
                profile_id: profile_id.to_string(),
                valence: mood.valence,
                energy: mood.energy,
            }),
            Err(err) => tracing::warn!(target: "system", "memory: no se pudo actualizar el mood de '{profile_id}': {err}"),
        }

        Ok(created)
    }

    async fn propose_fusion(&self, profile_id: &str, provider: Arc<dyn LLMProvider>, model: &str, language: &str) -> Result<Vec<FusionProposal>, MemoryError> {
        let memories = self.repo().list_for_profile(profile_id).await?;
        if memories.len() < MIN_MEMORIES_FOR_FUSION {
            return Ok(Vec::new());
        }

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage::text(MessageRole::System, fusion_system_prompt()),
                ChatMessage::text(MessageRole::User, format!("Memorias:\n{}", memories_listing(&memories))),
            ],
            temperature: Some(FUSION_TEMPERATURE),
            max_tokens: Some(FUSION_MAX_TOKENS),
            tools: Vec::new(),
            context: Vec::new(),
            contains_stored_memories: true,
        };

        let mut stream = provider.stream_chat(request);
        let mut raw = String::new();
        while let Some(chunk) = stream.next().await {
            raw.push_str(&chunk?.delta);
        }

        let Some(response) = parse_fusion_response(&raw) else {
            tracing::warn!(
                target: "activity",
                category = "MEMORY",
                module = "fusion",
                action = "parse",
                status = "error",
                summary = %fusion_parse_failure_message(language),
            );
            return Ok(Vec::new());
        };

        let mut proposals = Vec::with_capacity(response.groups.len());
        for group in response.groups {
            // Menos de dos ids no es una fusión — y cada id debe corresponder
            // a una memoria real de este perfil, nunca uno inventado por el
            // modelo (o ya borrado entre que se cargó la lista y ahora).
            let originals: Vec<Memory> = group.memory_ids.iter().filter_map(|id| memories.iter().find(|m| &m.id == id).cloned()).collect();
            if originals.len() < 2 || originals.len() != group.memory_ids.len() {
                continue;
            }
            // A model-agnostic safety net, independent of how well the model
            // followed the prompt: two memories can only be "genuinely the
            // same thing" (the bar the prompt itself sets) if they're the
            // same *kind* of thing. Confirmed live with more than one local
            // model that a weak one will otherwise merge a `fact` with an
            // unrelated `preference` (job + hobby) just because both were
            // in the list — a mistake no amount of prompt wording fixed
            // reliably, but this check catches for free regardless of
            // model quality, at zero cost to the genuinely-same-category
            // case fusion actually exists for.
            if originals.windows(2).any(|pair| pair[0].category != pair[1].category) {
                continue;
            }
            let content = group.content.trim();
            if content.is_empty() {
                continue;
            }
            proposals.push(FusionProposal {
                originals,
                title: group.resolved_title(),
                summary: group.resolved_summary(),
                content: content.to_string(),
                category: group.normalized_category(),
                importance: group.clamped_importance(),
            });
        }

        Ok(proposals)
    }

    async fn apply_fusion(&self, profile_id: &str, groups: Vec<FusionGroupInput>) -> Result<Vec<Memory>, MemoryError> {
        let mut merged = Vec::with_capacity(groups.len());
        for group in groups {
            if group.original_ids.len() < 2 {
                continue;
            }
            // La memoria fusionada solo es pública si TODAS las originales lo
            // eran: mezclar un dato privado en una pública lo expondría en
            // Twitch/Discord. Ante cualquier duda queda privada (el valor por
            // defecto) y el usuario puede hacerla pública a mano.
            let mut found_any = false;
            let mut all_public = true;
            for id in &group.original_ids {
                if let Some(original) = self.repo().get(id).await? {
                    found_any = true;
                    all_public &= original.scope == MEMORY_SCOPE_PUBLIC;
                }
            }
            let mut memory = self
                .create(NewMemory {
                    profile_id: profile_id.to_string(),
                    title: group.title,
                    content: group.content,
                    summary: group.summary,
                    category: group.category,
                    importance: group.importance.clamp(0, 100),
                    confidence: 100,
                    source: "fusion".to_string(),
                    source_id: Some(group.original_ids.join(",")),
                })
                .await?;
            if found_any && all_public {
                self.repo().set_scope(&memory.id, MEMORY_SCOPE_PUBLIC).await?;
                memory.scope = MEMORY_SCOPE_PUBLIC.to_string();
            }
            for id in &group.original_ids {
                if let Err(err) = self.delete(id).await {
                    tracing::warn!(target: "system", "memory: fusión creó '{}' pero no se pudo borrar el original '{id}': {err}", memory.id);
                }
            }
            merged.push(memory);
        }
        Ok(merged)
    }

    async fn backfill_embeddings(&self, profile_id: &str) -> Result<usize, MemoryError> {
        const PROGRESS_THROTTLE: Duration = Duration::from_millis(250);

        let missing = self.repo().missing_embeddings_for_profile(profile_id).await?;
        let total = missing.len();
        let mut embedded = 0usize;
        let mut last_emit = Instant::now();
        self.event_bus.publish(AppEvent::EmbeddingBackfillProgress { profile_id: profile_id.to_string(), embedded, total });

        for memory in missing {
            match self.embeddings.embed(&embedding_text(&memory)).await {
                Ok(vector) => {
                    let _ = self.embeddings_repo().upsert(&memory.id, &vector, MODEL_VERSION).await;
                    embedded += 1;
                }
                Err(EmbeddingError::Unavailable(message)) => {
                    self.event_bus.publish(AppEvent::EmbeddingBackfillError { profile_id: profile_id.to_string(), message });
                    return Ok(embedded);
                }
                Err(err) => {
                    tracing::warn!(target: "system", "memory: backfill: no se pudo embeber '{}': {err}", memory.id);
                }
            }
            if last_emit.elapsed() >= PROGRESS_THROTTLE || embedded == total {
                self.event_bus.publish(AppEvent::EmbeddingBackfillProgress { profile_id: profile_id.to_string(), embedded, total });
                last_emit = Instant::now();
            }
        }

        self.event_bus.publish(AppEvent::EmbeddingBackfillFinished { profile_id: profile_id.to_string(), embedded, total });
        Ok(embedded)
    }

    async fn get_mood(&self, profile_id: &str) -> Result<ProfileMood, MemoryError> {
        Ok(self.mood_repo().get(profile_id).await?)
    }

    async fn reset_mood(&self, profile_id: &str) -> Result<ProfileMood, MemoryError> {
        Ok(self.mood_repo().reset(profile_id).await?)
    }

    async fn proactive_settings(&self, profile_id: &str) -> Result<ProactiveSettings, MemoryError> {
        Ok(self.proactive_repo().get(profile_id).await?)
    }

    async fn set_proactive_enabled(&self, profile_id: &str, enabled: bool) -> Result<ProactiveSettings, MemoryError> {
        Ok(self.proactive_repo().set_enabled(profile_id, enabled).await?)
    }

    async fn mark_proactive_triggered(&self, profile_id: &str) -> Result<(), MemoryError> {
        Ok(self.proactive_repo().record_triggered(profile_id).await?)
    }
}

fn extraction_parse_failure_message(lang: &str) -> &'static str {
    match lang {
        "en" => "The model's response couldn't be parsed as memories",
        "pt" => "Não foi possível interpretar a resposta do modelo como memórias",
        "ja" => "モデルの応答を記憶として解釈できませんでした",
        "ko" => "모델의 응답을 기억으로 해석할 수 없습니다",
        "zh" => "无法将模型的回复解析为记忆",
        _ => "La respuesta del modelo no se pudo interpretar como memorias",
    }
}

fn fusion_parse_failure_message(lang: &str) -> &'static str {
    match lang {
        "en" => "The model's response couldn't be parsed as fusion groups",
        "pt" => "Não foi possível interpretar a resposta do modelo como grupos de fusão",
        "ja" => "モデルの応答を統合グループとして解釈できませんでした",
        "ko" => "모델의 응답을 병합 그룹으로 해석할 수 없습니다",
        "zh" => "无法将模型的回复解析为合并分组",
        _ => "La respuesta del modelo no se pudo interpretar como grupos de fusión",
    }
}

#[cfg(test)]
mod tests {
    use aivt_database::{MessageRole as DbMessageRole, MessageStatus};
    use aivt_provider_llm::{ChatChunk, ProviderError};
    use aivt_testing::MockLLMProvider;

    use super::*;

    /// Minimal `EmbeddingProvider` test double — local to this module
    /// since no other crate needs to fake embeddings yet (Chat/Twitch/
    /// Discord/Call only touch `MemoryManager::search`, already
    /// mockable via `MockMemoryManager`). Keeping it here also sidesteps
    /// any question about a dependency cycle with `aivt-testing` (which
    /// already depends on `aivt-memory` for `MockMemoryManager`).
    #[derive(Default)]
    struct FakeEmbeddings {
        calls: std::sync::Mutex<Vec<String>>,
        unavailable: bool,
    }

    impl FakeEmbeddings {
        fn unavailable() -> Self {
            Self { unavailable: true, ..Self::default() }
        }

        fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl EmbeddingProvider for FakeEmbeddings {
        async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
            self.calls.lock().unwrap().push(text.to_string());
            if self.unavailable {
                return Err(EmbeddingError::Unavailable("modelo no descargado".to_string()));
            }
            Ok(vec![1.0, 0.0])
        }
    }

    /// `spawn_embed` is fire-and-forget (`tokio::spawn`, never awaited by
    /// the caller) — same reason `crates/services/chat/src/service.rs`
    /// polls for its own background-task side effects instead of
    /// awaiting them directly.
    async fn wait_for_embed_calls(embeddings: &FakeEmbeddings, min_count: usize) {
        for _ in 0..200 {
            if embeddings.calls().len() >= min_count {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("timed out waiting for {min_count} embed() calls, got {}", embeddings.calls().len());
    }

    async fn test_service(embeddings: Arc<dyn EmbeddingProvider>) -> MemoryService {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        MemoryService::new(pool, embeddings, EventBus::new())
    }

    async fn seed_profile(service: &MemoryService) -> String {
        aivt_database::ProfilesRepository::new(&service.pool)
            .create(aivt_database::NewProfile { name: "Ciel".to_string(), nickname: None, description: None })
            .await
            .unwrap()
            .id
    }

    fn sample_message(role: DbMessageRole, content: &str) -> Message {
        Message {
            id: "m1".to_string(),
            conversation_id: "c1".to_string(),
            role,
            content: content.to_string(),
            status: MessageStatus::Complete,
            provider_id: None,
            model: None,
            input_tokens: None,
            output_tokens: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn extract_and_store_persists_every_valid_candidate() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let provider = Arc::new(MockLLMProvider::with_response([
            r#"[{"title": "Bebida favorita", "summary": "Le encanta el café", "content": "Le encanta el café",
                "category": "preference", "importance": 60, "confidence": 90},"#,
            r#"{"content": "Trabaja como diseñadora", "category": "fact"}]"#,
        ]));
        let messages = vec![
            sample_message(DbMessageRole::User, "Me encanta el café. Trabajo como diseñadora."),
            sample_message(DbMessageRole::Assistant, "¡Qué interesante!"),
        ];

        let created = service
            .extract_and_store(&profile_id, Some("c1"), &messages, provider, "mock-model", "es")
            .await
            .unwrap();

        assert_eq!(created.len(), 2);
        assert_eq!(created[0].title, "Bebida favorita");
        assert_eq!(created[0].summary.as_deref(), Some("Le encanta el café"));
        assert_eq!(created[0].content, "Le encanta el café");
        assert_eq!(created[0].importance, 60);
        assert_eq!(created[1].category, "fact");
        // The model didn't send a title for the second candidate — falls
        // back to (a truncation of) its content rather than an empty title.
        assert_eq!(created[1].title, "Trabaja como diseñadora");
        assert_eq!(created[0].source_id.as_deref(), Some("c1"));

        let listed = service.list_for_profile(&profile_id).await.unwrap();
        assert_eq!(listed.len(), 2);
    }

    #[tokio::test]
    async fn extract_and_store_applies_a_mood_delta_and_publishes_an_event() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let mut rx = service.event_bus.subscribe();
        let provider = Arc::new(MockLLMProvider::with_response([
            r#"{"memories": [], "people": [], "mood": {"valence_delta": 10, "energy_delta": -5}}"#,
        ]));
        let messages = vec![sample_message(DbMessageRole::User, "Qué día horrible tuve")];

        service.extract_and_store(&profile_id, None, &messages, provider, "mock-model", "es").await.unwrap();

        let mood = service.get_mood(&profile_id).await.unwrap();
        assert_eq!(mood.valence, 60);
        assert_eq!(mood.energy, 45);

        match rx.recv().await.unwrap() {
            AppEvent::MoodUpdated { profile_id: id, valence, energy } => {
                assert_eq!(id, profile_id);
                assert_eq!(valence, 60);
                assert_eq!(energy, 45);
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }

    #[tokio::test]
    async fn extract_and_store_still_runs_decay_when_the_model_omits_mood() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        // Un mood ya alejado del baseline, con `updated_at` viejo — si la
        // decadencia corre igual sin que el modelo mande "mood", debería
        // acercarse a 50 solo por el tiempo transcurrido.
        service.mood_repo().apply_delta(&profile_id, 40, 0).await.unwrap();
        sqlx::query("UPDATE profile_moods SET updated_at = ?1 WHERE profile_id = ?2")
            .bind((chrono::Utc::now() - chrono::Duration::hours(1000)).to_rfc3339())
            .bind(&profile_id)
            .execute(&service.pool)
            .await
            .unwrap();
        let provider = Arc::new(MockLLMProvider::with_response([r#"{"memories": [], "people": []}"#]));
        let messages = vec![sample_message(DbMessageRole::User, "Hola de nuevo")];

        service.extract_and_store(&profile_id, None, &messages, provider, "mock-model", "es").await.unwrap();

        let mood = service.get_mood(&profile_id).await.unwrap();
        assert_eq!(mood.valence, 50, "sin 'mood' en la respuesta, igual debería correr la decadencia por tiempo");
    }

    #[tokio::test]
    async fn an_empty_array_response_stores_nothing() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let provider = Arc::new(MockLLMProvider::with_response(["[]"]));
        let messages = vec![sample_message(DbMessageRole::User, "Hola")];

        let created = service.extract_and_store(&profile_id, None, &messages, provider, "mock-model", "es").await.unwrap();
        assert!(created.is_empty());
    }

    #[tokio::test]
    async fn an_unparseable_response_stores_nothing_and_does_not_error() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let provider = Arc::new(MockLLMProvider::with_response(["lo siento, no puedo ayudar"]));
        let messages = vec![sample_message(DbMessageRole::User, "Hola")];

        let created = service.extract_and_store(&profile_id, None, &messages, provider, "mock-model", "es").await.unwrap();
        assert!(created.is_empty());
    }

    #[tokio::test]
    async fn no_new_messages_skips_the_llm_call_entirely() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let provider = Arc::new(MockLLMProvider::with_error(ProviderError::ConnectionRefused));

        let created = service.extract_and_store(&profile_id, None, &[], provider, "mock-model", "es").await.unwrap();
        assert!(created.is_empty());
    }

    #[tokio::test]
    async fn a_provider_error_propagates() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let provider = Arc::new(MockLLMProvider::with_error(ProviderError::ConnectionRefused));
        let messages = vec![sample_message(DbMessageRole::User, "Hola")];

        let result = service.extract_and_store(&profile_id, None, &messages, provider, "mock-model", "es").await;
        assert!(matches!(result, Err(MemoryError::Llm(ProviderError::ConnectionRefused))));
    }

    /// `MockLLMProvider::with_response` only accepts `&'static str` pieces
    /// (fine for literal fixtures elsewhere in this file), but a fusion
    /// response needs real memory ids baked in via `format!` — this builds
    /// the same single-chunk-then-finished shape `with_response` would,
    /// straight from an owned `String`, with no leaking involved.
    fn provider_with_owned_response(text: String) -> Arc<MockLLMProvider> {
        Arc::new(MockLLMProvider::with_call_sequence([vec![Ok(ChatChunk {
            delta: text,
            finished: true,
            finish_reason: Some("stop".to_string()),
            tool_calls: Vec::new(),
        })]]))
    }

    async fn seed_memory(service: &MemoryService, profile_id: &str, title: &str, content: &str) -> Memory {
        service
            .create(NewMemory {
                profile_id: profile_id.to_string(),
                title: title.to_string(),
                content: content.to_string(),
                summary: None,
                category: "preference".to_string(),
                importance: 50,
                confidence: 100,
                source: "manual".to_string(),
                source_id: None,
            })
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn propose_fusion_skips_the_llm_call_when_there_are_too_few_memories() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        seed_memory(&service, &profile_id, "Café", "Le gusta el café").await;
        seed_memory(&service, &profile_id, "Ciclismo", "Le gusta el ciclismo").await;
        let provider = Arc::new(MockLLMProvider::with_error(ProviderError::ConnectionRefused));

        let proposals = service.propose_fusion(&profile_id, provider, "mock-model", "es").await.unwrap();
        assert!(proposals.is_empty());
    }

    #[tokio::test]
    async fn propose_fusion_resolves_full_memory_objects_for_a_valid_group() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let a = seed_memory(&service, &profile_id, "Café solo", "Le gusta el café solo").await;
        let b = seed_memory(&service, &profile_id, "Café con leche", "También le gusta el café con leche").await;
        seed_memory(&service, &profile_id, "Ciclismo", "Le gusta el ciclismo").await;
        seed_memory(&service, &profile_id, "Senderismo", "Le gusta el senderismo").await;
        let response = format!(
            r#"{{"groups": [{{"memory_ids": ["{}", "{}"], "title": "Café", "summary": "Le gusta el café",
                "content": "Le gusta el café, solo y con leche", "category": "preference", "importance": 65}}]}}"#,
            a.id, b.id
        );
        let provider = provider_with_owned_response(response);

        let proposals = service.propose_fusion(&profile_id, provider, "mock-model", "es").await.unwrap();

        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].title, "Café");
        assert_eq!(proposals[0].originals.len(), 2);
        let original_ids: Vec<&str> = proposals[0].originals.iter().map(|m| m.id.as_str()).collect();
        assert!(original_ids.contains(&a.id.as_str()));
        assert!(original_ids.contains(&b.id.as_str()));
    }

    #[tokio::test]
    async fn propose_fusion_discards_a_group_spanning_different_categories() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let a = service
            .create(NewMemory {
                profile_id: profile_id.clone(),
                title: "Trabaja como diseñadora".to_string(),
                content: "Trabaja como diseñadora gráfica".to_string(),
                summary: None,
                category: "fact".to_string(),
                importance: 50,
                confidence: 100,
                source: "manual".to_string(),
                source_id: None,
            })
            .await
            .unwrap();
        let b = seed_memory(&service, &profile_id, "Senderismo", "Le gusta el senderismo").await;
        seed_memory(&service, &profile_id, "Ciclismo", "Le gusta el ciclismo").await;
        seed_memory(&service, &profile_id, "Natación", "Le gusta la natación").await;
        let response = format!(r#"{{"groups": [{{"memory_ids": ["{}", "{}"], "content": "algo"}}]}}"#, a.id, b.id);
        let provider = provider_with_owned_response(response);

        let proposals = service.propose_fusion(&profile_id, provider, "mock-model", "es").await.unwrap();
        assert!(proposals.is_empty(), "un hecho y una preferencia no son 'genuinamente lo mismo', sin importar qué proponga el modelo");
    }

    #[tokio::test]
    async fn propose_fusion_discards_a_group_referencing_an_unknown_id() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let a = seed_memory(&service, &profile_id, "Café", "Le gusta el café").await;
        seed_memory(&service, &profile_id, "Ciclismo", "Le gusta el ciclismo").await;
        seed_memory(&service, &profile_id, "Senderismo", "Le gusta el senderismo").await;
        seed_memory(&service, &profile_id, "Natación", "Le gusta la natación").await;
        let response = format!(r#"{{"groups": [{{"memory_ids": ["{}", "no-existe"], "content": "algo"}}]}}"#, a.id);
        let provider = provider_with_owned_response(response);

        let proposals = service.propose_fusion(&profile_id, provider, "mock-model", "es").await.unwrap();
        assert!(proposals.is_empty(), "un id inexistente debe descartar el grupo entero, no fusionar parcialmente");
    }

    #[tokio::test]
    async fn propose_fusion_discards_a_group_with_only_one_id() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let a = seed_memory(&service, &profile_id, "Café", "Le gusta el café").await;
        seed_memory(&service, &profile_id, "Ciclismo", "Le gusta el ciclismo").await;
        seed_memory(&service, &profile_id, "Senderismo", "Le gusta el senderismo").await;
        seed_memory(&service, &profile_id, "Natación", "Le gusta la natación").await;
        let response = format!(r#"{{"groups": [{{"memory_ids": ["{}"], "content": "algo"}}]}}"#, a.id);
        let provider = provider_with_owned_response(response);

        let proposals = service.propose_fusion(&profile_id, provider, "mock-model", "es").await.unwrap();
        assert!(proposals.is_empty(), "un solo id no es una fusión");
    }

    #[tokio::test]
    async fn apply_fusion_creates_the_merged_memory_and_deletes_the_originals() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let a = seed_memory(&service, &profile_id, "Café solo", "Le gusta el café solo").await;
        let b = seed_memory(&service, &profile_id, "Café con leche", "También le gusta el café con leche").await;

        let merged = service
            .apply_fusion(
                &profile_id,
                vec![FusionGroupInput {
                    original_ids: vec![a.id.clone(), b.id.clone()],
                    title: "Café".to_string(),
                    summary: Some("Le gusta el café".to_string()),
                    content: "Le gusta el café, solo y con leche".to_string(),
                    category: "preference".to_string(),
                    importance: 65,
                }],
            )
            .await
            .unwrap();

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].source, "fusion");
        assert_eq!(merged[0].source_id.as_deref(), Some(format!("{},{}", a.id, b.id).as_str()));

        let remaining = service.list_for_profile(&profile_id).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, merged[0].id);
    }

    fn fusion_group(a: &Memory, b: &Memory) -> FusionGroupInput {
        FusionGroupInput {
            original_ids: vec![a.id.clone(), b.id.clone()],
            title: "Café".to_string(),
            summary: None,
            content: "Le gusta el café".to_string(),
            category: "preference".to_string(),
            importance: 60,
        }
    }

    #[tokio::test]
    async fn search_public_only_returns_what_the_owner_made_public() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let private = seed_memory(&service, &profile_id, "Café secreto", "Toma café a escondidas").await;
        let public = seed_memory(&service, &profile_id, "Café favorito", "Le gusta el café con leche").await;
        service.set_scope(&public.id, MEMORY_SCOPE_PUBLIC).await.unwrap();

        let owner_view = service.search(&profile_id, "café", 10).await.unwrap();
        assert_eq!(owner_view.len(), 2, "Chat y Llamada ven todo");

        let public_view = service.search_public(&profile_id, "café", 10).await.unwrap();
        assert_eq!(public_view.len(), 1);
        assert_eq!(public_view[0].id, public.id);
        assert!(public_view.iter().all(|m| m.id != private.id), "una memoria privada nunca llega a un canal público");
    }

    #[tokio::test]
    async fn search_public_finds_nothing_while_every_memory_is_still_private() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        seed_memory(&service, &profile_id, "Café", "Le gusta el café").await;

        assert!(service.search_public(&profile_id, "café", 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_fusion_of_only_public_memories_stays_public() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let a = seed_memory(&service, &profile_id, "Café solo", "Le gusta el café solo").await;
        let b = seed_memory(&service, &profile_id, "Café con leche", "Le gusta el café con leche").await;
        service.set_scope(&a.id, MEMORY_SCOPE_PUBLIC).await.unwrap();
        service.set_scope(&b.id, MEMORY_SCOPE_PUBLIC).await.unwrap();

        let merged = service.apply_fusion(&profile_id, vec![fusion_group(&a, &b)]).await.unwrap();

        assert_eq!(merged[0].scope, MEMORY_SCOPE_PUBLIC);
        assert_eq!(service.search_public(&profile_id, "café", 10).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_fusion_that_mixes_in_a_private_memory_becomes_private() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let public = seed_memory(&service, &profile_id, "Café solo", "Le gusta el café solo").await;
        let private = seed_memory(&service, &profile_id, "Café secreto", "Toma café a escondidas").await;
        service.set_scope(&public.id, MEMORY_SCOPE_PUBLIC).await.unwrap();

        let merged = service.apply_fusion(&profile_id, vec![fusion_group(&public, &private)]).await.unwrap();

        assert_eq!(merged[0].scope, "private", "mezclar un dato privado no puede exponerlo");
        assert!(service.search_public(&profile_id, "café", 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn apply_fusion_still_creates_the_merged_memory_when_an_original_is_already_gone() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;
        let a = seed_memory(&service, &profile_id, "Café", "Le gusta el café").await;

        let merged = service
            .apply_fusion(
                &profile_id,
                vec![FusionGroupInput {
                    original_ids: vec![a.id.clone(), "ya-no-existe".to_string()],
                    title: "Café".to_string(),
                    summary: None,
                    content: "Le gusta el café".to_string(),
                    category: "preference".to_string(),
                    importance: 50,
                }],
            )
            .await
            .unwrap();

        assert_eq!(merged.len(), 1);
    }

    #[tokio::test]
    async fn search_and_manual_create_and_pin_and_delete_all_work() {
        let service = test_service(Arc::new(FakeEmbeddings::default())).await;
        let profile_id = seed_profile(&service).await;

        let created = service
            .create(NewMemory {
                profile_id: profile_id.clone(),
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

        let found = service.search(&profile_id, "senderismo", 10).await.unwrap();
        assert_eq!(found.len(), 1);

        service.set_pinned(&created.id, true).await.unwrap();
        assert!(service.list_for_profile(&profile_id).await.unwrap()[0].is_pinned);

        service.delete(&created.id).await.unwrap();
        assert!(service.list_for_profile(&profile_id).await.unwrap().is_empty());
    }

    fn sample_new_memory(profile_id: &str) -> NewMemory {
        NewMemory {
            profile_id: profile_id.to_string(),
            title: "Senderismo".to_string(),
            content: "Le gusta el senderismo".to_string(),
            summary: Some("Le gusta caminar en la montaña".to_string()),
            category: "preference".to_string(),
            importance: 70,
            confidence: 100,
            source: "manual".to_string(),
            source_id: None,
        }
    }

    #[tokio::test]
    async fn creating_a_memory_eventually_embeds_it_with_title_and_summary() {
        let embeddings = Arc::new(FakeEmbeddings::default());
        let service = test_service(embeddings.clone()).await;
        let profile_id = seed_profile(&service).await;

        service.create(sample_new_memory(&profile_id)).await.unwrap();

        wait_for_embed_calls(&embeddings, 1).await;
        assert_eq!(embeddings.calls()[0], "Senderismo\nLe gusta caminar en la montaña");
    }

    #[tokio::test]
    async fn updating_a_memory_re_embeds_with_the_new_text() {
        let embeddings = Arc::new(FakeEmbeddings::default());
        let service = test_service(embeddings.clone()).await;
        let profile_id = seed_profile(&service).await;
        let created = service.create(sample_new_memory(&profile_id)).await.unwrap();
        wait_for_embed_calls(&embeddings, 1).await;

        service
            .update(
                &created.id,
                MemoryUpdate {
                    title: "Ciclismo".to_string(),
                    content: "Le gusta el ciclismo".to_string(),
                    summary: None,
                    category: "preference".to_string(),
                    importance: 70,
                },
            )
            .await
            .unwrap();

        wait_for_embed_calls(&embeddings, 2).await;
        // Sin resumen esta vez — cae al contenido, mismo criterio que
        // `AIOrchestrator::memory_section`.
        assert_eq!(embeddings.calls()[1], "Ciclismo\nLe gusta el ciclismo");
    }

    #[tokio::test]
    async fn an_unavailable_embedding_model_does_not_fail_creating_a_memory() {
        let service = test_service(Arc::new(FakeEmbeddings::unavailable())).await;
        let profile_id = seed_profile(&service).await;

        let result = service.create(sample_new_memory(&profile_id)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn backfill_embeddings_embeds_only_missing_memories_and_publishes_events() {
        let embeddings = Arc::new(FakeEmbeddings::default());
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        let event_bus = EventBus::new();
        let mut rx = event_bus.subscribe();
        let service = MemoryService::new(pool, embeddings.clone(), event_bus);
        let profile_id = seed_profile(&service).await;

        // Ya embebida antes del backfill (no debería volver a calcularse).
        let already_embedded = service.create(sample_new_memory(&profile_id)).await.unwrap();
        wait_for_embed_calls(&embeddings, 1).await;
        // Simula una memoria creada antes de que el modelo estuviera
        // descargado — insertada directo por el repo, sin pasar por
        // `create` (que hubiera disparado el embed).
        let pending = service.repo().create(sample_new_memory(&profile_id)).await.unwrap();
        assert_ne!(already_embedded.id, pending.id);

        let embedded_count = service.backfill_embeddings(&profile_id).await.unwrap();
        assert_eq!(embedded_count, 1);

        assert!(service.repo().missing_embeddings_for_profile(&profile_id).await.unwrap().is_empty());

        assert!(matches!(rx.recv().await.unwrap(), AppEvent::EmbeddingBackfillProgress { embedded: 0, total: 1, .. }));
        assert!(matches!(rx.recv().await.unwrap(), AppEvent::EmbeddingBackfillProgress { embedded: 1, total: 1, .. }));
        assert!(matches!(rx.recv().await.unwrap(), AppEvent::EmbeddingBackfillFinished { embedded: 1, total: 1, .. }));
    }

    #[tokio::test]
    async fn backfill_embeddings_stops_and_reports_when_the_model_is_unavailable() {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        let event_bus = EventBus::new();
        let mut rx = event_bus.subscribe();
        let service = MemoryService::new(pool, Arc::new(FakeEmbeddings::unavailable()), event_bus);
        let profile_id = seed_profile(&service).await;
        service.repo().create(sample_new_memory(&profile_id)).await.unwrap();

        let embedded_count = service.backfill_embeddings(&profile_id).await.unwrap();
        assert_eq!(embedded_count, 0);

        assert!(matches!(rx.recv().await.unwrap(), AppEvent::EmbeddingBackfillProgress { embedded: 0, total: 1, .. }));
        assert!(matches!(rx.recv().await.unwrap(), AppEvent::EmbeddingBackfillError { .. }));
    }

    fn fake_memory(id: &str) -> Memory {
        Memory {
            id: id.to_string(),
            profile_id: "p1".to_string(),
            title: "título".to_string(),
            content: "contenido".to_string(),
            summary: None,
            category: "fact".to_string(),
            importance: 50,
            confidence: 50,
            source: "manual".to_string(),
            source_id: None,
            is_pinned: false,
            scope: "private".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            last_accessed_at: None,
            access_count: 0,
        }
    }

    #[test]
    fn reciprocal_rank_fusion_with_only_fts_results_preserves_their_order() {
        let fts = vec![fake_memory("a"), fake_memory("b")];
        let result = reciprocal_rank_fusion(&fts, &[], 10);
        assert_eq!(result.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn reciprocal_rank_fusion_with_only_semantic_results_preserves_their_order() {
        let semantic = vec![fake_memory("a"), fake_memory("b")];
        let result = reciprocal_rank_fusion(&[], &semantic, 10);
        assert_eq!(result.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn reciprocal_rank_fusion_ranks_a_memory_present_in_both_lists_above_one_present_in_only_one() {
        let fts = vec![fake_memory("both"), fake_memory("fts_only")];
        let semantic = vec![fake_memory("semantic_only"), fake_memory("both")];

        let result = reciprocal_rank_fusion(&fts, &semantic, 10);

        assert_eq!(result[0].id, "both", "presente en ambas listas debe rankear primero");
    }

    #[test]
    fn reciprocal_rank_fusion_breaks_a_rank_tie_in_favor_of_fts() {
        let fts = vec![fake_memory("fts_top")];
        let semantic = vec![fake_memory("semantic_top")];

        let result = reciprocal_rank_fusion(&fts, &semantic, 10);

        assert_eq!(result[0].id, "fts_top", "FTS_WEIGHT > SEMANTIC_WEIGHT debe desempatar a su favor");
    }

    #[test]
    fn reciprocal_rank_fusion_respects_the_limit() {
        let fts = vec![fake_memory("a"), fake_memory("b"), fake_memory("c")];
        let result = reciprocal_rank_fusion(&fts, &[], 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn reciprocal_rank_fusion_with_both_lists_empty_returns_nothing() {
        assert!(reciprocal_rank_fusion(&[], &[], 10).is_empty());
    }

    #[tokio::test]
    async fn search_with_an_empty_query_returns_nothing_without_calling_embed() {
        let embeddings = Arc::new(FakeEmbeddings::default());
        let service = test_service(embeddings.clone()).await;
        let profile_id = seed_profile(&service).await;

        let results = service.search(&profile_id, "   ", 10).await.unwrap();

        assert!(results.is_empty());
        assert!(embeddings.calls().is_empty(), "una consulta vacía no debería llegar a pedir un embedding");
    }

    #[tokio::test]
    async fn search_degrades_to_fts_only_when_the_embedding_model_is_unavailable() {
        let service = test_service(Arc::new(FakeEmbeddings::unavailable())).await;
        let profile_id = seed_profile(&service).await;
        service
            .repo()
            .create(NewMemory {
                profile_id: profile_id.clone(),
                title: "Senderismo".to_string(),
                content: "Le gusta el senderismo".to_string(),
                summary: None,
                category: "preference".to_string(),
                importance: 50,
                confidence: 80,
                source: "manual".to_string(),
                source_id: None,
            })
            .await
            .unwrap();

        let results = service.search(&profile_id, "senderismo", 10).await.unwrap();

        assert_eq!(results.len(), 1, "sin modelo disponible, debe seguir encontrando por FTS5 como siempre");
    }

    #[tokio::test]
    async fn search_finds_a_memory_via_semantic_similarity_with_no_shared_words() {
        // `FakeEmbeddings::embed` por defecto devuelve el mismo vector fijo
        // para cualquier texto — se aprovecha eso para simular "la consulta
        // y esta memoria son semánticamente cercanas" sin depender del
        // modelo real.
        let embeddings = Arc::new(FakeEmbeddings::default());
        let service = test_service(embeddings.clone()).await;
        let profile_id = seed_profile(&service).await;

        let memory = service
            .repo()
            .create(NewMemory {
                profile_id: profile_id.clone(),
                title: "Postre favorito".to_string(),
                content: "Le encanta el chocolate con almendras".to_string(),
                summary: None,
                category: "preference".to_string(),
                importance: 50,
                confidence: 80,
                source: "manual".to_string(),
                source_id: None,
            })
            .await
            .unwrap();
        service.embeddings_repo().upsert(&memory.id, &[1.0, 0.0], "test-model").await.unwrap();

        // Ninguna palabra de esta consulta aparece en el título/contenido
        // de la memoria — si esto la encuentra, es la ruta semántica, no FTS.
        let results = service.search(&profile_id, "clima en Bariloche mañana", 10).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, memory.id);
    }
}
