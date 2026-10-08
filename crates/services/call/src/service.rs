use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc as tokio_mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use aivt_core::contains_word;
use aivt_database::{DbPool, MessageRole, PeopleRepository, Profile, ProfilesRepository};
use aivt_eventbus::{AppEvent, CallState, EventBus};
use aivt_memory::MemoryManager;
use aivt_orchestrator::AIOrchestrator;
use aivt_provider_llm::LLMProvider;
use aivt_provider_tools::{BraveSearchTool, CurrentTimeTool, Tool};
use aivt_provider_stt::{SttModelManager, TranscribeRequest, WhisperCppProvider, WhisperModelSize, STTProvider};
use aivt_provider_tts::{SpeakRequest, TTSProvider};
use aivt_secrets::SecretStore;
use aivt_service_manager::ManagedService;
use aivt_service_settings::{SettingsService, SttModelSize};

use crate::audio_guard::{EchoGuard, LevelMeter};
use crate::capture::MicCapture;
use crate::error::CallError;
use crate::history::CallHistory;
use crate::segmenter::{Segmenter, SegmenterEvent};

/// How many known-people names `transcription_prompt` folds in alongside
/// the active profile's own name — Whisper's `initial_prompt` shares a
/// small, fixed text-context budget with the actual transcription output
/// (see `aivt_provider_stt`), so this stays a short, curated list (see
/// `PeopleRepository::top_for_prompt`'s ranking) rather than every person
/// the app has ever learned about.
const MAX_KNOWN_PEOPLE_IN_PROMPT: i64 = 8;

fn box_err(err: CallError) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(err)
}

/// Everything torn down together when a call ends. Dropping `_mic` stops
/// the `cpal` stream and closes the sample channel it feeds — the
/// segmenter thread's receive loop ends on its own once that closes,
/// which in turn closes the channel the async orchestrator task reads
/// from, ending its loop too. `cancellation` additionally makes `stop()`
/// responsive even while a generation/speak is in flight (mirrors
/// `TwitchService::Inner::cancellation`).
struct Session {
    _mic: MicCapture,
    cancellation: CancellationToken,
    /// Checked in the capture callback (see `start`) before samples ever
    /// reach the segmenter — muting just stops feeding audio in, rather
    /// than closing and reopening the mic stream each time.
    muted: Arc<AtomicBool>,
}

/// Cada cuánto se publica el nivel del micrófono (~15 veces por segundo).
const LEVEL_INTERVAL: Duration = Duration::from_millis(66);

/// Por debajo de este pico (de 32768) se considera que no se habló: ruido de
/// fondo de un micrófono abierto, no voz.
const SILENCE_PEAK: i16 = 500;

fn is_near_silence(pcm: &[i16]) -> bool {
    pcm.iter().all(|sample| sample.saturating_abs() < SILENCE_PEAK)
}

