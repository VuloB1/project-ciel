//! `TTSProvider` trait + its implementation: native OS voices are always
//! available (`NativeEngine`), with ElevenLabs (`ElevenLabsEngine`),
//! Google Cloud (`GoogleEngine`), Azure Speech (`AzureEngine`) or a
//! generic OpenAI-compatible HTTP endpoint (`HttpEngine`, also how the
//! `OpenAi` kind talks) as optional backends the user can switch to in
//! Configuración > Motor de voz — see docs/ADR/003-provider-architecture.md
//! and `aivt_service_settings::TtsProviderEntry`.

mod audio_player;
mod azure;
mod elevenlabs;
mod error;
mod chunking;
mod deepgram;
mod fish;
mod google;
mod http;
mod markdown;
mod native;
mod types;

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use aivt_eventbus::EventBus;

pub use error::TtsError;
pub use markdown::strip_markdown;
pub use types::{SpeakRequest, TtsBackend, VoiceInfo};

use audio_player::AudioPlayer;
pub use audio_player::list_output_devices;
use azure::AzureEngine;
use elevenlabs::ElevenLabsEngine;
use deepgram::DeepgramEngine;
use fish::FishAudioEngine;
use google::GoogleEngine;
use http::{HttpEngine, GROQ, TOGETHER};
use native::NativeEngine;

/// Groq's Orpheus voices (from its docs): English model vs Saudi-Arabic model.
fn groq_voices(model: Option<&str>) -> Vec<VoiceInfo> {
    let (language, names): (&str, &[&str]) = if model.is_some_and(|m| m.contains("arabic")) {
        ("ar", &["abdullah", "fahad", "sultan", "lulwa", "noura", "aisha"])
    } else {
        ("en", &["autumn", "diana", "hannah", "austin", "daniel", "troy"])
    };
    names.iter().map(|n| static_voice(n, language)).collect()
}

/// Together's Orpheus voices (from its docs). Other Together models
/// (Kokoro, Cartesia, ...) each have their own set: for those the picker is
/// empty and the id is typed by hand.
fn together_voices(model: Option<&str>) -> Vec<VoiceInfo> {
    if model.map(str::trim).filter(|m| !m.is_empty()).is_some_and(|m| !m.contains("orpheus")) {
        return Vec::new();
    }
    ["tara", "leah", "jess", "leo", "dan", "mia", "zac", "zoe"].iter().map(|n| static_voice(n, "en")).collect()
}

fn static_voice(id: &str, language: &str) -> VoiceInfo {
    let mut name = id.to_string();
    if let Some(first) = name.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    VoiceInfo { id: id.to_string(), name, language: Some(language.to_string()) }
}

#[async_trait]
pub trait TTSProvider: Send + Sync {
    /// Non-empty for `TtsBackend::Native` and `TtsBackend::ElevenLabs` —
    /// see `VoiceInfo`'s doc comment for why the generic HTTP backend
    /// can't be enumerated.
    async fn list_voices(&self, backend: &TtsBackend) -> Result<Vec<VoiceInfo>, TtsError>;

    /// Searches the active service's voice catalogue by name — only Fish
    /// Audio has one big enough to need it; every other backend returns an
    /// empty list (their `list_voices` already is the whole set).
    async fn search_voices(&self, _backend: &TtsBackend, _query: &str) -> Result<Vec<VoiceInfo>, TtsError> {
        Ok(Vec::new())
    }

    /// Synthesizes and plays back through the shared audio output, same as
    /// `speak()`, but also returns the raw encoded bytes (always MP3) when
    /// the active backend can produce a capturable buffer — `Http` and
    /// `ElevenLabs`, which fetch audio over the network. `None` for
    /// `Native`, whose OS-level speech engine speaks directly with no byte
    /// buffer to hand back at all. Chat's optional "guardar audio de los
    /// mensajes" persistence (see `aivt_service_chat`'s `maybe_speak`) is
    /// the only caller that reads the returned bytes — every other caller
    /// uses `speak()` and discards them.
    async fn speak_and_capture(&self, request: SpeakRequest) -> Result<Option<Vec<u8>>, TtsError>;

    async fn speak(&self, request: SpeakRequest) -> Result<(), TtsError> {
        self.speak_and_capture(request).await.map(|_| ())
    }

