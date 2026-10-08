//! Uso de cada proveedor de IA: cuántas peticiones, cuántas fallaron, cuánto texto
//! se mandó y se recibió, y cuándo se usó por última vez. Solo cifras: nunca se
//! guarda el contenido de los mensajes.

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use chrono::{Duration as ChronoDuration, Utc};
use futures::stream::BoxStream;
use futures::Stream;
use serde::{Deserialize, Serialize};

use aivt_database::{DbPool, LlmUsageRepository, LlmUsageSample};
use aivt_provider_llm::{ChatChunk, ChatRequest, ConnectionStatus, LLMProvider, ModelCapabilities, ModelInfo, ProviderError};

use crate::service::SettingsService;
use crate::SettingsError;

/// Los modelos reparten ~4 caracteres de texto por token (promedio para español e
/// inglés). Los proveedores no devuelven siempre el consumo real y los adaptadores
/// no lo leen, así que los tokens que se muestran son una estimación.
const CHARS_PER_TOKEN: i64 = 4;

/// Una petición abandonada sin ninguna respuesta después de tanto tiempo se cuenta
/// como fallo (el servidor no contestó): el cambio automático la corta por tiempo y
/// el proveedor nunca llega a devolver un error propio.
const SILENT_FAILURE_AFTER: Duration = Duration::from_secs(30);

/// Largo máximo del último error que se guarda.
const MAX_ERROR_CHARS: usize = 200;

fn short_error(err: &ProviderError) -> String {
    let text = err.to_string();
    match text.char_indices().nth(MAX_ERROR_CHARS) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text,
    }
}

fn request_chars(request: &ChatRequest) -> i64 {
    request
        .messages
        .iter()
        .map(|m| m.content.chars().count() + m.tool_calls.iter().map(|c| c.arguments.chars().count()).sum::<usize>())
        .sum::<usize>() as i64
}

/// Envuelve un proveedor y anota cada petición en `llm_usage` al terminar (o al
/// abandonarse). Va POR DENTRO del `PrivacyGuardProvider`: así cuenta lo que de
/// verdad se manda al proveedor, ya filtrado.
pub struct UsageRecordingProvider {
    inner: Arc<dyn LLMProvider>,
    pool: DbPool,
    provider_id: String,
}

impl UsageRecordingProvider {
    pub fn new(inner: Arc<dyn LLMProvider>, pool: DbPool, provider_id: String) -> Self {
        Self { inner, pool, provider_id }
    }
}

#[async_trait]
impl LLMProvider for UsageRecordingProvider {
    async fn test_connection(&self) -> Result<ConnectionStatus, ProviderError> {
        self.inner.test_connection().await
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        self.inner.list_models().await
    }

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        self.inner.capabilities(model)
    }

    fn stream_chat(&self, request: ChatRequest) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
        let input_chars = request_chars(&request);
        Box::pin(TrackedStream {
            inner: self.inner.stream_chat(request),
            pool: self.pool.clone(),
            provider_id: self.provider_id.clone(),
            started: Instant::now(),
            input_chars,
            output_chars: 0,
            got_chunk: false,
            error: None,
            recorded: false,
        })
    }
}

struct TrackedStream {
    inner: BoxStream<'static, Result<ChatChunk, ProviderError>>,
    pool: DbPool,
    provider_id: String,
    started: Instant,
    input_chars: i64,
    output_chars: i64,
    got_chunk: bool,
    error: Option<String>,
    recorded: bool,
}

impl TrackedStream {
    fn record(&mut self) {
        if self.recorded {
            return;
        }
        self.recorded = true;
        let mut error = self.error.take();
        if error.is_none() && !self.got_chunk && self.started.elapsed() >= SILENT_FAILURE_AFTER {
            error = Some("no respondió a tiempo".to_string());
        }
        let sample = LlmUsageSample { input_chars: self.input_chars, output_chars: self.output_chars, error };
        let pool = self.pool.clone();
        let provider_id = self.provider_id.clone();
        // Se escribe fuera del flujo de la respuesta: contar nunca la retrasa. Si no hay
        // runtime (cierre de la app) simplemente no se anota.
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Err(err) = LlmUsageRepository::new(&pool).record(&provider_id, &sample).await {
                    tracing::warn!(target: "system", "uso de IA: no se pudo anotar: {err}");
                }
            });
        }
    }
}

impl Stream for TrackedStream {
    type Item = Result<ChatChunk, ProviderError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let polled = self.inner.as_mut().poll_next(cx);
        match &polled {
            Poll::Ready(Some(Ok(chunk))) => {
                self.got_chunk = true;
                self.output_chars += chunk.delta.chars().count() as i64 + chunk.tool_calls.iter().map(|c| c.arguments.chars().count() as i64).sum::<i64>();
            }
            Poll::Ready(Some(Err(err))) => {
                if self.error.is_none() {
                    self.error = Some(short_error(err));
                }
            }
            Poll::Ready(None) => self.record(),
            Poll::Pending => {}
        }
        polled
    }
}

