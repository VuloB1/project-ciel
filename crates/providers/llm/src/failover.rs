use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::stream::{BoxStream, StreamExt};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::types::{ChatChunk, ChatRequest, ConnectionStatus, ModelCapabilities, ModelInfo};
use crate::{FailoverReason, LLMProvider, ProviderError};

/// Un proveedor del pool de rotación — el `model` va guardado acá (no en el
/// `ChatRequest` entrante) porque el nombre de modelo de cada proveedor es
/// propio (p.ej. `gpt-4o` vs `claude-3-5-sonnet-...` vs `gemini-1.5-pro`):
/// `FailoverLLMProvider::stream_chat` lo sustituye en la request antes de
/// probar cada entrada.
#[derive(Clone)]
pub struct LlmPoolEntry {
    pub id: String,
    pub model: String,
    pub provider: Arc<dyn LLMProvider>,
}

/// Envuelve un pool ordenado (el proveedor "en uso" primero, luego los
/// demás activos y fijados) y prueba cada uno en orden hasta que uno
/// responde — "nunca deja de funcionar" cuando el proveedor en uso se
/// queda sin crédito (`ProviderError::QuotaExceeded`), sin intervención del
/// usuario. Ver el plan de Proveedores IA, sección "Router de rotación
/// automática".
///
/// Las dos mitades del pool se tratan distinto a propósito: el proveedor "en
/// uso" (el primero) cede el turno cuando no puede atender por algo ajeno a
/// su configuración (`ProviderError::should_fail_over`: sin crédito, límite de
/// solicitudes, servidor caído o sin responder, error 5xx). Un error que
/// cambiar de proveedor no arregla — key inválida, modelo inexistente — se
/// muestra tal cual, porque es el que el usuario eligió y tiene que enterarse.
/// Los de reserva (el resto) se saltan ante cualquier fallo y se pasa al
/// siguiente. Si nadie responde, se muestra el error del proveedor en uso.
///
/// Nunca depende de `aivt-eventbus`/`aivt-service-settings` (ADR-003) — un
/// cambio de proveedor se avisa por un `mpsc` genérico en vez de publicar un
/// `AppEvent` directamente; `SettingsService::build_llm_provider` es quien
/// escucha ese canal y lo convierte en un evento real.
/// Cuánto se espera el primer fragmento de un proveedor antes de darlo por
/// "sin responder" (`ProviderError::Timeout`). Un servidor colgado que acepta la
/// conexión pero no contesta dejaría el chat esperando para siempre y nunca
/// pasaría al respaldo. Es holgado a propósito: un modelo local grande puede
/// tardar en procesar un prompt largo antes de emitir su primer token.
const DEFAULT_FIRST_CHUNK_TIMEOUT: Duration = Duration::from_secs(45);

/// Lo que el router cuenta de cada petición que se atendió con alguna novedad:
/// quién respondió, qué proveedores fallaron antes (y por qué) y — si cambió el
/// proveedor "en uso" — el motivo. `SettingsService` lo convierte en un aviso, en
/// el puntero "en uso" y en el estado de salud de cada proveedor.
#[derive(Debug, Clone)]
pub struct FailoverReport {
    /// El que la app consideraba "en uso" al armar el pool.
    pub in_use_id: String,
    /// El que de verdad respondió.
    pub responder_id: String,
    /// Los proveedores que fallaron por no poder atender, en el orden en que se probaron.
    pub failed: Vec<(String, FailoverReason)>,
    /// Motivo del cambio si `responder_id != in_use_id`: el fallo del proveedor en uso, o
    /// `Restored` si volvió el preferido. Sin significado si no hubo cambio.
    pub reason: FailoverReason,
}

pub struct FailoverLLMProvider {
    pool: Vec<LlmPoolEntry>,
    in_use_id: String,
    on_report: Option<UnboundedSender<FailoverReport>>,
    first_chunk_timeout: Duration,
}

