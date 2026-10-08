use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use aivt_database::{ConversationsRepository, DbPool, ProfilesRepository};
use aivt_memory::MemoryManager;
use aivt_orchestrator::{MoodPromptBuilder, PromptComposer};
use aivt_provider_llm::{ChatMessage, ChatRequest, LLMProvider, MessageRole};
use aivt_secrets::SecretStore;
use aivt_service_chat::ChatService;
use aivt_service_manager::ManagedService;
use aivt_service_settings::SettingsService;

use crate::decision::{self, DecisionContext};

/// Cada cuánto se evalúa si corresponde un mensaje proactivo, mientras la
/// app está abierta — no hay bandeja del sistema ni proceso en segundo
/// plano una vez cerrada la ventana (ver docs/ADR/007-proactive-mode.md).
const CHECK_INTERVAL: Duration = Duration::from_secs(15 * 60);
/// Silencio mínimo desde el último mensaje antes de considerar hablar —
/// evita interrumpir una charla que sigue activa.
const MIN_INACTIVITY: Duration = Duration::from_secs(90 * 60);
/// Tiempo mínimo entre dos mensajes proactivos consecutivos, sin importar
/// cuánta inactividad haya — evita que el personaje "spamee".
const MIN_COOLDOWN: Duration = Duration::from_secs(4 * 60 * 60);
const PROVIDER_LABEL: &str = "local";

struct Inner {
    pool: DbPool,
    secrets: Arc<dyn SecretStore>,
    memory: Arc<dyn MemoryManager>,
    chat: ChatService,
    /// Sincronizado desde el frontend (`proactive_set_active_profile`),
    /// mismo patrón que `CallService`/`TwitchService::Inner::active_profile_id`
    /// — el backend nunca lo adivina.
    active_profile_id: RwLock<Option<String>>,
    cancellation: Mutex<Option<CancellationToken>>,
}

impl Inner {
    fn settings(&self) -> SettingsService {
        SettingsService::new(self.pool.clone(), self.secrets.clone())
    }

    /// Corre en cada tick del loop de `start()`. Resuelve la conexión LLM
    /// actual (puede cambiar en cualquier momento desde Configuración, así
    /// que se lee fresca cada vez, igual que
    /// `CallService::Inner::settings()`) y delega en `evaluate_with` — sin
    /// conexión configurada, no hay nada con qué decidir.
    async fn evaluate_once(&self) {
        let Some(profile_id) = self.active_profile_id.read().await.clone() else { return };

        let connection = match self.settings().llm_connection().await {
            Ok(Some(connection)) => connection,
            Ok(None) => return,
            Err(err) => {
                tracing::warn!(target: "system", "proactive: no se pudo leer la conexión LLM: {err}");
                return;
            }
        };
        let provider = match self.settings().build_llm_provider(&self.chat.event_bus()).await {
            Ok(provider) => provider,
            Err(err) => {
                tracing::warn!(target: "system", "proactive: no se pudo armar el proveedor de IA: {err}");
                return;
            }
        };

        self.evaluate_with(&profile_id, provider, &connection.model).await;
    }