impl Drop for TrackedStream {
    fn drop(&mut self) {
        self.record();
    }
}

/// Periodo del panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsagePeriod {
    Today,
    Last7Days,
    Last30Days,
    All,
}

impl UsagePeriod {
    fn since_day(self) -> Option<String> {
        let days_back = match self {
            UsagePeriod::Today => 0,
            UsagePeriod::Last7Days => 6,
            UsagePeriod::Last30Days => 29,
            UsagePeriod::All => return None,
        };
        Some((Utc::now() - ChronoDuration::days(days_back)).format("%Y-%m-%d").to_string())
    }
}

/// Lo que muestra el panel por proveedor configurado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmUsageEntry {
    pub provider_id: String,
    pub requests: i64,
    pub errors: i64,
    /// Estimados a partir del texto (~4 caracteres por token), no el consumo facturado.
    pub input_tokens_estimate: i64,
    pub output_tokens_estimate: i64,
    pub last_used_at: Option<String>,
    pub last_error: Option<String>,
}

impl SettingsService {
    /// Una fila por proveedor configurado (con ceros si no se ha usado), en el
    /// orden de la lista. El uso de proveedores ya eliminados no aparece.
    pub async fn llm_usage(&self, period: UsagePeriod) -> Result<Vec<LlmUsageEntry>, SettingsError> {
        let providers = self.llm_providers().await?.providers;
        let since = period.since_day();
        let totals = LlmUsageRepository::new(&self.pool).totals_since(since.as_deref()).await?;

        Ok(providers
            .into_iter()
            .map(|p| {
                let t = totals.iter().find(|(id, _)| *id == p.id).map(|(_, t)| t.clone()).unwrap_or_default();
                LlmUsageEntry {
                    provider_id: p.id,
                    requests: t.requests,
                    errors: t.errors,
                    input_tokens_estimate: t.input_chars / CHARS_PER_TOKEN,
                    output_tokens_estimate: t.output_chars / CHARS_PER_TOKEN,
                    last_used_at: t.last_used_at,
                    last_error: t.last_error,
                }
            })
            .collect())
    }