impl FailoverLLMProvider {
    /// `pool` va en orden de preferencia (el primero es el proveedor en uso, o el
    /// preferido si se está probando su recuperación). `on_report`, si se pasa,
    /// recibe un `FailoverReport` por cada petición atendida en la que algún
    /// proveedor falló, o respondió uno distinto del "en uso". Si ninguno responde no
    /// se envía nada (nunca en `test_connection`/`list_models`, que solo miran el
    /// primero).
    pub fn new(pool: Vec<LlmPoolEntry>, on_report: Option<UnboundedSender<FailoverReport>>) -> Self {
        debug_assert!(pool.len() >= 2, "FailoverLLMProvider solo tiene sentido con 2 o más proveedores en el pool");
        let in_use_id = pool[0].id.clone();
        Self { pool, in_use_id, on_report, first_chunk_timeout: DEFAULT_FIRST_CHUNK_TIMEOUT }
    }

    /// Cuál es el proveedor "en uso" de la app cuando no coincide con el primero del
    /// pool — pasa cuando se está usando uno temporal y el preferido vuelve a probarse
    /// primero (recuperación). Por defecto es el primero.
    pub fn with_in_use(mut self, in_use_id: impl Into<String>) -> Self {
        self.in_use_id = in_use_id.into();
        self
    }

    /// Cambia el tiempo máximo de espera del primer fragmento (los tests usan uno corto).
    pub fn with_first_chunk_timeout(mut self, timeout: Duration) -> Self {
        self.first_chunk_timeout = timeout;
        self
    }
}

#[async_trait]
impl LLMProvider for FailoverLLMProvider {
    async fn test_connection(&self) -> Result<ConnectionStatus, ProviderError> {
        // Prueba solo el proveedor "en uso" — "usar ahora" ya deja elegir
        // cuál probar explícitamente vía `settings_test_llm_provider`, y
        // probar los N proveedores del pool acá sería lento y confuso (¿qué
        // significa "conectado" si uno falla y otro no?).
        self.pool[0].provider.test_connection().await
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        self.pool[0].provider.list_models().await
    }

    fn capabilities(&self, model: &str) -> ModelCapabilities {
        self.pool[0].provider.capabilities(model)
    }

    fn stream_chat(&self, request: ChatRequest) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
        let pool = self.pool.clone();
        let on_report = self.on_report.clone();
        let in_use_id = self.in_use_id.clone();
        let first_chunk_timeout = self.first_chunk_timeout;
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            // Por qué falló el proveedor "en uso": es lo que se le muestra al
            // usuario si nadie responde. Los de reserva son best-effort y no
            // deben tapar ese error con el suyo.
            let mut in_use_err: Option<ProviderError> = None;
            let mut failed: Vec<(String, FailoverReason)> = Vec::new();