    /// El núcleo testeable: dado un `provider`/`model` ya resueltos,
    /// decide si corresponde un mensaje proactivo y lo manda. Separado de
    /// `evaluate_once` para que los tests nunca necesiten una conexión LLM
    /// real — mismo motivo por el que `MemoryManager::extract_and_store`
    /// recibe `provider`/`model` como parámetros en vez de resolverlos
    /// internamente.
    async fn evaluate_with(&self, profile_id: &str, provider: Arc<dyn LLMProvider>, model: &str) {
        let profile = match ProfilesRepository::new(&self.pool).get(profile_id).await {
            Ok(Some(profile)) if !profile.is_archived => profile,
            Ok(_) => return,
            Err(err) => {
                tracing::warn!(target: "system", "proactive: no se pudo leer el perfil '{profile_id}': {err}");
                return;
            }
        };

        let settings = match self.memory.proactive_settings(profile_id).await {
            Ok(settings) => settings,
            Err(err) => {
                tracing::warn!(target: "system", "proactive: no se pudo leer la configuración de '{profile_id}': {err}");
                return;
            }
        };
        if !settings.enabled {
            return;
        }
        if let Some(last_triggered_at) = &settings.last_triggered_at {
            if hours_since(last_triggered_at) * 3600.0 < MIN_COOLDOWN.as_secs_f64() {
                return;
            }
        }

        let conversations = match ConversationsRepository::new(&self.pool).list_for_profile(profile_id).await {
            Ok(conversations) => conversations,
            Err(err) => {
                tracing::warn!(target: "system", "proactive: no se pudieron listar conversaciones de '{profile_id}': {err}");
                return;
            }
        };
        // v1 no abre conversaciones en frío — solo continúa relaciones ya
        // existentes (ver "Fuera de alcance" en el ADR).
        let Some(latest) = conversations.first() else { return };
        let hours_since_last_message = hours_since(&latest.updated_at);
        if hours_since_last_message * 3600.0 < MIN_INACTIVITY.as_secs_f64() {
            return;
        }

        let mood = self.memory.get_mood(profile_id).await.ok();
        let memories = self.memory.list_for_profile(profile_id).await.unwrap_or_default();
        let language = self.settings().language().await.unwrap_or_else(|_| "es".to_string());

        let mut character_context = PromptComposer::compose(&profile, &language);
        if let Some(mood) = &mood {
            let mood_text = MoodPromptBuilder::build(mood, &language);
            if !mood_text.is_empty() {
                character_context.push_str("\n\n");
                character_context.push_str(&mood_text);
            }
        }

        let decision_ctx = DecisionContext {
            character_context,
            hours_since_last_message,
            now: chrono::Utc::now(),
            initiate_conversation_tendency: profile.personality.initiate_conversation_tendency,
            memories: &memories,
        };

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage::text(MessageRole::System, decision::system_prompt()),
                ChatMessage::text(MessageRole::User, decision::user_prompt(&decision_ctx)),
            ],
            temperature: Some(decision::DECISION_TEMPERATURE),
            max_tokens: Some(decision::DECISION_MAX_TOKENS),
            tools: Vec::new(),
            context: Vec::new(),
            contains_stored_memories: false,
        };

        let mut stream = provider.stream_chat(request);
        let mut raw = String::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => raw.push_str(&chunk.delta),
                Err(err) => {
                    tracing::warn!(target: "system", "proactive: el modelo falló para '{profile_id}': {err}");
                    return;
                }
            }
        }

        let Some(decision) = decision::parse_decision(&raw) else {
            tracing::warn!(target: "system", "proactive: no se pudo interpretar la decisión del modelo para '{profile_id}'");
            return;
        };
        let Some(message) = decision.resolved_message() else { return };

        if let Err(err) = self.chat.send_proactive_message(&profile, message, PROVIDER_LABEL, model, None).await {
            tracing::warn!(target: "system", "proactive: no se pudo enviar el mensaje a '{profile_id}': {err}");
            return;
        }
        if let Err(err) = self.memory.mark_proactive_triggered(profile_id).await {
            tracing::warn!(target: "system", "proactive: no se pudo registrar el disparo para '{profile_id}': {err}");
        }
    }
}

fn hours_since(rfc3339: &str) -> f64 {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|then| (chrono::Utc::now() - then.with_timezone(&chrono::Utc)).num_seconds() as f64 / 3600.0)
        .unwrap_or(0.0)
}

/// Motor de "modo proactivo" — el personaje inicia una conversación por su
/// cuenta bajo ciertas condiciones (inactividad, mood, hora del día,
/// memorias recientes), todas combinadas en una sola llamada de decisión
/// al LLM. Ver docs/ADR/007-proactive-mode.md.
#[derive(Clone)]
pub struct ProactiveService(Arc<Inner>);

impl ProactiveService {
    pub fn new(pool: DbPool, secrets: Arc<dyn SecretStore>, memory: Arc<dyn MemoryManager>, chat: ChatService) -> Self {
        Self(Arc::new(Inner {
            pool,
            secrets,
            memory,
            chat,
            active_profile_id: RwLock::new(None),
            cancellation: Mutex::new(None),
        }))
    }

    pub async fn set_active_profile(&self, profile_id: Option<String>) {
        *self.0.active_profile_id.write().await = profile_id;
    }
}

#[async_trait]
impl ManagedService for ProactiveService {
    fn name(&self) -> &str {
        "proactive"
    }