    /// Borra todas las cifras de uso (botón "Reiniciar" del panel).
    pub async fn clear_llm_usage(&self) -> Result<(), SettingsError> {
        LlmUsageRepository::new(&self.pool).clear().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use aivt_provider_llm::{ChatMessage, MessageRole};

    use super::*;
    use crate::llm::{AddLlmProviderInput, LlmProviderKind};
    use crate::service::test_service;

    struct Canned(Vec<Result<ChatChunk, ProviderError>>);

    #[async_trait]
    impl LLMProvider for Canned {
        async fn test_connection(&self) -> Result<ConnectionStatus, ProviderError> {
            unimplemented!()
        }
        async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
            Ok(Vec::new())
        }
        fn capabilities(&self, _model: &str) -> ModelCapabilities {
            ModelCapabilities::default()
        }
        fn stream_chat(&self, _request: ChatRequest) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
            futures::stream::iter(self.0.clone()).boxed()
        }
    }

    fn chunk(text: &str, finished: bool) -> Result<ChatChunk, ProviderError> {
        Ok(ChatChunk { delta: text.to_string(), finished, finish_reason: finished.then(|| "stop".to_string()), tool_calls: Vec::new() })
    }

    fn request(text: &str) -> ChatRequest {
        ChatRequest {
            model: "m".to_string(),
            messages: vec![ChatMessage::text(MessageRole::User, text)],
            temperature: None,
            max_tokens: None,
            tools: Vec::new(),
            context: Vec::new(),
            contains_stored_memories: false,
        }
    }

    /// El anotado se hace en una tarea aparte: se espera a que aparezca.
    async fn wait_for_requests(service: &SettingsService, expected: i64) -> LlmUsageEntry {
        for _ in 0..100 {
            let usage = service.llm_usage(UsagePeriod::All).await.unwrap();
            if let Some(entry) = usage.into_iter().find(|e| e.requests >= expected) {
                return entry;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("el uso no se anotó");
    }

    async fn service_with_provider() -> (SettingsService, String) {
        let service = test_service().await;
        let entry = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::OpenAi, label: None, base_url: None }).await.unwrap();
        (service, entry.id)
    }

    #[tokio::test]
    async fn a_finished_request_is_counted_with_what_was_sent_and_received() {
        let (service, id) = service_with_provider().await;
        let provider = UsageRecordingProvider::new(Arc::new(Canned(vec![chunk("hola ", false), chunk("mundo!!", true)])), service.pool.clone(), id.clone());

        let _ = provider.stream_chat(request(&"x".repeat(400))).collect::<Vec<_>>().await;

        let entry = wait_for_requests(&service, 1).await;
        assert_eq!(entry.provider_id, id);
        assert_eq!((entry.requests, entry.errors), (1, 0));
        assert_eq!(entry.input_tokens_estimate, 100);
        assert_eq!(entry.output_tokens_estimate, 3, "12 caracteres ≈ 3 tokens");
        assert!(entry.last_used_at.is_some());
        assert!(entry.last_error.is_none());
    }

    #[tokio::test]
    async fn a_failed_request_counts_as_an_error_with_its_message() {
        let (service, id) = service_with_provider().await;
        let provider = UsageRecordingProvider::new(Arc::new(Canned(vec![Err(ProviderError::QuotaExceeded(String::new()))])), service.pool.clone(), id);

        let _ = provider.stream_chat(request("hola")).collect::<Vec<_>>().await;

        let entry = wait_for_requests(&service, 1).await;
        assert_eq!((entry.requests, entry.errors), (1, 1));
        assert_eq!(entry.last_error.as_deref(), Some("se agotó el crédito de esta cuenta"));
    }

    #[tokio::test]
    async fn a_request_dropped_midway_is_still_counted_once() {
        let (service, id) = service_with_provider().await;
        let provider = UsageRecordingProvider::new(Arc::new(Canned(vec![chunk("par", false), chunk("cial", true)])), service.pool.clone(), id);

        let mut stream = provider.stream_chat(request("hola"));
        let _ = stream.next().await; // el usuario interrumpe después del primer trozo
        drop(stream);

        let entry = wait_for_requests(&service, 1).await;
        assert_eq!((entry.requests, entry.errors), (1, 0));
        assert_eq!(entry.output_tokens_estimate, 0, "3 caracteres no llegan a 1 token");
    }

    #[tokio::test]
    async fn the_panel_lists_every_configured_provider_even_unused_ones() {
        let (service, used) = service_with_provider().await;
        let unused = service.add_llm_provider(AddLlmProviderInput { kind: LlmProviderKind::Anthropic, label: None, base_url: None }).await.unwrap();
        let provider = UsageRecordingProvider::new(Arc::new(Canned(vec![chunk("ok", true)])), service.pool.clone(), used.clone());
        let _ = provider.stream_chat(request("hola")).collect::<Vec<_>>().await;
        wait_for_requests(&service, 1).await;

        let usage = service.llm_usage(UsagePeriod::Today).await.unwrap();
        assert_eq!(usage.len(), 2);
        assert_eq!(usage[0].provider_id, used);
        assert_eq!(usage[0].requests, 1);
        assert_eq!(usage[1].provider_id, unused.id);
        assert_eq!(usage[1], LlmUsageEntry { provider_id: unused.id.clone(), requests: 0, errors: 0, input_tokens_estimate: 0, output_tokens_estimate: 0, last_used_at: None, last_error: None });
    }

    #[tokio::test]
    async fn removing_a_provider_drops_its_numbers_and_reset_clears_everything() {
        let (service, id) = service_with_provider().await;
        let provider = UsageRecordingProvider::new(Arc::new(Canned(vec![chunk("ok", true)])), service.pool.clone(), id.clone());
        let _ = provider.stream_chat(request("hola")).collect::<Vec<_>>().await;
        wait_for_requests(&service, 1).await;

        service.clear_llm_usage().await.unwrap();
        assert_eq!(service.llm_usage(UsagePeriod::All).await.unwrap()[0].requests, 0);

        let _ = provider.stream_chat(request("hola")).collect::<Vec<_>>().await;
        wait_for_requests(&service, 1).await;
        service.remove_llm_provider(&id).await.unwrap();
        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM llm_usage").fetch_one(&service.pool).await.unwrap();
        assert_eq!(rows, 0, "al eliminar el proveedor se borra también su uso");
    }

    #[test]
    fn long_errors_are_cut_and_the_periods_start_on_the_right_day() {
        let long = ProviderError::Request("x".repeat(500));
        assert!(short_error(&long).chars().count() <= MAX_ERROR_CHARS + 1);
        assert!(UsagePeriod::All.since_day().is_none());
        let today = Utc::now().format("%Y-%m-%d").to_string();
        assert_eq!(UsagePeriod::Today.since_day().unwrap(), today);
        assert!(UsagePeriod::Last7Days.since_day().unwrap() < today);
        assert!(UsagePeriod::Last30Days.since_day().unwrap() < UsagePeriod::Last7Days.since_day().unwrap());
    }
}