    /// Stops whatever's currently speaking, regardless of which backend is
    /// active right now — safe to call unconditionally, even when nothing
    /// is speaking.
    async fn stop(&self) -> Result<(), TtsError>;
    /// Global mute — a single switch covering every caller (Chat, Twitch,
    /// Discord, Llamada) uniformly, independent of each profile's own
    /// `voice.enabled` setting. `speak()` checks it before touching either
    /// backend. Default no-op implementations so test doubles that don't
    /// care about muting (`MockTTSProvider`) don't need to implement it —
    /// only `AivtTtsProvider` overrides these. See
    /// `commands::voice::voice_set_muted` (Sidebar's mute button, next to
    /// "mostrar ventana flotante") for the only real caller.
    fn set_muted(&self, _muted: bool) {}
    fn is_muted(&self) -> bool {
        false
    }

    /// Whether the main window is currently hidden in the system tray —
    /// updated by `src-tauri`'s window close/focus handling. Combined with
    /// `set_voice_while_hidden_enabled` below to decide whether a
    /// `speak()` call goes through; see `speak_and_capture`. Same
    /// default-no-op reasoning as `set_muted`.
    fn set_window_hidden(&self, _hidden: bool) {}
    /// Persisted opt-in ("Reproducir voz estando en la bandeja",
    /// Configuración > General) — off by default. See
    /// `aivt_service_settings::BackgroundSettings::voice_while_hidden`.
    fn set_voice_while_hidden_enabled(&self, _enabled: bool) {}
    /// Dispositivo de salida de las voces que se reproducen aquí mismo
    /// (todas menos la voz del sistema, que siempre usa la salida
    /// predeterminada de Windows). `None` = la predeterminada.
    fn set_output_device(&self, _name: Option<String>) {}
}

/// The one `TTSProvider` this app constructs. Routes each call to native,
/// ElevenLabs, or generic HTTP per `SpeakRequest::backend` rather than
/// needing separate provider instances swapped in and out of `AppState` —
/// see the type's fields for why each engine is built the way it is.
pub struct AivtTtsProvider {
    /// `None` when native synthesis failed to initialize (see
    /// `NativeEngine::new`) — every native-backend call then fails with
    /// `TtsError::Unavailable` instead of the app refusing to start.
    native: Option<NativeEngine>,
    http: HttpEngine,
    elevenlabs: ElevenLabsEngine,
    google: GoogleEngine,
    fish: FishAudioEngine,
    deepgram: DeepgramEngine,
    azure: AzureEngine,
    /// Shared by every backend that produces a byte buffer to play
    /// (`Http`/`ElevenLabs`) — owned here rather than inside each engine so
    /// `speak_and_capture` can play the same bytes it hands back for
    /// Chat's optional persistence, instead of fetching audio twice.
    audio: Arc<AudioPlayer>,
    event_bus: EventBus,
    /// Backing store for `TTSProvider::{set_muted,is_muted}` — a plain
    /// `AtomicBool` rather than a DB read on every `speak()` call, since
    /// this is checked on a hot, latency-sensitive path (see
    /// `aivt_service_call`'s barge-in for how much that path already cares
    /// about response time). The persisted source of truth lives in
    /// `SettingsService::voice_muted`; `src-tauri`'s `run()` seeds this
    /// field from it once at startup so a restart remembers the last mute
    /// state.
    /// Sube con cada `speak` y cada `stop`: una síntesis remota que termina
    /// después de que cambió (el usuario pulsó stop, o empezó otra voz) ya no
    /// reproduce su audio. Sin esto, parar mientras el motor remoto todavía
    /// generaba no hacía nada y el audio sonaba igual al llegar.
    epoch: AtomicU64,
    muted: AtomicBool,
    /// See `TTSProvider::set_window_hidden`.
    window_hidden: AtomicBool,
    /// See `TTSProvider::set_voice_while_hidden_enabled`. Persisted source
    /// of truth is `SettingsService::background_settings`, seeded here at
    /// startup same as `muted` is from `voice_muted`.
    voice_while_hidden: AtomicBool,
}

impl AivtTtsProvider {
    pub fn new(event_bus: EventBus, muted: bool, voice_while_hidden: bool) -> Self {
        let native = match NativeEngine::new(event_bus.clone()) {
            Ok(engine) => Some(engine),
            Err(err) => {
                tracing::warn!(target: "system", "native TTS unavailable, only the HTTP backend will work: {err}");
                None
            }
        };

        let audio = match AudioPlayer::spawn(event_bus.clone()) {
            Ok(player) => Arc::new(player),
            Err(err) => {
                // Extremely unlikely (no audio output device at all) — the
                // HTTP backend degrades to "requests succeed, nothing is
                // heard" rather than the app failing to start over it.
                tracing::warn!(target: "system", "could not open an audio output device for the HTTP TTS backend: {err}");
                Arc::new(AudioPlayer::dead())
            }
        };

        let http_client = reqwest::Client::new();
        Self {
            native,
            http: HttpEngine::new(http_client.clone()),
            elevenlabs: ElevenLabsEngine::new(http_client.clone()),
            google: GoogleEngine::new(http_client.clone()),
            fish: FishAudioEngine::new(http_client.clone()),
            deepgram: DeepgramEngine::new(http_client.clone()),
            azure: AzureEngine::new(http_client),
            audio,
            event_bus,
            epoch: AtomicU64::new(0),
            muted: AtomicBool::new(muted),
            window_hidden: AtomicBool::new(false),
            voice_while_hidden: AtomicBool::new(voice_while_hidden),
        }
    }
}

