use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use aivt_database::{Memory, MemoryUpdate, Message, NewMemory, ProactiveSettings, ProfileMood};
use aivt_memory::{FusionGroupInput, FusionProposal, MemoryError, MemoryManager};
use aivt_provider_llm::LLMProvider;
use uuid::Uuid;

/// One call to `extract_and_store`, recorded so tests can assert
/// consolidation actually fired (or didn't) without needing a real LLM.
#[derive(Debug, Clone)]
pub struct ExtractCall {
    pub profile_id: String,
    pub source_id: Option<String>,
    pub message_count: usize,
}

/// In-memory `MemoryManager` — `search`/`list_for_profile` both return
/// whatever's configured via `with_search_results` (the mock doesn't run
/// real FTS ranking; it's a stand-in for "memory retrieval found this"),
/// and `extract_and_store` returns a fixed result while recording every
/// call for assertions, rather than talking to an `LLMProvider` for real.
#[derive(Default)]
pub struct MockMemoryManager {
    memories: Mutex<Vec<Memory>>,
    extract_calls: Mutex<Vec<ExtractCall>>,
    extract_result: Mutex<Vec<Memory>>,
    fusion_proposals: Mutex<Vec<FusionProposal>>,
    /// Every query string `search` was called with — the mock ignores it
    /// when deciding what to return (see `with_search_results`), but tests
    /// still need to assert *what was asked for*, e.g. that a retrieval
    /// query spans more than just the latest message.
    search_queries: Mutex<Vec<String>>,
}

impl MockMemoryManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_search_results(memories: Vec<Memory>) -> Self {
        Self { memories: Mutex::new(memories), ..Default::default() }
    }

    pub fn set_extract_result(&self, memories: Vec<Memory>) {
        *self.extract_result.lock().expect("mock mutex poisoned") = memories;
    }

    pub fn extract_calls(&self) -> Vec<ExtractCall> {
        self.extract_calls.lock().expect("mock mutex poisoned").clone()
    }

    pub fn set_fusion_proposals(&self, proposals: Vec<FusionProposal>) {
        *self.fusion_proposals.lock().expect("mock mutex poisoned") = proposals;
    }

    pub fn search_queries(&self) -> Vec<String> {
        self.search_queries.lock().expect("mock mutex poisoned").clone()
    }
}

fn fake_memory(profile_id: &str, title: &str, content: &str) -> Memory {
    Memory {
        id: Uuid::new_v4().to_string(),
        profile_id: profile_id.to_string(),
        title: title.to_string(),
        content: content.to_string(),
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

#[async_trait]
impl MemoryManager for MockMemoryManager {
    async fn list_for_profile(&self, _profile_id: &str) -> Result<Vec<Memory>, MemoryError> {
        Ok(self.memories.lock().expect("mock mutex poisoned").clone())
    }

    async fn search(&self, _profile_id: &str, query: &str, limit: i64) -> Result<Vec<Memory>, MemoryError> {
        self.search_queries.lock().expect("mock mutex poisoned").push(query.to_string());
        let memories = self.memories.lock().expect("mock mutex poisoned");
        Ok(memories.iter().take(limit.max(0) as usize).cloned().collect())
    }

    async fn search_public(&self, _profile_id: &str, query: &str, limit: i64) -> Result<Vec<Memory>, MemoryError> {
        self.search_queries.lock().expect("mock mutex poisoned").push(query.to_string());
        let memories = self.memories.lock().expect("mock mutex poisoned");
        Ok(memories.iter().filter(|m| m.scope == "public").take(limit.max(0) as usize).cloned().collect())
    }

    async fn set_scope(&self, id: &str, scope: &str) -> Result<(), MemoryError> {
        if let Some(memory) = self.memories.lock().expect("mock mutex poisoned").iter_mut().find(|m| m.id == id) {
            memory.scope = scope.to_string();
        }
        Ok(())
    }

    async fn create(&self, input: NewMemory) -> Result<Memory, MemoryError> {
        let memory = fake_memory(&input.profile_id, &input.title, &input.content);
        self.memories.lock().expect("mock mutex poisoned").push(memory.clone());
        Ok(memory)
    }

    async fn set_pinned(&self, id: &str, pinned: bool) -> Result<(), MemoryError> {
        if let Some(memory) = self.memories.lock().expect("mock mutex poisoned").iter_mut().find(|m| m.id == id) {
            memory.is_pinned = pinned;
        }
        Ok(())
    }

    async fn update(&self, id: &str, update: MemoryUpdate) -> Result<Memory, MemoryError> {
        let mut memories = self.memories.lock().expect("mock mutex poisoned");
        let memory = memories.iter_mut().find(|m| m.id == id).expect("update called on unknown mock memory id");
        memory.title = update.title;
        memory.content = update.content;
        memory.summary = update.summary;
        memory.category = update.category;
        memory.importance = update.importance;
        Ok(memory.clone())
    }

    async fn delete(&self, id: &str) -> Result<(), MemoryError> {
        self.memories.lock().expect("mock mutex poisoned").retain(|m| m.id != id);
        Ok(())
    }

    async fn extract_and_store(
        &self,
        profile_id: &str,
        source_id: Option<&str>,
        messages: &[Message],
        _provider: Arc<dyn LLMProvider>,
        _model: &str,
        _language: &str,
    ) -> Result<Vec<Memory>, MemoryError> {
        self.extract_calls.lock().expect("mock mutex poisoned").push(ExtractCall {
            profile_id: profile_id.to_string(),
            source_id: source_id.map(str::to_string),
            message_count: messages.len(),
        });
        let created = self.extract_result.lock().expect("mock mutex poisoned").clone();
        self.memories.lock().expect("mock mutex poisoned").extend(created.iter().cloned());
        Ok(created)
    }

    async fn propose_fusion(&self, _profile_id: &str, _provider: Arc<dyn LLMProvider>, _model: &str, _language: &str) -> Result<Vec<FusionProposal>, MemoryError> {
        Ok(self.fusion_proposals.lock().expect("mock mutex poisoned").clone())
    }

    async fn apply_fusion(&self, profile_id: &str, groups: Vec<FusionGroupInput>) -> Result<Vec<Memory>, MemoryError> {
        let mut merged = Vec::with_capacity(groups.len());
        for group in groups {
            let memory = fake_memory(profile_id, &group.title, &group.content);
            let mut memories = self.memories.lock().expect("mock mutex poisoned");
            memories.retain(|m| !group.original_ids.contains(&m.id));
            memories.push(memory.clone());
            merged.push(memory);
        }
        Ok(merged)
    }

    async fn backfill_embeddings(&self, _profile_id: &str) -> Result<usize, MemoryError> {
        Ok(0)
    }

    async fn get_mood(&self, profile_id: &str) -> Result<ProfileMood, MemoryError> {
        Ok(ProfileMood::baseline(profile_id))
    }

    async fn reset_mood(&self, profile_id: &str) -> Result<ProfileMood, MemoryError> {
        Ok(ProfileMood::baseline(profile_id))
    }

    async fn proactive_settings(&self, profile_id: &str) -> Result<ProactiveSettings, MemoryError> {
        Ok(ProactiveSettings::disabled(profile_id))
    }

    async fn set_proactive_enabled(&self, profile_id: &str, enabled: bool) -> Result<ProactiveSettings, MemoryError> {
        Ok(ProactiveSettings { profile_id: profile_id.to_string(), enabled, last_triggered_at: None })
    }

    async fn mark_proactive_triggered(&self, _profile_id: &str) -> Result<(), MemoryError> {
        Ok(())
    }
}