/// Whisper marca los sonidos que no son voz entre corchetes (`[Música]`,
/// `[Aplausos]`) y suele inventarlos sobre silencio o ruido: se quitan, y si
/// no queda nada el dictado cuenta como vacío.
fn clean_dictation(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0usize;
    for ch in text.chars() {
        match ch {
            '[' => depth += 1,
            ']' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Una grabación de dictado en curso (botón de micrófono del chat): el audio
/// se acumula acá hasta que el usuario la detiene y se transcribe de una vez.
/// Al soltar `_mic` se cierra el stream de captura.
struct Dictation {
    _mic: MicCapture,
    samples: Arc<std::sync::Mutex<Vec<i16>>>,
    /// Detiene la tarea que transcribe textos provisionales mientras se graba.
    cancel: CancellationToken,
}

impl Drop for Dictation {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

/// Cada cuánto se transcribe lo grabado hasta ahora para mostrar el texto
/// en vivo. La siguiente pasada empieza cuando termina la anterior, así que
/// nunca se acumulan transcripciones en cola.
const PARTIAL_INTERVAL: Duration = Duration::from_millis(1500);
/// Los provisionales solo miran los últimos 25 s: transcribir todo el audio
/// cada vez se volvería cada vez más lento. La pasada final sí usa todo.
const PARTIAL_WINDOW_SAMPLES: usize = 16_000 * 25;

/// Tope de un dictado: 5 minutos a 16 kHz. Pasado eso se descartan las muestras
/// nuevas en vez de crecer sin límite si el usuario olvida detener la grabación.
const MAX_DICTATION_SAMPLES: usize = 16_000 * 60 * 5;
/// Menos de ~0,3 s de audio no alcanza para que Whisper entienda nada.
const MIN_DICTATION_SAMPLES: usize = 4_800;

struct Inner {
    pool: DbPool,
    event_bus: EventBus,
    secrets: Arc<dyn SecretStore>,
    tts: Arc<dyn TTSProvider>,
    stt_models: SttModelManager,
    /// Which profile answers the call — kept live-synced from the frontend
    /// (`call_set_active_profile`), same reasoning as
    /// `TwitchService::Inner::active_profile_id`.
    active_profile_id: RwLock<Option<String>>,
    session: Mutex<Option<Session>>,
    dictation: Mutex<Option<Dictation>>,
    /// Micrófono abierto solo para mostrar su nivel en Configuración, sin llamada.
    mic_test: Mutex<Option<MicCapture>>,
    /// Lo dicho en la llamada en curso — se vacía al empezar y al colgar.
    history: std::sync::Mutex<CallHistory>,
    /// Recuerdos y estado de ánimo del personaje. La Llamada es una charla
    /// con el dueño, así que lee TODA la memoria (`search`, no
    /// `search_public`): el alcance público/privado solo recorta lo que ven
    /// los canales abiertos a terceros (Twitch/Discord).
    memory: Arc<dyn MemoryManager>,
}

/// Cuántos recuerdos entran al prompt de un turno de la llamada — pocos a
/// propósito: cada token extra encarece la respuesta hablada (mismo valor que
/// `ChatService`).
const MEMORY_RETRIEVAL_LIMIT: i64 = 5;

impl Inner {
    fn settings(&self) -> SettingsService {
        SettingsService::new(self.pool.clone(), self.secrets.clone())
    }

    /// Reads which model size is selected and transcribes `pcm` against it.
    /// Unlike the old HTTP path, loading the model itself is real work —
    /// that part is cached inside `stt_models` (see
    /// `SttModelManager::ensure_loaded`) and only reloaded when the
    /// selected size changes; only the cheap selection read happens fresh
    /// on every call, same reasoning `should_respond_to` already uses for
    /// `call_policy`.
    async fn transcribe(&self, pcm: Vec<i16>) -> Result<String, CallError> {
        let prompt = self.transcription_prompt().await;
        self.transcribe_with_prompt(pcm, prompt).await
    }

    /// Mientras dura un dictado, transcribe cada tanto lo grabado y publica el
    /// texto provisional para que el chat lo muestre en vivo.
    async fn run_dictation_partials(&self, samples: Arc<std::sync::Mutex<Vec<i16>>>, cancel: CancellationToken) {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = tokio::time::sleep(PARTIAL_INTERVAL) => {}
            }
            let pcm: Vec<i16> = {
                let buffer = samples.lock().expect("dictation buffer poisoned");
                let start = buffer.len().saturating_sub(PARTIAL_WINDOW_SAMPLES);
                buffer[start..].to_vec()
            };
            if pcm.len() < MIN_DICTATION_SAMPLES * 3 || is_near_silence(&pcm) {
                continue;
            }
            let Ok(text) = self.transcribe_with_prompt(pcm, None).await else { continue };
            let text = clean_dictation(&text);
            if cancel.is_cancelled() {
                return;
            }
            if !text.is_empty() {
                self.event_bus.publish(AppEvent::DictationPartial { text });
            }
        }
    }

    async fn transcribe_with_prompt(&self, pcm: Vec<i16>, prompt: Option<String>) -> Result<String, CallError> {
        let model = self.settings().stt_model().await.map_err(|err| CallError::Settings(err.to_string()))?;
        let language = self.settings().language().await.map_err(|err| CallError::Settings(err.to_string()))?;

        let context = match self.stt_models.ensure_loaded(to_whisper_model_size(model.size), model.gpu_enabled).await {
            Ok(context) => context,
            Err(aivt_provider_stt::SttError::Unavailable(_)) => return Err(CallError::NoSttModel),
            Err(err) => return Err(CallError::Transcription(err)),
        };
        let provider = WhisperCppProvider::new(context);
        let request = TranscribeRequest {
            pcm_i16: pcm,
            sample_rate: crate::capture::CAPTURE_SAMPLE_RATE,
            language: Some(language),
            prompt,
        };
        Ok(provider.transcribe(request).await?)
    }

    /// Whisper-style STT models bias their output toward whatever text
    /// they're given as context ("prompt") — this exists specifically so
    /// an active profile's name (an uncommon word Whisper has no other
    /// reason to expect) gets recognized as itself instead of the closest
    /// ordinary words it knows. A live-testing checkpoint hit exactly
    /// this: "Ciel" kept transcribing as "si él" until this was added —
    /// see docs/DEVELOPMENT_STATUS.md's Fase 6 section. Also folds in the
    /// registro de personas (real people the app has learned about from
    /// conversations and Twitch chat — see `aivt_memory::PeopleManager`)
    /// so ordinary human names get the same recognition boost, not just
    /// the character's own name (pedido del usuario: que funcione "de
    /// forma general" para cualquier nombre, incluyendo japoneses/poco
    /// comunes).
    async fn transcription_prompt(&self) -> Option<String> {
        let profile_id = self.active_profile_id.read().await.clone()?;
        self.transcription_prompt_for(&profile_id).await
    }

    async fn transcription_prompt_for(&self, profile_id: &str) -> Option<String> {
        let profile = ProfilesRepository::new(&self.pool).get(profile_id).await.ok()??;

        let mut prompt = match profile.nickname.filter(|n| !n.trim().is_empty()) {
            Some(nickname) => format!("Conversación con un personaje llamado {} (también llamado {nickname}).", profile.name),
            None => format!("Conversación con un personaje llamado {}.", profile.name),
        };

        if let Ok(people) = PeopleRepository::new(&self.pool).top_for_prompt(MAX_KNOWN_PEOPLE_IN_PROMPT).await {
            if !people.is_empty() {
                let names = people.iter().map(|p| p.canonical_name.as_str()).collect::<Vec<_>>().join(", ");
                prompt.push_str(&format!(" Otras personas que pueden mencionarse: {names}."));
            }
        }

        Some(prompt)
    }

    /// Whether an already-transcribed utterance should get a reply, per
    /// the current `CallPolicy` — mirrors Twitch's `mentioned_only` gating
    /// (`aivt_service_twitch::policy::should_respond` +
    /// `mentions_active_character`), reusing the same `contains_word`
    /// match (`aivt_core`) so "does this text address the character by
    /// name" behaves identically across channels. Read fresh per
    /// utterance, same reasoning as `transcribe`'s settings reads.
    async fn should_respond_to(&self, text: &str) -> bool {
        let policy = match self.settings().call_policy().await {
            Ok(policy) => policy,
            Err(err) => {
                tracing::warn!(target: "system", "call: no se pudo leer la política de llamada, se responde por defecto: {err}");
                return true;
            }
        };
        if policy.respond_to_all {
            return true;
        }

        if let Some(phrase) = policy.wake_phrase.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
            return contains_word(text, phrase);
        }

        // No custom phrase configured — fall back to the active profile's
        // name/nickname, the same trigger Twitch's `mentioned_only` uses.
        let Some(profile_id) = self.active_profile_id.read().await.clone() else {
            return false;
        };
        let profile = match ProfilesRepository::new(&self.pool).get(&profile_id).await {
            Ok(Some(profile)) => profile,
            _ => return false,
        };
        if contains_word(text, &profile.name) {
            return true;
        }
        match profile.nickname {
            Some(nickname) => contains_word(text, &nickname),
            None => false,
        }
    }

    /// Loads the active profile, calls the LLM, and returns the full
    /// reply text — mirrors `TwitchService::Inner::generate_reply`, minus
    /// the Twitch-length cap (a call reply isn't posted anywhere with a
    /// character limit) and with cancellation support so `stop()`/barge-in
    /// can cut a generation short instead of waiting for it to finish.
    async fn generate_reply(&self, trigger_text: &str, cancellation: &CancellationToken) -> Result<(Profile, String), CallError> {
        let Some(profile_id) = self.active_profile_id.read().await.clone() else {
            tracing::info!(target: "system", "call: no se puede responder, no hay un personaje activo (selecciónalo en Identidad)");
            return Err(CallError::NoActiveProfile);
        };
        let profile = ProfilesRepository::new(&self.pool).get(&profile_id).await?.ok_or(CallError::NoActiveProfile)?;

        let llm_connection = match self.settings().llm_connection().await {
            Ok(Some(connection)) => connection,
            Ok(None) => {
                tracing::warn!(target: "system", "call: no hay proveedor de IA configurado, se omite la respuesta");
                return Err(CallError::NoLlmConnection);
            }
            Err(err) => return Err(CallError::Settings(err.to_string())),
        };
        let provider: Arc<dyn LLMProvider> = self.settings().build_llm_provider(&self.event_bus).await.map_err(|err| CallError::Settings(err.to_string()))?;
        let language = self.settings().language().await.unwrap_or_else(|_| "es".to_string());
        let mut orchestrator = AIOrchestrator::new(provider, llm_connection.model, Some(llm_connection.temperature), None);
        // `get_current_time` is always available — pure local calculation,
        // no network/API key, so it doesn't need the same opt-in as
        // `web_search` (Fase 10b). `web_search` stays behind
        // `WebSearchSettings.enabled`, incluida Llamada desde el arranque
        // (pedido explícito del usuario, aceptando el silencio ocasional
        // de 1-3s que agrega una búsqueda) — ver su doc comment.
        let mut tools: Vec<Arc<dyn Tool>> = vec![Arc::new(CurrentTimeTool::new(language.as_str()))];
        if let Ok(web_search) = self.settings().web_search_settings().await {
            if web_search.enabled {
                if let Ok(Some(key)) = self.settings().web_search_api_key().await {
                    tools.push(Arc::new(BraveSearchTool::new(key, language.as_str())));
                }
            }
        }
        orchestrator = orchestrator.with_tools(tools);

        // Los turnos anteriores de ESTA llamada + lo que acaba de decir el usuario:
        // sin esto el personaje respondía cada frase como si fuera la primera.
        let (synthetic_history, retrieval_query) = {
            let history = self.history.lock().expect("call history poisoned");
            (history.as_messages_with(trigger_text), history.retrieval_query_with(trigger_text))
        };

        // Un fallo de memoria o de mood nunca bloquea la respuesta: degrada a
        // "sin esa sección" (mismo criterio que el Chat, ver ADR-006).
        let relevant_memories = match self.memory.search(&profile.id, &retrieval_query, MEMORY_RETRIEVAL_LIMIT).await {
            Ok(memories) => memories,
            Err(err) => {
                tracing::warn!(target: "system", "call: no se pudieron buscar recuerdos, se responde sin ellos: {err}");
                Vec::new()
            }
        };
        let mood = self.memory.get_mood(&profile.id).await.ok();

        use futures::StreamExt;
        let mut stream = orchestrator.stream_respond(&profile, &synthetic_history, None, &relevant_memories, mood.as_ref(), None, &language);
        let mut full_text = String::new();
        loop {
            let next = tokio::select! {
                () = cancellation.cancelled() => return Err(CallError::Generation("cancelada".to_string())),
                next = stream.next() => next,
            };
            match next {
                Some(Ok(chunk)) => {
                    full_text.push_str(&chunk.delta);
                    if chunk.finished {
                        break;
                    }
                }
                Some(Err(err)) => return Err(CallError::Generation(err.to_string())),
                None => break,
            }
        }

        Ok((profile, full_text.trim().to_string()))
    }

    /// Speaks `text` and waits for it to actually finish (or be
    /// interrupted) before returning — unlike Twitch's fire-and-forget
    /// `spawn_speak`, the call orchestrator loop needs to know when
    /// speaking ends so it can go back to listening for the next turn.
    /// Subscribes to `EventBus` *before* calling `speak` so a fast
    /// backend can't finish and publish before this starts listening.
    async fn speak_and_wait(&self, profile: &Profile, text: &str, cancellation: &CancellationToken) {
        if !profile.voice.enabled || text.trim().is_empty() {
            tracing::info!(
                target: "system",
                "call: TTS omitido (voz activada={}, texto vacío={})",
                profile.voice.enabled,
                text.trim().is_empty(),
            );
            return;
        }

        let backend = match self.settings().build_tts_backend().await {
            Ok(backend) => backend,
            Err(err) => {
                tracing::warn!(target: "system", "call: no se pudo leer la configuración de voz: {err}");
                return;
            }
        };

        let message_id = uuid::Uuid::new_v4().to_string();
        let mut events = self.event_bus.subscribe();

        let voice = profile.voice.clone();
        let request = SpeakRequest {
            text: text.to_string(),
            voice_id: voice.voice_id,
            rate: voice.rate as f32,
            pitch: voice.pitch as f32,
            volume: voice.volume as f32,
            backend,
            profile_id: profile.id.clone(),
            message_id: Some(message_id.clone()),
        };
        if let Err(err) = self.tts.speak(request).await {
            tracing::warn!(target: "system", "call: TTS falló: {err}");
            return;
        }

        loop {
            let event = tokio::select! {
                () = cancellation.cancelled() => return,
                event = events.recv() => event,
            };
            match event {
                Ok(AppEvent::VoicePlaybackFinished { message_id: Some(id), .. }) if id == message_id => return,
                Ok(AppEvent::VoicePlaybackError { message_id: Some(id), .. }) if id == message_id => return,
                Ok(_) => continue,
                // Con el medidor de nivel publicando varias veces por segundo,
                // quedarse atrás unos eventos es posible y no significa que la
                // voz terminó: se sigue esperando el aviso de fin.
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return, // don't hang the call over it
            }
        }
    }

    /// `self: Arc<Self>` (not `&self`) — needs an owned, `'static` handle
    /// to move into each turn's spawned task (same reasoning as
    /// `TwitchService::run_message_loop`'s `self: Arc<Self>`).
    ///
    /// Each `UtteranceComplete` now runs as its own detached task instead
    /// of being awaited inline, so this loop keeps consuming segmenter
    /// events (in particular `SpeechConfirmed`) while a turn is still
    /// thinking/speaking — that's what makes barge-in possible at all.
    /// `active_turn` tracks the one currently running (a plain local, not
    /// behind a lock: only this single task ever touches it).
    async fn run_orchestrator(self: Arc<Self>, mut event_rx: tokio_mpsc::UnboundedReceiver<SegmenterEvent>, cancellation: CancellationToken) {
        self.event_bus.publish(AppEvent::CallStateChanged { state: CallState::Idle });
        let mut active_turn: Option<(CancellationToken, tokio::task::JoinHandle<()>)> = None;

        loop {
            let event = tokio::select! {
                () = cancellation.cancelled() => return,
                event = event_rx.recv() => event,
            };
            let Some(event) = event else { return }; // channel closed — capture stopped

            match event {
                SegmenterEvent::OnsetDetected => {
                    self.event_bus.publish(AppEvent::CallStateChanged { state: CallState::Listening });
                }
                SegmenterEvent::SpeechConfirmed => {
                    // Eager barge-in: only when *every* utterance is meant
                    // to get a reply. In wake-phrase mode, cutting the
                    // character off here — before knowing whether this
                    // utterance even mentions it — risks silencing it over
                    // a side conversation not addressed to it; that mode
                    // instead waits for `should_respond_to` inside
                    // `handle_utterance` (pedido del usuario).
                    if let Some((token, _)) = &active_turn {
                        let respond_to_all = self.settings().call_policy().await.map(|p| p.respond_to_all).unwrap_or(true);
                        if respond_to_all {
                            token.cancel();
                            if let Err(err) = self.tts.stop().await {
                                tracing::warn!(target: "system", "call: no se pudo detener la voz al interrumpir: {err}");
                            }
                        }
                    }
                }
                SegmenterEvent::UtteranceComplete(pcm) => {
                    // Handed to `handle_utterance` so *it* can cancel the
                    // previous turn once it confirms (in wake-phrase mode)
                    // this new utterance actually deserves a reply — see
                    // that function. In `respond_to_all` mode the previous
                    // turn is typically already cancelled by
                    // `SpeechConfirmed` above; this is a no-op then.
                    let previous_turn = active_turn.as_ref().map(|(token, _)| token.clone());
                    let turn_cancellation = cancellation.child_token();

                    let inner = self.clone();
                    let handle_token = turn_cancellation.clone();
                    let handle = tokio::spawn(async move {
                        inner.handle_utterance(pcm, &handle_token, previous_turn).await;
                        // Only the turn that actually ran to completion
                        // gets to declare the call Idle again — a turn
                        // that was cancelled (interrupted) must stay
                        // quiet, since whichever turn interrupted it is
                        // the one now driving state (Listening →
                        // Transcribing → ...); publishing Idle here would
                        // stomp on that.
                        if !handle_token.is_cancelled() {
                            inner.event_bus.publish(AppEvent::CallStateChanged { state: CallState::Idle });
                        }
                    });

                    // Deliberately not awaiting the previous turn's handle
                    // — it winds down on its own (either already
                    // cancelled, or left to finish naturally if this new
                    // utterance turns out not to warrant a reply at all,
                    // which is exactly the wake-phrase "wait to confirm"
                    // behavior).
                    active_turn = Some((turn_cancellation, handle));
                }
            }
        }
    }

    /// `previous_turn` is the still-active previous turn's cancellation
    /// token, if any — only ever non-`None` in wake-phrase mode (in
    /// `respond_to_all` mode `run_orchestrator` already cancels it on
    /// `SpeechConfirmed`, before this function is even called for the new
    /// utterance).
    async fn handle_utterance(&self, pcm: Vec<i16>, cancellation: &CancellationToken, previous_turn: Option<CancellationToken>) {
        self.event_bus.publish(AppEvent::CallStateChanged { state: CallState::Transcribing });

        let text = match self.transcribe(pcm).await {
            Ok(text) => text,
            Err(err) => {
                tracing::warn!(target: "system", "call: la transcripción falló: {err}");
                self.event_bus.publish(AppEvent::CallError { message: err.to_string() });
                self.event_bus.publish(AppEvent::CallStateChanged { state: CallState::Error });
                return;
            }
        };

        self.event_bus.publish(AppEvent::CallUserUtterance { text: text.clone() });
        if text.trim().is_empty() {
            return; // Noise that tripped the VAD but transcribed to nothing.
        }

        if !self.should_respond_to(&text).await {
            // Heard and transcribed, but no wake phrase in it — stays in
            // the transcript, just never reaches the LLM/TTS. Lets a call
            // stay open while gaming/streaming without the character
            // replying to every single thing said, and — in wake-phrase
            // mode — without cutting off whatever it was already saying,
            // since `previous_turn` is left untouched here.
            return;
        }

        // This utterance is confirmed to deserve a reply — if the
        // character was still finishing a previous turn (only possible in
        // wake-phrase mode; `respond_to_all` mode already cancelled it
        // eagerly on `SpeechConfirmed`), cut it off now, right before this
        // one takes over.
        if let Some(previous) = previous_turn {
            previous.cancel();
            if let Err(err) = self.tts.stop().await {
                tracing::warn!(target: "system", "call: no se pudo detener la voz al interrumpir: {err}");
            }
        }

        self.event_bus.publish(AppEvent::CallStateChanged { state: CallState::Thinking });
        let (profile, reply) = match self.generate_reply(&text, cancellation).await {
            Ok(result) => result,
            Err(err) => {
                // A cancelled generation (this turn itself got
                // barge-in'd, or the call was hung up mid-turn) isn't a
                // real failure — surfacing it as `CallError` would show a
                // spurious error for what's actually an intentional
                // interruption.
                if cancellation.is_cancelled() {
                    // Lo que dijo el usuario cuenta aunque este turno se haya
                    // interrumpido: el siguiente debe saber que se lo dijo.
                    self.history.lock().expect("call history poisoned").push(MessageRole::User, &text);
                    return;
                }
                self.history.lock().expect("call history poisoned").push(MessageRole::User, &text);
                tracing::warn!(target: "system", "call: la generación falló: {err}");
                self.event_bus.publish(AppEvent::CallError { message: err.to_string() });
                self.event_bus.publish(AppEvent::CallStateChanged { state: CallState::Error });
                return;
            }
        };
        {
            let mut history = self.history.lock().expect("call history poisoned");
            history.push(MessageRole::User, &text);
            history.push(MessageRole::Assistant, &reply);
        }
        if reply.is_empty() {
            return;
        }

        self.event_bus.publish(AppEvent::CallCharacterReply { text: reply.clone() });
        self.event_bus.publish(AppEvent::CallStateChanged { state: CallState::Speaking });
        self.speak_and_wait(&profile, &reply, cancellation).await;
    }
}

/// `aivt-service-settings` and `aivt-provider-stt` each define their own
/// model-size enum (settings doesn't depend on that provider crate) —
/// this just translates between them.
fn to_whisper_model_size(size: SttModelSize) -> WhisperModelSize {
    match size {
        SttModelSize::Tiny => WhisperModelSize::Tiny,
        SttModelSize::Base => WhisperModelSize::Base,
        SttModelSize::Small => WhisperModelSize::Small,
        SttModelSize::Medium => WhisperModelSize::Medium,
    }
}

/// `Arc`-wrapped from the start, same reasoning as `TwitchService`: the
/// background orchestrator task spawned by `start()` needs an owned,
/// `'static` handle back into this service's state.
#[derive(Clone)]
pub struct CallService(Arc<Inner>);

impl CallService {
    pub fn new(
        pool: DbPool,
        event_bus: EventBus,
        secrets: Arc<dyn SecretStore>,
        tts: Arc<dyn TTSProvider>,
        stt_models: SttModelManager,
        memory: Arc<dyn MemoryManager>,
    ) -> Self {
        Self(Arc::new(Inner {
            pool,
            event_bus,
            secrets,
            tts,
            stt_models,
            active_profile_id: RwLock::new(None),
            session: Mutex::new(None),
            dictation: Mutex::new(None),
            mic_test: Mutex::new(None),
            history: std::sync::Mutex::new(CallHistory::default()),
            memory,
        }))
    }

    pub async fn set_active_profile(&self, profile_id: Option<String>) {
        *self.0.active_profile_id.write().await = profile_id;
    }

    /// No-op when no call is active (nothing to mute yet) — the frontend
    /// only exposes this control while `Running`, but a stray call from a
    /// stale UI state shouldn't error.
    pub async fn set_muted(&self, muted: bool) {
        if let Some(session) = self.0.session.lock().await.as_ref() {
            session.muted.store(muted, Ordering::Relaxed);
        }
    }

    /// Empieza a grabar el micrófono para dictar un mensaje (botón de micrófono
    /// del chat). Falla de entrada, con un mensaje claro, si hay una llamada en
    /// curso o si todavía no hay un modelo de transcripción descargado — mejor
    /// enterarse acá que después de haber hablado un minuto.
    pub async fn dictation_start(&self) -> Result<(), CallError> {
        if self.0.session.lock().await.is_some() {
            return Err(CallError::CallInProgress);
        }
        let settings = self.0.settings();
        let model = settings.stt_model().await.map_err(|err| CallError::Settings(err.to_string()))?;
        let wanted = to_whisper_model_size(model.size);
        if !self.0.stt_models.list_status().iter().any(|status| status.size == wanted && status.downloaded) {
            return Err(CallError::NoSttModel);
        }
        let policy = settings.call_policy().await.map_err(|err| CallError::Settings(err.to_string()))?;

        let mut dictation = self.0.dictation.lock().await;
        // Un segundo `start` sin `stop` reemplaza la grabación anterior.
        let samples = Arc::new(std::sync::Mutex::new(Vec::<i16>::new()));
        let sink = samples.clone();
        let mic = MicCapture::start(policy.microphone.clone(), move |chunk: &[i16]| {
            let mut buffer = sink.lock().expect("dictation buffer poisoned");
            let room = MAX_DICTATION_SAMPLES.saturating_sub(buffer.len());
            buffer.extend_from_slice(&chunk[..chunk.len().min(room)]);
        })?;
        let cancel = CancellationToken::new();
        let inner = self.0.clone();
        let partial_samples = samples.clone();
        let partial_cancel = cancel.clone();
        tokio::spawn(async move {
            inner.run_dictation_partials(partial_samples, partial_cancel).await;
        });
        // Reemplazar una grabación previa suelta su `Dictation` (y cancela su tarea).
        *dictation = Some(Dictation { _mic: mic, samples, cancel });
        Ok(())
    }

    /// Termina la grabación y devuelve el texto transcrito (vacío si no se
    /// entendió nada). `profile_id` solo sirve para sesgar el reconocimiento
    /// hacia el nombre del personaje, igual que en una llamada.
    pub async fn dictation_stop(&self, profile_id: &str) -> Result<String, CallError> {
        let Some(dictation) = self.0.dictation.lock().await.take() else {
            return Ok(String::new());
        };
        let samples = dictation.samples.clone();
        drop(dictation); // cierra el micrófono y cancela los provisionales antes de transcribir
        let pcm = std::mem::take(&mut *samples.lock().expect("dictation buffer poisoned"));
        if pcm.len() < MIN_DICTATION_SAMPLES {
            return Ok(String::new());
        }
        if is_near_silence(&pcm) {
            // Whisper "alucina" texto sobre audio sin voz; ni se lo mandamos.
            return Ok(String::new());
        }
        let prompt = self.0.transcription_prompt_for(profile_id).await;
        let text = self.0.transcribe_with_prompt(pcm, prompt).await?;
        Ok(clean_dictation(&text))
    }

    /// Abre el micrófono elegido solo para publicar su nivel (`CallMicLevel`),
    /// sin transcribir ni guardar nada: la barra de Configuración > Llamada
    /// para comprobar que el micrófono correcto se oye. No hace nada si hay
    /// una llamada en curso (ya publica el nivel ella misma).
    pub async fn mic_test_start(&self) -> Result<(), CallError> {
        if self.0.session.lock().await.is_some() {
            return Ok(());
        }
        let policy = self.0.settings().call_policy().await.map_err(|err| CallError::Settings(err.to_string()))?;
        let bus = self.0.event_bus.clone();
        let mut meter = LevelMeter::new(LEVEL_INTERVAL);
        let mic = MicCapture::start(policy.microphone.clone(), move |chunk: &[i16]| {
            if let Some(level) = meter.push(chunk) {
                bus.publish(AppEvent::CallMicLevel { level });
            }
        })?;
        // Reemplaza una prueba anterior (p. ej. al cambiar de micrófono).
        *self.0.mic_test.lock().await = Some(mic);
        Ok(())
    }

    pub async fn mic_test_stop(&self) {
        self.0.mic_test.lock().await.take();
    }

    /// Descarta la grabación en curso sin transcribir.
    pub async fn dictation_cancel(&self) {
        self.0.dictation.lock().await.take();
    }

    pub async fn is_muted(&self) -> bool {
        match self.0.session.lock().await.as_ref() {
            Some(session) => session.muted.load(Ordering::Relaxed),
            None => false,
        }
    }
}

#[async_trait::async_trait]
impl ManagedService for CallService {
    fn name(&self) -> &str {
        "call"
    }

    /// Opens the mic and starts the capture → VAD → STT → IA → TTS loop —
    /// stays `Running` for as long as the call lasts, looping through
    /// arbitrarily many utterance→reply turns on its own (same "start" =
    /// "begin listening" semantics as `TwitchService`).
    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let policy = self.0.settings().call_policy().await.map_err(|err| box_err(CallError::Settings(err.to_string())))?;

        let cancellation = CancellationToken::new();
        let (frame_tx, frame_rx) = std_mpsc::channel::<Vec<i16>>();
        let (event_tx, event_rx) = tokio_mpsc::unbounded_channel::<SegmenterEvent>();

        // Segmenter thread: owns the VAD (its C library state lives and
        // dies on one thread — `webrtc_vad::Vad` wraps a raw `*mut Fvad`
        // and isn't `Send`, so it's built *inside* the spawned closure
        // rather than passed in already-constructed).
        let vad_mode = policy.vad_mode;
        let silence_timeout_ms = policy.silence_timeout_ms;
        std::thread::spawn(move || {
            let mut segmenter = Segmenter::new(vad_mode, silence_timeout_ms);
            for chunk in frame_rx {
                for event in segmenter.push(&chunk) {
                    if event_tx.send(event).is_err() {
                        return; // orchestrator task already ended
                    }
                }
            }
        });

        // Capture thread (owned by `MicCapture`): the audio callback just
        // forwards raw samples — no VAD work happens on the realtime audio
        // thread itself. Checking `muted` here (before anything reaches
        // the segmenter) is deliberately the *only* place mute is
        // implemented — no separate "pause"/"resume" path for the
        // capture stream or the segmenter thread to keep in sync with.
        let muted = Arc::new(AtomicBool::new(false));
        let muted_for_capture = muted.clone();

        // Con altavoces, lo que ella dice vuelve por el micrófono: mientras
        // habla (y un instante después) se le pasa silencio al detector. Se
        // manda silencio en vez de no mandar nada para que el segmentador
        // siga contando el tiempo igual.
        let echo_guard = policy.echo_guard.then(|| Arc::new(EchoGuard::new()));
        if let Some(guard) = &echo_guard {
            let guard = guard.clone();
            let mut events = self.0.event_bus.subscribe();
            let guard_cancellation = cancellation.clone();
            tokio::spawn(async move {
                loop {
                    let event = tokio::select! {
                        () = guard_cancellation.cancelled() => return,
                        event = events.recv() => event,
                    };
                    match event {
                        Ok(AppEvent::VoicePlaybackStarted { .. }) => guard.playback_started(),
                        Ok(AppEvent::VoicePlaybackFinished { .. } | AppEvent::VoicePlaybackError { .. }) => guard.playback_ended(),
                        Ok(_) => {}
                        // Si se perdió el aviso de fin, mejor soltar el micrófono
                        // que dejarlo ignorado para siempre.
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => guard.playback_ended(),
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
                    }
                }
            });
        }

        // El micrófono de prueba de Configuración se cierra: ya no hace falta.
        self.0.mic_test.lock().await.take();

        let level_bus = self.0.event_bus.clone();
        let mut meter = LevelMeter::new(LEVEL_INTERVAL);
        let mic = MicCapture::start(policy.microphone.clone(), move |samples: &[i16]| {
            let is_muted = muted_for_capture.load(Ordering::Relaxed);
            // Silenciado, la barra baja a cero en vez de quedarse congelada.
            if let Some(level) = meter.push(if is_muted { &[] } else { samples }) {
                level_bus.publish(AppEvent::CallMicLevel { level });
            }
            if is_muted {
                return;
            }
            if echo_guard.as_ref().is_some_and(|guard| guard.is_holding()) {
                let _ = frame_tx.send(vec![0; samples.len()]);
                return;
            }
            let _ = frame_tx.send(samples.to_vec());
        })
        .map_err(box_err)?;

        let inner = self.0.clone();
        let orchestrator_cancellation = cancellation.clone();
        tokio::spawn(async move {
            inner.run_orchestrator(event_rx, orchestrator_cancellation).await;
        });

        // Una llamada nueva empieza sin recuerdo de la anterior.
        self.0.history.lock().expect("call history poisoned").clear();
        *self.0.session.lock().await = Some(Session { _mic: mic, cancellation, muted });
        Ok(())
    }

    async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.0.history.lock().expect("call history poisoned").clear();
        if let Some(session) = self.0.session.lock().await.take() {
            session.cancellation.cancel();
            // Dropping `session` here drops `_mic`, stopping the `cpal`
            // stream and closing `frame_tx` — the segmenter thread's `for
            // chunk in frame_rx` loop ends on its own once that happens,
            // which drops `event_tx` and ends the orchestrator task's loop
            // too, even without the cancellation above.
        }
        // Cuts any in-progress speech immediately — hanging up mid-reply
        // shouldn't let a long response keep playing out audibly after
        // the call has visibly ended. Safe to call even when nothing is
        // speaking (see `TTSProvider::stop`'s own doc comment).
        if let Err(err) = self.0.tts.stop().await {
            tracing::warn!(target: "system", "call: no se pudo detener la voz al colgar: {err}");
        }
        Ok(())
    }
}

#[cfg(test)]
mod dictation_tests {
    use super::*;

    #[test]
    fn bracketed_sound_annotations_are_removed() {
        assert_eq!(clean_dictation("[Música]"), "");
        assert_eq!(clean_dictation("[Para el video] [Música]"), "");
        assert_eq!(clean_dictation("Hola [Aplausos] qué tal"), "Hola qué tal");
    }

    #[test]
    fn plain_speech_is_kept_and_whitespace_is_collapsed() {
        assert_eq!(clean_dictation("  Hola,   ¿qué tal?  "), "Hola, ¿qué tal?");
    }

    #[test]
    fn an_unclosed_bracket_does_not_swallow_the_rest_by_panicking() {
        assert_eq!(clean_dictation("Hola [ruido"), "Hola");
    }

    #[test]
    fn quiet_audio_counts_as_silence_and_speech_does_not() {
        assert!(is_near_silence(&[0, 120, -300, 499]));
        assert!(!is_near_silence(&[0, 120, 4000, -300]));
        assert!(is_near_silence(&[]));
    }
}
