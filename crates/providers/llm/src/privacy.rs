//! Modos de privacidad por proveedor: qué datos del usuario se le mandan a cada
//! proveedor de IA. Una app local con un modelo en tu equipo y una app que habla
//! con un servicio en la nube no deberían mandar lo mismo; el usuario elige por
//! proveedor, y como el cambio automático puede pasar a otro proveedor a mitad de
//! camino, la regla viaja con cada uno (`PrivacyGuardProvider`) en vez de
//! decidirse una sola vez al armar el prompt.

use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::{self, BoxStream, StreamExt};
use serde::{Deserialize, Serialize};

use crate::types::{ChatChunk, ChatMessage, ChatRequest, ConnectionStatus, ContextBlock, MessageRole, ModelCapabilities, ModelInfo};
use crate::{LLMProvider, ProviderError};

/// Qué recibe un proveedor además de la conversación (que siempre recibe: sin
/// ella no puede responder).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyMode {
    /// Todo: recuerdos (privados y públicos), estado de ánimo e imágenes
    /// adjuntas. Es lo que había antes de que existieran los modos.
    #[default]
    Full,
    /// Solo los recuerdos marcados como públicos; el resto de recuerdos no sale.
    /// El estado de ánimo y las imágenes sí. No puede fusionar memorias.
    PublicOnly,
    /// Solo la conversación: sin recuerdos, sin estado de ánimo y sin imágenes.
    /// No puede fusionar memorias.
    Minimal,
}

impl PrivacyMode {
    pub fn allows_private_memories(self) -> bool {
        self == PrivacyMode::Full
    }

    pub fn allows_public_memories(self) -> bool {
        self != PrivacyMode::Minimal
    }

    pub fn allows_mood(self) -> bool {
        self != PrivacyMode::Minimal
    }

    pub fn allows_images(self) -> bool {
        self != PrivacyMode::Minimal
    }

    /// Operaciones que mandan TODOS los recuerdos guardados (fusionar).
    pub fn allows_stored_memories(self) -> bool {
        self == PrivacyMode::Full
    }
}

/// Envuelve un proveedor y le aplica su `PrivacyMode` a cada petición: anexa al
/// mensaje de sistema solo el contexto permitido, quita las imágenes si no se
/// permiten, y rechaza lo que mande recuerdos guardados si no se puede.
pub struct PrivacyGuardProvider {
    inner: Arc<dyn LLMProvider>,
    mode: PrivacyMode,
}

impl PrivacyGuardProvider {
    pub fn new(inner: Arc<dyn LLMProvider>, mode: PrivacyMode) -> Self {
        Self { inner, mode }
    }

    /// La petición que realmente sale hacia el proveedor, o el error si el modo la
    /// prohíbe del todo.
    pub fn apply(mode: PrivacyMode, mut request: ChatRequest) -> Result<ChatRequest, ProviderError> {
        if request.contains_stored_memories && !mode.allows_stored_memories() {
            return Err(ProviderError::PrivacyBlocked(
                "manda tus recuerdos guardados y este proveedor no tiene permiso para verlos".to_string(),
            ));
        }

        let mut extra = Vec::new();
        for block in std::mem::take(&mut request.context) {
            let text = match block {
                ContextBlock::Memories { all, public_only } => {
                    if mode.allows_private_memories() {
                        all
                    } else if mode.allows_public_memories() {
                        public_only
                    } else {
                        String::new()
                    }
                }
                ContextBlock::Mood(text) => {
                    if mode.allows_mood() {
                        text
                    } else {
                        String::new()
                    }
                }
            };
            if !text.is_empty() {
                extra.push(text);
            }
        }
        if !extra.is_empty() {
            let appended = extra.join("\n\n");
            match request.messages.iter_mut().find(|m| m.role == MessageRole::System) {
                Some(system) => {
                    system.content.push_str("\n\n");
                    system.content.push_str(&appended);
                }
                None => request.messages.insert(0, ChatMessage::text(MessageRole::System, appended)),
            }
        }

        if !mode.allows_images() {
            for message in &mut request.messages {
                message.images.clear();
            }
        }
        Ok(request)
    }
}