#[async_trait]
impl TTSProvider for AivtTtsProvider {
    async fn search_voices(&self, backend: &TtsBackend, query: &str) -> Result<Vec<VoiceInfo>, TtsError> {
        match backend {
            TtsBackend::FishAudio { api_key, .. } => self.fish.search_voices(api_key, query).await,
            _ => Ok(Vec::new()),
        }
    }

    async fn list_voices(&self, backend: &TtsBackend) -> Result<Vec<VoiceInfo>, TtsError> {
        match backend {
            TtsBackend::Native => match &self.native {
                Some(engine) => engine.list_voices(),
                None => Err(TtsError::Unavailable("no hay un motor de voz nativo disponible en este sistema".to_string())),
            },
            // Sin endpoint de voces: la pestaña Voz pide el id/modelo en texto libre.
            TtsBackend::Http { .. } | TtsBackend::Deepgram { .. } => Ok(Vec::new()),
            TtsBackend::FishAudio { api_key, .. } => self.fish.list_voices(api_key).await,
            TtsBackend::Groq { model, .. } => Ok(groq_voices(model.as_deref())),
            TtsBackend::Together { model, .. } => Ok(together_voices(model.as_deref())),
            TtsBackend::ElevenLabs { api_key } => self.elevenlabs.list_voices(api_key).await,
            TtsBackend::Google { api_key } => self.google.list_voices(api_key).await,
            TtsBackend::Azure { api_key, region } => self.azure.list_voices(api_key, region).await,
        }
    }