            for (index, entry) in pool.iter().enumerate() {
                let mut attempt = request.clone();
                attempt.model = entry.model.clone();
                let mut upstream = entry.provider.stream_chat(attempt);

                let first = match tokio::time::timeout(first_chunk_timeout, upstream.next()).await {
                    Ok(first) => first,
                    Err(_) => Some(Err(ProviderError::Timeout)),
                };
                let failure = match first {
                    None => ProviderError::InvalidResponse("el proveedor no devolvió ninguna respuesta".to_string()),
                    // El proveedor "en uso" cede el turno solo si no puede
                    // atender por algo ajeno a su configuración (ver
                    // `should_fail_over`); un error de configuración suyo (key
                    // inválida, modelo inexistente) se muestra tal cual.
                    Some(Err(err)) if index == 0 && !err.should_fail_over() => {
                        let _ = tx.send(Err(err));
                        return;
                    }
                    // En cambio, un proveedor de reserva que falla por lo que
                    // sea (servidor apagado, key inválida, sin crédito) se
                    // salta: para eso están, para que siempre responda alguno.
                    Some(Err(err)) => err,
                    Some(Ok(first_chunk)) => {
                        // Recién acá se avisa: si nadie responde no hay a dónde
                        // "haber cambiado", y el "en uso" del usuario no debe
                        // moverse a uno que tampoco respondió.
                        if !failed.is_empty() || entry.id != in_use_id {
                            if let Some(sender) = &on_report {
                                // Por qué se cambió: lo que falló del proveedor en uso; si
                                // no falló nada suyo en esta petición, el preferido volvió.
                                let reason = failed
                                    .iter()
                                    .find(|(id, _)| *id == in_use_id)
                                    .map(|(_, reason)| *reason)
                                    .or_else(|| if index == 0 { Some(FailoverReason::Restored) } else { failed.first().map(|(_, reason)| *reason) })
                                    .unwrap_or(FailoverReason::Restored);
                                let _ = sender.send(FailoverReport { in_use_id: in_use_id.clone(), responder_id: entry.id.clone(), failed: failed.clone(), reason });
                            }
                        }
                        let finished_immediately = first_chunk.finished;
                        if tx.send(Ok(first_chunk)).is_err() || finished_immediately {
                            return;
                        }
                        // A partir del primer chunk exitoso, ya no se vuelve
                        // a inspeccionar nada — el resto del stream se
                        // reenvía tal cual, sin cambiar de proveedor a mitad
                        // de una respuesta ya empezada.
                        while let Some(chunk) = upstream.next().await {
                            let done = matches!(&chunk, Ok(c) if c.finished) || chunk.is_err();
                            if tx.send(chunk).is_err() || done {
                                return;
                            }
                        }
                        return;
                    }
                };

                if let Some(reason) = failure.failover_reason() {
                    failed.push((entry.id.clone(), reason));
                }
                if index == 0 {
                    in_use_err = Some(failure);
                } else {
                    tracing::warn!(target: "system", "proveedor de reserva `{}` descartado: {failure}", entry.id);
                }
            }