#[async_trait]
impl LLMProvider for PrivacyGuardProvider {
    async fn test_connection(&self) -> Result<ConnectionStatus, ProviderError> {
        self.inner.test_connection().await
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        self.inner.list_models().await
    }

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        let mut capabilities = self.inner.capabilities(model);
        if !self.mode.allows_images() {
            capabilities.vision = false;
        }
        capabilities
    }

    fn stream_chat(&self, request: ChatRequest) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
        match Self::apply(self.mode, request) {
            Ok(request) => self.inner.stream_chat(request),
            Err(err) => stream::once(async move { Err(err) }).boxed(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    fn memories() -> ContextBlock {
        ContextBlock::Memories { all: "Recuerdos:\n- privado\n- público".to_string(), public_only: "Recuerdos:\n- público".to_string() }
    }

    fn request(context: Vec<ContextBlock>) -> ChatRequest {
        let mut user = ChatMessage::text(MessageRole::User, "hola");
        user.images.push("data:image/png;base64,AAAA".to_string());
        ChatRequest {
            model: "m".to_string(),
            messages: vec![ChatMessage::text(MessageRole::System, "Eres Ciel"), user],
            temperature: None,
            max_tokens: None,
            tools: Vec::new(),
            context,
            contains_stored_memories: false,
        }
    }

    fn system_of(request: &ChatRequest) -> &str {
        &request.messages[0].content
    }

    #[test]
    fn full_mode_sends_everything() {
        let sent = PrivacyGuardProvider::apply(PrivacyMode::Full, request(vec![memories(), ContextBlock::Mood("Está contenta".to_string())])).unwrap();
        assert!(system_of(&sent).contains("- privado"));
        assert!(system_of(&sent).contains("- público"));
        assert!(system_of(&sent).contains("Está contenta"));
        assert_eq!(sent.messages[1].images.len(), 1);
        assert!(sent.context.is_empty(), "el contexto ya se anexó y no viaja aparte");
    }

    #[test]
    fn public_only_mode_drops_private_memories_but_keeps_mood_and_images() {
        let sent = PrivacyGuardProvider::apply(PrivacyMode::PublicOnly, request(vec![memories(), ContextBlock::Mood("Está contenta".to_string())])).unwrap();
        assert!(!system_of(&sent).contains("- privado"));
        assert!(system_of(&sent).contains("- público"));
        assert!(system_of(&sent).contains("Está contenta"));
        assert_eq!(sent.messages[1].images.len(), 1);
    }

    #[test]
    fn minimal_mode_sends_only_the_conversation() {
        let sent = PrivacyGuardProvider::apply(PrivacyMode::Minimal, request(vec![memories(), ContextBlock::Mood("Está contenta".to_string())])).unwrap();
        assert_eq!(system_of(&sent), "Eres Ciel");
        assert!(sent.messages[1].images.is_empty());
        assert_eq!(sent.messages[1].content, "hola");
    }

    #[test]
    fn public_only_with_no_public_memories_adds_nothing() {
        let only_private = ContextBlock::Memories { all: "Recuerdos:\n- privado".to_string(), public_only: String::new() };
        let sent = PrivacyGuardProvider::apply(PrivacyMode::PublicOnly, request(vec![only_private])).unwrap();
        assert_eq!(system_of(&sent), "Eres Ciel");
    }

    #[test]
    fn context_without_a_system_message_gets_one() {
        let mut req = request(vec![memories()]);
        req.messages.remove(0);
        let sent = PrivacyGuardProvider::apply(PrivacyMode::Full, req).unwrap();
        assert_eq!(sent.messages[0].role, MessageRole::System);
        assert!(sent.messages[0].content.contains("- privado"));
    }

    #[test]
    fn requests_with_stored_memories_are_refused_unless_the_mode_is_full() {
        let mut req = request(Vec::new());
        req.contains_stored_memories = true;
        assert!(PrivacyGuardProvider::apply(PrivacyMode::Full, req.clone()).is_ok());
        for mode in [PrivacyMode::PublicOnly, PrivacyMode::Minimal] {
            let err = PrivacyGuardProvider::apply(mode, req.clone()).unwrap_err();
            assert!(matches!(err, ProviderError::PrivacyBlocked(_)));
            assert!(!err.should_fail_over(), "no se salta a otro proveedor sin que el usuario lo decida");
        }
    }

    #[test]
    fn a_minimal_provider_stops_advertising_vision() {
        struct Vision;
        #[async_trait]
        impl LLMProvider for Vision {
            async fn test_connection(&self) -> Result<ConnectionStatus, ProviderError> {
                unimplemented!()
            }
            async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
                Ok(Vec::new())
            }
            fn capabilities(&self, _model: &str) -> ModelCapabilities {
                ModelCapabilities { vision: true, ..Default::default() }
            }
            fn stream_chat(&self, _request: ChatRequest) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
                stream::empty().boxed()
            }
        }
        assert!(PrivacyGuardProvider::new(Arc::new(Vision), PrivacyMode::Full).capabilities("m").vision);
        assert!(!PrivacyGuardProvider::new(Arc::new(Vision), PrivacyMode::Minimal).capabilities("m").vision);
    }

    #[tokio::test]
    async fn the_inner_provider_only_ever_sees_the_filtered_request() {
        struct Recorder(Mutex<Vec<ChatRequest>>);
        #[async_trait]
        impl LLMProvider for Arc<Recorder> {
            async fn test_connection(&self) -> Result<ConnectionStatus, ProviderError> {
                unimplemented!()
            }
            async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
                Ok(Vec::new())
            }
            fn capabilities(&self, _model: &str) -> ModelCapabilities {
                ModelCapabilities::default()
            }
            fn stream_chat(&self, request: ChatRequest) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
                self.0.lock().unwrap().push(request);
                stream::empty().boxed()
            }
        }
        let recorder = Arc::new(Recorder(Mutex::new(Vec::new())));
        let guard = PrivacyGuardProvider::new(Arc::new(recorder.clone()), PrivacyMode::Minimal);
        let _ = guard.stream_chat(request(vec![memories()])).collect::<Vec<_>>().await;

        let seen = recorder.0.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert!(!seen[0].messages.iter().any(|m| m.content.contains("privado")));
        assert!(seen[0].messages.iter().all(|m| m.images.is_empty()));
    }

    #[test]
    fn modes_serialize_as_snake_case_and_default_to_full() {
        assert_eq!(serde_json::to_string(&PrivacyMode::PublicOnly).unwrap(), "\"public_only\"");
        assert_eq!(serde_json::from_str::<PrivacyMode>("\"minimal\"").unwrap(), PrivacyMode::Minimal);
        assert_eq!(PrivacyMode::default(), PrivacyMode::Full);
    }
}