    async fn speak_and_capture(&self, request: SpeakRequest) -> Result<Option<Vec<u8>>, TtsError> {
        if self.muted.load(Ordering::Relaxed) {
            tracing::info!(target: "system", "voz global silenciada, se omite la reproducción");
            return Ok(None);
        }
        if self.window_hidden.load(Ordering::Relaxed) && !self.voice_while_hidden.load(Ordering::Relaxed) {
            tracing::info!(target: "system", "ventana minimizada en la bandeja y voz en segundo plano desactivada, se omite la reproducción");
            return Ok(None);
        }
        // The character's replies are Markdown (Fase 11: the Chat bubble
        // now renders it) — reading `**bold**`/`[link](url)` syntax or a
        // raw URL aloud verbatim is worse than just the words underneath.
        // One transform here covers every caller (auto-play, the manual
        // speak button, Twitch/Discord/Llamada) since they all funnel
        // through this same method.
        let spoken_text = markdown::strip_markdown(&request.text);
        let epoch = self.epoch.fetch_add(1, Ordering::SeqCst) + 1;
        match &request.backend {
            TtsBackend::Native => {
                match &self.native {
                    Some(engine) => engine.speak(
                        &spoken_text,
                        &request.voice_id,
                        request.rate,
                        request.pitch,
                        request.volume,
                        request.profile_id,
                        request.message_id,
                        &self.event_bus,
                    )?,
                    None => return Err(TtsError::Unavailable("no hay un motor de voz nativo disponible en este sistema".to_string())),
                }
                Ok(None)
            }
            TtsBackend::Http { base_url, api_key } => {
                let bytes = self.http.synthesize(&spoken_text, &request.voice_id, request.rate, base_url, api_key.as_deref()).await?;
                if self.epoch.load(Ordering::SeqCst) != epoch {
                    // Mientras se sintetizaba, el usuario pulsó stop (o empezó otra voz):
                    // el audio llega tarde y no debe sonar.
                    return Ok(Some(bytes));
                }
                self.audio.play(bytes.clone(), request.profile_id, request.message_id).map_err(TtsError::Playback)?;
                Ok(Some(bytes))
            }
            TtsBackend::ElevenLabs { api_key } => {
                let bytes = self.elevenlabs.synthesize(&spoken_text, &request.voice_id, request.rate, api_key).await?;
                if self.epoch.load(Ordering::SeqCst) != epoch {
                    // Mientras se sintetizaba, el usuario pulsó stop (o empezó otra voz):
                    // el audio llega tarde y no debe sonar.
                    return Ok(Some(bytes));
                }
                self.audio.play(bytes.clone(), request.profile_id, request.message_id).map_err(TtsError::Playback)?;
                Ok(Some(bytes))
            }
            TtsBackend::Google { api_key } => {
                let bytes = self.google.synthesize(&spoken_text, &request.voice_id, request.rate, api_key).await?;
                if self.epoch.load(Ordering::SeqCst) != epoch {
                    // Mientras se sintetizaba, el usuario pulsó stop (o empezó otra voz):
                    // el audio llega tarde y no debe sonar.
                    return Ok(Some(bytes));
                }
                self.audio.play(bytes.clone(), request.profile_id, request.message_id).map_err(TtsError::Playback)?;
                Ok(Some(bytes))
            }
            TtsBackend::FishAudio { api_key, model } => {
                let bytes = self.fish.synthesize(&spoken_text, &request.voice_id, request.rate, api_key, model.as_deref()).await?;
                if self.epoch.load(Ordering::SeqCst) != epoch {
                    // Mientras se sintetizaba, el usuario pulsó stop (o empezó otra voz):
                    // el audio llega tarde y no debe sonar.
                    return Ok(Some(bytes));
                }
                self.audio.play(bytes.clone(), request.profile_id, request.message_id).map_err(TtsError::Playback)?;
                Ok(Some(bytes))
            }
            TtsBackend::Groq { api_key, model } => {
                let bytes = self.http.synthesize_preset(&GROQ, &spoken_text, &request.voice_id, request.rate, api_key, model.as_deref()).await?;
                if self.epoch.load(Ordering::SeqCst) != epoch {
                    return Ok(Some(bytes));
                }
                self.audio.play(bytes.clone(), request.profile_id, request.message_id).map_err(TtsError::Playback)?;
                Ok(Some(bytes))
            }
            TtsBackend::Together { api_key, model } => {
                let bytes = self.http.synthesize_preset(&TOGETHER, &spoken_text, &request.voice_id, request.rate, api_key, model.as_deref()).await?;
                if self.epoch.load(Ordering::SeqCst) != epoch {
                    return Ok(Some(bytes));
                }
                self.audio.play(bytes.clone(), request.profile_id, request.message_id).map_err(TtsError::Playback)?;
                Ok(Some(bytes))
            }
            TtsBackend::Deepgram { api_key } => {
                let bytes = self.deepgram.synthesize(&spoken_text, &request.voice_id, api_key).await?;
                if self.epoch.load(Ordering::SeqCst) != epoch {
                    // Mientras se sintetizaba, el usuario pulsó stop (o empezó otra voz):
                    // el audio llega tarde y no debe sonar.
                    return Ok(Some(bytes));
                }
                self.audio.play(bytes.clone(), request.profile_id, request.message_id).map_err(TtsError::Playback)?;
                Ok(Some(bytes))
            }
            TtsBackend::Azure { api_key, region } => {
                let bytes = self.azure.synthesize(&spoken_text, &request.voice_id, request.rate, api_key, region).await?;
                if self.epoch.load(Ordering::SeqCst) != epoch {
                    // Mientras se sintetizaba, el usuario pulsó stop (o empezó otra voz):
                    // el audio llega tarde y no debe sonar.
                    return Ok(Some(bytes));
                }
                self.audio.play(bytes.clone(), request.profile_id, request.message_id).map_err(TtsError::Playback)?;
                Ok(Some(bytes))
            }
        }
    }

    async fn stop(&self) -> Result<(), TtsError> {
        self.epoch.fetch_add(1, Ordering::SeqCst);
        // Las dos cosas se intentan siempre: un error al parar la voz nativa
        // no debe dejar sonando el audio de un motor remoto.
        let native = match &self.native {
            Some(engine) => engine.stop(),
            None => Ok(()),
        };
        let audio = self.audio.stop().map_err(TtsError::Playback);
        native.and(audio)
    }

    fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    fn set_output_device(&self, name: Option<String>) {
        if let Err(err) = self.audio.set_device(name) {
            tracing::warn!(target: "system", "voz: no se pudo cambiar la salida de audio: {err}");
        }
    }

    fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    fn set_window_hidden(&self, hidden: bool) {
        self.window_hidden.store(hidden, Ordering::Relaxed);
    }

    fn set_voice_while_hidden_enabled(&self, enabled: bool) {
        self.voice_while_hidden.store(enabled, Ordering::Relaxed);
    }
}