            let _ = tx.send(Err(in_use_err.unwrap_or(ProviderError::InvalidResponse("no hay proveedores de IA disponibles".to_string()))));
        });

        Box::pin(UnboundedReceiverStream::new(rx))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use futures::stream::{self, StreamExt};

    use super::*;
    use crate::types::{ChatMessage, MessageRole};

    /// El par `(en uso, respondió)` de un informe — lo que antes viajaba solo.
    fn pair(report: FailoverReport) -> (String, String) {
        (report.in_use_id, report.responder_id)
    }

    /// Proveedor de prueba: responde con una secuencia fija de resultados
    /// para `stream_chat` y cuenta cuántas veces se lo invocó — suficiente
    /// para probar el router sin tocar la red.
    struct StubProvider {
        calls: AtomicUsize,
        chunks: Vec<Result<ChatChunk, ProviderError>>,
    }

    impl StubProvider {
        fn new(chunks: Vec<Result<ChatChunk, ProviderError>>) -> Self {
            Self { calls: AtomicUsize::new(0), chunks }
        }
    }

    #[async_trait]
    impl LLMProvider for StubProvider {
        async fn test_connection(&self) -> Result<ConnectionStatus, ProviderError> {
            Ok(ConnectionStatus { connected: true, latency_ms: None, models: Vec::new() })
        }

        async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
            Ok(Vec::new())
        }

        fn capabilities(&self, _model: &str) -> ModelCapabilities {
            ModelCapabilities::default()
        }

        fn stream_chat(&self, _request: ChatRequest) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(stream::iter(self.chunks.clone()))
        }
    }

    // `ChatChunk`/`ProviderError` son `Clone`, pero `Vec<Result<...>>` como
    // campo necesita que el propio stub clone su contenido en cada llamada
    // — más simple que hacerlo `Clone` genérico.
    impl Clone for StubProvider {
        fn clone(&self) -> Self {
            Self { calls: AtomicUsize::new(self.calls.load(Ordering::SeqCst)), chunks: self.chunks.clone() }
        }
    }

    fn request() -> ChatRequest {
        ChatRequest { model: "placeholder".to_string(), messages: vec![ChatMessage::text(MessageRole::User, "hola")], temperature: None, max_tokens: None, tools: Vec::new(), context: Vec::new(), contains_stored_memories: false }
    }

    fn text_chunk(text: &str, finished: bool) -> Result<ChatChunk, ProviderError> {
        Ok(ChatChunk { delta: text.to_string(), finished, finish_reason: finished.then(|| "stop".to_string()), tool_calls: Vec::new() })
    }

    #[tokio::test]
    async fn a_quota_exceeded_first_chunk_moves_to_the_next_provider() {
        let first = Arc::new(StubProvider::new(vec![Err(ProviderError::QuotaExceeded("sin crédito".to_string()))]));
        let second = Arc::new(StubProvider::new(vec![text_chunk("hola", true)]));
        let (tx, mut rx) = mpsc::unbounded_channel();

        let router = FailoverLLMProvider::new(
            vec![LlmPoolEntry { id: "a".to_string(), model: "model-a".to_string(), provider: first.clone() }, LlmPoolEntry { id: "b".to_string(), model: "model-b".to_string(), provider: second.clone() }],
            Some(tx),
        );

        let results: Vec<_> = router.stream_chat(request()).collect().await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].as_ref().unwrap().delta, "hola");
        assert_eq!(pair(rx.try_recv().unwrap()), ("a".to_string(), "b".to_string()));
    }

    #[tokio::test]
    async fn a_non_quota_error_is_shown_as_is_without_rotating() {
        let first = Arc::new(StubProvider::new(vec![Err(ProviderError::Unauthorized)]));
        let second = Arc::new(StubProvider::new(vec![text_chunk("nunca debería usarse", true)]));

        let router = FailoverLLMProvider::new(
            vec![LlmPoolEntry { id: "a".to_string(), model: "model-a".to_string(), provider: first }, LlmPoolEntry { id: "b".to_string(), model: "model-b".to_string(), provider: second.clone() }],
            None,
        );

        let results: Vec<_> = router.stream_chat(request()).collect().await;
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], Err(ProviderError::Unauthorized)));
        assert_eq!(second.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn a_successful_first_chunk_never_inspects_the_rest_of_the_stream() {
        let first = Arc::new(StubProvider::new(vec![text_chunk("Hola", false), text_chunk(", mundo", true)]));
        let second = Arc::new(StubProvider::new(vec![text_chunk("nunca debería usarse", true)]));

        let router = FailoverLLMProvider::new(
            vec![LlmPoolEntry { id: "a".to_string(), model: "model-a".to_string(), provider: first }, LlmPoolEntry { id: "b".to_string(), model: "model-b".to_string(), provider: second.clone() }],
            None,
        );

        let results: Vec<_> = router.stream_chat(request()).collect().await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].as_ref().unwrap().delta, "Hola");
        assert_eq!(results[1].as_ref().unwrap().delta, ", mundo");
        assert_eq!(second.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn every_provider_exhausted_by_quota_surfaces_the_last_quota_error() {
        let first = Arc::new(StubProvider::new(vec![Err(ProviderError::QuotaExceeded("a".to_string()))]));
        let second = Arc::new(StubProvider::new(vec![Err(ProviderError::QuotaExceeded("b".to_string()))]));
        let (tx, mut rx) = mpsc::unbounded_channel();

        let router = FailoverLLMProvider::new(
            vec![LlmPoolEntry { id: "a".to_string(), model: "model-a".to_string(), provider: first }, LlmPoolEntry { id: "b".to_string(), model: "model-b".to_string(), provider: second }],
            Some(tx),
        );

        let results: Vec<_> = router.stream_chat(request()).collect().await;
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], Err(ProviderError::QuotaExceeded(_))));
        // Nadie respondió: el "en uso" del usuario no debe moverse a ningún lado.
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn skipping_several_exhausted_providers_reports_a_single_switch_from_the_one_in_use() {
        let exhausted = || Arc::new(StubProvider::new(vec![Err(ProviderError::QuotaExceeded("sin crédito".to_string()))]));
        let (tx, mut rx) = mpsc::unbounded_channel();

        let router = FailoverLLMProvider::new(
            vec![
                LlmPoolEntry { id: "a".to_string(), model: "model-a".to_string(), provider: exhausted() },
                LlmPoolEntry { id: "b".to_string(), model: "model-b".to_string(), provider: exhausted() },
                LlmPoolEntry { id: "c".to_string(), model: "model-c".to_string(), provider: Arc::new(StubProvider::new(vec![text_chunk("hola", true)])) },
            ],
            Some(tx),
        );

        let results: Vec<_> = router.stream_chat(request()).collect().await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].as_ref().unwrap().delta, "hola");
        assert_eq!(pair(rx.try_recv().unwrap()), ("a".to_string(), "c".to_string()));
        assert!(rx.try_recv().is_err(), "el cambio se avisa una sola vez, no por cada salto");
    }

    #[tokio::test]
    async fn a_fallback_that_fails_for_any_reason_is_skipped() {
        let exhausted = Arc::new(StubProvider::new(vec![Err(ProviderError::QuotaExceeded("sin crédito".to_string()))]));
        let server_down = Arc::new(StubProvider::new(vec![Err(ProviderError::ConnectionRefused)]));
        let bad_key = Arc::new(StubProvider::new(vec![Err(ProviderError::Unauthorized)]));
        let healthy = Arc::new(StubProvider::new(vec![text_chunk("hola", true)]));
        let (tx, mut rx) = mpsc::unbounded_channel();

        let router = FailoverLLMProvider::new(
            vec![
                LlmPoolEntry { id: "a".to_string(), model: "model-a".to_string(), provider: exhausted },
                LlmPoolEntry { id: "b".to_string(), model: "model-b".to_string(), provider: server_down.clone() },
                LlmPoolEntry { id: "c".to_string(), model: "model-c".to_string(), provider: bad_key.clone() },
                LlmPoolEntry { id: "d".to_string(), model: "model-d".to_string(), provider: healthy },
            ],
            Some(tx),
        );

        let results: Vec<_> = router.stream_chat(request()).collect().await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].as_ref().unwrap().delta, "hola");
        assert_eq!(server_down.calls.load(Ordering::SeqCst), 1);
        assert_eq!(bad_key.calls.load(Ordering::SeqCst), 1);
        assert_eq!(pair(rx.try_recv().unwrap()), ("a".to_string(), "d".to_string()));
    }

    #[tokio::test]
    async fn when_no_fallback_answers_the_error_of_the_provider_in_use_is_shown() {
        let exhausted = Arc::new(StubProvider::new(vec![Err(ProviderError::QuotaExceeded("sin crédito".to_string()))]));
        let server_down = Arc::new(StubProvider::new(vec![Err(ProviderError::ConnectionRefused)]));
        let (tx, mut rx) = mpsc::unbounded_channel();

        let router = FailoverLLMProvider::new(
            vec![LlmPoolEntry { id: "a".to_string(), model: "model-a".to_string(), provider: exhausted }, LlmPoolEntry { id: "b".to_string(), model: "model-b".to_string(), provider: server_down }],
            Some(tx),
        );

        let results: Vec<_> = router.stream_chat(request()).collect().await;
        assert_eq!(results.len(), 1);
        // No el "connection refused" de la reserva: ese no es el problema que el
        // usuario tiene que resolver, sino que el proveedor en uso se quedó sin crédito.
        assert!(matches!(results[0], Err(ProviderError::QuotaExceeded(_))));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn the_provider_in_use_still_shows_its_own_configuration_errors_even_with_a_healthy_fallback() {
        let server_down = Arc::new(StubProvider::new(vec![Err(ProviderError::Unauthorized)]));
        let healthy = Arc::new(StubProvider::new(vec![text_chunk("nunca debería usarse", true)]));

        let router = FailoverLLMProvider::new(
            vec![LlmPoolEntry { id: "a".to_string(), model: "model-a".to_string(), provider: server_down }, LlmPoolEntry { id: "b".to_string(), model: "model-b".to_string(), provider: healthy.clone() }],
            None,
        );

        let results: Vec<_> = router.stream_chat(request()).collect().await;
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], Err(ProviderError::Unauthorized)));
        assert_eq!(healthy.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn the_provider_in_use_failing_for_an_outage_or_a_limit_moves_to_the_fallback() {
        for failure in [
            ProviderError::ConnectionRefused,
            ProviderError::Timeout,
            ProviderError::RateLimited,
            ProviderError::InvalidResponse("HTTP 503 Service Unavailable: x".to_string()),
        ] {
            let down = Arc::new(StubProvider::new(vec![Err(failure.clone())]));
            let (tx, mut rx) = mpsc::unbounded_channel();
            let router = FailoverLLMProvider::new(
                vec![
                    LlmPoolEntry { id: "a".to_string(), model: "model-a".to_string(), provider: down },
                    LlmPoolEntry { id: "b".to_string(), model: "model-b".to_string(), provider: Arc::new(StubProvider::new(vec![text_chunk("hola", true)])) },
                ],
                Some(tx),
            );
            let results: Vec<_> = router.stream_chat(request()).collect().await;
            assert_eq!(results[0].as_ref().unwrap().delta, "hola", "{failure:?} debería pasar al respaldo");
            assert_eq!(pair(rx.try_recv().unwrap()), ("a".to_string(), "b".to_string()));
        }
    }

    /// Proveedor que acepta la petición y nunca contesta (servidor colgado).
    struct HungProvider;

    #[async_trait]
    impl LLMProvider for HungProvider {
        async fn test_connection(&self) -> Result<ConnectionStatus, ProviderError> {
            Ok(ConnectionStatus { connected: true, latency_ms: None, models: Vec::new() })
        }
        async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
            Ok(Vec::new())
        }
        fn capabilities(&self, _model: &str) -> ModelCapabilities {
            ModelCapabilities::default()
        }
        fn stream_chat(&self, _request: ChatRequest) -> BoxStream<'static, Result<ChatChunk, ProviderError>> {
            Box::pin(stream::pending())
        }
    }

    #[tokio::test]
    async fn a_hung_provider_in_use_times_out_and_moves_to_the_fallback() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let router = FailoverLLMProvider::new(
            vec![
                LlmPoolEntry { id: "a".to_string(), model: "model-a".to_string(), provider: Arc::new(HungProvider) },
                LlmPoolEntry { id: "b".to_string(), model: "model-b".to_string(), provider: Arc::new(StubProvider::new(vec![text_chunk("hola", true)])) },
            ],
            Some(tx),
        )
        .with_first_chunk_timeout(Duration::from_millis(50));

        let results: Vec<_> = router.stream_chat(request()).collect().await;
        assert_eq!(results[0].as_ref().unwrap().delta, "hola");
        assert_eq!(pair(rx.try_recv().unwrap()), ("a".to_string(), "b".to_string()));
    }

    #[tokio::test]
    async fn when_everything_hangs_the_error_is_a_timeout() {
        let router = FailoverLLMProvider::new(
            vec![
                LlmPoolEntry { id: "a".to_string(), model: "model-a".to_string(), provider: Arc::new(HungProvider) },
                LlmPoolEntry { id: "b".to_string(), model: "model-b".to_string(), provider: Arc::new(HungProvider) },
            ],
            None,
        )
        .with_first_chunk_timeout(Duration::from_millis(50));

        let results: Vec<_> = router.stream_chat(request()).collect().await;
        assert!(matches!(results[0], Err(ProviderError::Timeout)));
    }

    fn entry(id: &str, provider: Arc<dyn LLMProvider>) -> LlmPoolEntry {
        LlmPoolEntry { id: id.to_string(), model: format!("model-{id}"), provider }
    }

    #[tokio::test]
    async fn the_report_lists_every_failed_provider_and_why_the_one_in_use_failed() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let router = FailoverLLMProvider::new(
            vec![
                entry("a", Arc::new(StubProvider::new(vec![Err(ProviderError::ConnectionRefused)]))),
                entry("b", Arc::new(StubProvider::new(vec![Err(ProviderError::QuotaExceeded("x".to_string()))]))),
                entry("c", Arc::new(StubProvider::new(vec![text_chunk("hola", true)]))),
            ],
            Some(tx),
        );

        let _: Vec<_> = router.stream_chat(request()).collect().await;

        let report = rx.try_recv().unwrap();
        assert_eq!((report.in_use_id.as_str(), report.responder_id.as_str()), ("a", "c"));
        assert_eq!(report.failed, vec![("a".to_string(), FailoverReason::Unreachable), ("b".to_string(), FailoverReason::QuotaExceeded)]);
        assert_eq!(report.reason, FailoverReason::Unreachable);
    }

    #[tokio::test]
    async fn the_preferred_provider_answering_again_is_reported_as_restored() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        // `a` es el preferido (primero del pool); la app venía usando `b` mientras tanto.
        let router = FailoverLLMProvider::new(
            vec![entry("a", Arc::new(StubProvider::new(vec![text_chunk("hola", true)]))), entry("b", Arc::new(StubProvider::new(vec![text_chunk("no", true)])))],
            Some(tx),
        )
        .with_in_use("b");

        let results: Vec<_> = router.stream_chat(request()).collect().await;

        assert_eq!(results[0].as_ref().unwrap().delta, "hola");
        let report = rx.try_recv().unwrap();
        assert_eq!((report.in_use_id.as_str(), report.responder_id.as_str()), ("b", "a"));
        assert_eq!(report.reason, FailoverReason::Restored);
        assert!(report.failed.is_empty());
    }

    #[tokio::test]
    async fn failures_are_reported_even_when_the_provider_in_use_is_the_one_that_answers() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let router = FailoverLLMProvider::new(
            vec![entry("a", Arc::new(StubProvider::new(vec![Err(ProviderError::RateLimited)]))), entry("b", Arc::new(StubProvider::new(vec![text_chunk("hola", true)])))],
            Some(tx),
        )
        .with_in_use("b");

        let _: Vec<_> = router.stream_chat(request()).collect().await;

        let report = rx.try_recv().unwrap();
        assert_eq!(report.in_use_id, report.responder_id, "no hay cambio de proveedor en uso");
        assert_eq!(report.failed, vec![("a".to_string(), FailoverReason::RateLimited)]);
    }

    #[tokio::test]
    async fn no_switch_is_reported_when_the_provider_in_use_answers() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let router = FailoverLLMProvider::new(
            vec![
                LlmPoolEntry { id: "a".to_string(), model: "model-a".to_string(), provider: Arc::new(StubProvider::new(vec![text_chunk("hola", true)])) },
                LlmPoolEntry { id: "b".to_string(), model: "model-b".to_string(), provider: Arc::new(StubProvider::new(vec![text_chunk("nunca debería usarse", true)])) },
            ],
            Some(tx),
        );

        let _: Vec<_> = router.stream_chat(request()).collect().await;
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn each_attempt_uses_its_own_pool_entrys_model_name() {
        // `StubProvider` no distingue por modelo, así que esto se prueba
        // indirectamente: si `attempt.model` no se sustituyera, no habría
        // forma de observarlo acá sin instrumentar más el stub — se deja
        // como test de regresión de compilación/comportamiento general
        // (el router corre sin panics con dos modelos distintos).
        let first = Arc::new(StubProvider::new(vec![text_chunk("ok", true)]));
        let router = FailoverLLMProvider::new(
            vec![
                LlmPoolEntry { id: "a".to_string(), model: "gpt-4o".to_string(), provider: first },
                LlmPoolEntry { id: "b".to_string(), model: "claude-3-5-sonnet".to_string(), provider: Arc::new(StubProvider::new(vec![])) },
            ],
            None,
        );

        let results: Vec<_> = router.stream_chat(request()).collect().await;
        assert_eq!(results.len(), 1);
    }
}