    /// A diferencia de Twitch/Discord/Llamada (que esperan una conexión
    /// explícita del usuario), este servicio arranca una vez al iniciar la
    /// app y corre mientras dure el proceso — el on/off real vive en
    /// `profile_proactive.enabled` por perfil, no en si el `ManagedService`
    /// está arrancado.
    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let token = CancellationToken::new();
        *self.0.cancellation.lock().await = Some(token.clone());

        let inner = self.0.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    () = token.cancelled() => return,
                    () = tokio::time::sleep(CHECK_INTERVAL) => {}
                }
                inner.evaluate_once().await;
            }
        });

        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(token) = self.0.cancellation.lock().await.take() {
            token.cancel();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use aivt_database::{NewProfile, ProfilesRepository as ProfilesRepo};
    use aivt_eventbus::{AppEvent, EventBus};
    use aivt_memory::MemoryService;
    use aivt_provider_embedding::EmbeddingProvider;
    use aivt_testing::MockLLMProvider;
    use chrono::Utc;

    async fn test_pool() -> DbPool {
        let pool = DbPool::connect("sqlite::memory:").await.unwrap();
        aivt_database::migrate(&pool).await.unwrap();
        pool
    }

    struct NoopEmbeddings;
    #[async_trait::async_trait]
    impl EmbeddingProvider for NoopEmbeddings {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, aivt_provider_embedding::EmbeddingError> {
            Err(aivt_provider_embedding::EmbeddingError::Unavailable("test double, no embeddings".to_string()))
        }
    }

    fn test_service(pool: DbPool, chat: ChatService) -> Inner {
        Inner {
            pool: pool.clone(),
            secrets: Arc::new(aivt_testing::MockSecretStore::new()),
            memory: Arc::new(MemoryService::new(pool, Arc::new(NoopEmbeddings), EventBus::new())),
            chat,
            active_profile_id: RwLock::new(None),
            cancellation: Mutex::new(None),
        }
    }

    async fn seed_profile(pool: &DbPool) -> aivt_database::Profile {
        let profiles = ProfilesRepo::new(pool);
        let created = profiles.create(NewProfile { name: "Ciel".to_string(), nickname: None, description: None }).await.unwrap();
        profiles.get(&created.id).await.unwrap().unwrap()
    }

    async fn seed_conversation_with_last_activity(pool: &DbPool, profile_id: &str, hours_ago: f64) -> aivt_database::Conversation {
        let conversations = ConversationsRepository::new(pool);
        let conversation = conversations.create(profile_id, "Charla", "chat").await.unwrap();
        let timestamp = (Utc::now() - chrono::Duration::milliseconds((hours_ago * 3_600_000.0) as i64)).to_rfc3339();
        sqlx::query("UPDATE conversations SET updated_at = ?1 WHERE id = ?2")
            .bind(&timestamp)
            .bind(&conversation.id)
            .execute(pool)
            .await
            .unwrap();
        conversations.get(&conversation.id).await.unwrap().unwrap()
    }

    #[tokio::test]
    async fn a_disabled_profile_never_fires() {
        let pool = test_pool().await;
        let chat = ChatService::new(pool.clone(), EventBus::new(), std::env::temp_dir(), Arc::new(aivt_testing::MockMemoryManager::new()));
        let profile = seed_profile(&pool).await;
        seed_conversation_with_last_activity(&pool, &profile.id, 10.0).await;
        let inner = test_service(pool.clone(), chat.clone());
        // proactive_settings defaults to disabled — never enabled here.

        let provider: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::with_response([r#"{"should_initiate": true, "message": "hola"}"#]));
        inner.evaluate_with(&profile.id, provider, "mock-model").await;

        let conversations = chat.list_conversations(&profile.id).await.unwrap();
        let messages = chat.get_messages(&conversations[0].id).await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn insufficient_inactivity_never_fires() {
        let pool = test_pool().await;
        let chat = ChatService::new(pool.clone(), EventBus::new(), std::env::temp_dir(), Arc::new(aivt_testing::MockMemoryManager::new()));
        let profile = seed_profile(&pool).await;
        seed_conversation_with_last_activity(&pool, &profile.id, 0.1).await; // 6 minutos, muy poco
        let inner = test_service(pool.clone(), chat.clone());
        inner.memory.set_proactive_enabled(&profile.id, true).await.unwrap();

        let provider: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::with_response([r#"{"should_initiate": true, "message": "hola"}"#]));
        inner.evaluate_with(&profile.id, provider, "mock-model").await;

        let conversations = chat.list_conversations(&profile.id).await.unwrap();
        let messages = chat.get_messages(&conversations[0].id).await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn a_cooldown_still_in_effect_never_fires() {
        let pool = test_pool().await;
        let chat = ChatService::new(pool.clone(), EventBus::new(), std::env::temp_dir(), Arc::new(aivt_testing::MockMemoryManager::new()));
        let profile = seed_profile(&pool).await;
        seed_conversation_with_last_activity(&pool, &profile.id, 10.0).await;
        let inner = test_service(pool.clone(), chat.clone());
        inner.memory.set_proactive_enabled(&profile.id, true).await.unwrap();
        inner.memory.mark_proactive_triggered(&profile.id).await.unwrap(); // recién disparado

        let provider: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::with_response([r#"{"should_initiate": true, "message": "hola"}"#]));
        inner.evaluate_with(&profile.id, provider, "mock-model").await;

        let conversations = chat.list_conversations(&profile.id).await.unwrap();
        let messages = chat.get_messages(&conversations[0].id).await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn no_existing_conversation_never_fires() {
        let pool = test_pool().await;
        let chat = ChatService::new(pool.clone(), EventBus::new(), std::env::temp_dir(), Arc::new(aivt_testing::MockMemoryManager::new()));
        let profile = seed_profile(&pool).await;
        let inner = test_service(pool.clone(), chat.clone());
        inner.memory.set_proactive_enabled(&profile.id, true).await.unwrap();

        let provider: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::with_response([r#"{"should_initiate": true, "message": "hola"}"#]));
        inner.evaluate_with(&profile.id, provider, "mock-model").await;

        let conversations = chat.list_conversations(&profile.id).await.unwrap();
        assert!(conversations.is_empty(), "no debería crear una conversación nueva en v1");
    }

    #[tokio::test]
    async fn the_model_declining_never_fires() {
        let pool = test_pool().await;
        let chat = ChatService::new(pool.clone(), EventBus::new(), std::env::temp_dir(), Arc::new(aivt_testing::MockMemoryManager::new()));
        let profile = seed_profile(&pool).await;
        seed_conversation_with_last_activity(&pool, &profile.id, 10.0).await;
        let inner = test_service(pool.clone(), chat.clone());
        inner.memory.set_proactive_enabled(&profile.id, true).await.unwrap();

        let provider: Arc<dyn LLMProvider> = Arc::new(MockLLMProvider::with_response([r#"{"should_initiate": false, "message": null}"#]));
        inner.evaluate_with(&profile.id, provider, "mock-model").await;

        let conversations = chat.list_conversations(&profile.id).await.unwrap();
        let messages = chat.get_messages(&conversations[0].id).await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn the_happy_path_sends_a_message_and_records_the_trigger() {
        let pool = test_pool().await;
        let event_bus = EventBus::new();
        let mut rx = event_bus.subscribe();
        let chat = ChatService::new(pool.clone(), event_bus, std::env::temp_dir(), Arc::new(aivt_testing::MockMemoryManager::new()));
        let profile = seed_profile(&pool).await;
        seed_conversation_with_last_activity(&pool, &profile.id, 10.0).await;
        let inner = test_service(pool.clone(), chat.clone());
        inner.memory.set_proactive_enabled(&profile.id, true).await.unwrap();

        let provider: Arc<dyn LLMProvider> =
            Arc::new(MockLLMProvider::with_response([r#"{"should_initiate": true, "message": "¿Cómo va todo?"}"#]));
        inner.evaluate_with(&profile.id, provider, "mock-model").await;

        let conversations = chat.list_conversations(&profile.id).await.unwrap();
        let messages = chat.get_messages(&conversations[0].id).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].message.content, "¿Cómo va todo?");

        let settings = inner.memory.proactive_settings(&profile.id).await.unwrap();
        assert!(settings.last_triggered_at.is_some());

        let mut saw_proactive_event = false;
        for _ in 0..4 {
            if matches!(rx.recv().await.unwrap(), AppEvent::ProactiveMessageSent { .. }) {
                saw_proactive_event = true;
                break;
            }
        }
        assert!(saw_proactive_event, "esperaba un AppEvent::ProactiveMessageSent");
    }
}
