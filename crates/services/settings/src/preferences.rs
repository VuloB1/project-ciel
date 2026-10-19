//! Preferencias de la app: apariencia, segundo plano, audio del chat, dictado, personas, avatar para OBS y banderas sueltas.

use serde::{Deserialize, Serialize};

use crate::service::SettingsService;
use crate::SettingsError;

const KEY_DICTATION_SETTINGS: &str = "dictation_settings";

const KEY_PEOPLE_POLICY: &str = "people_policy";

const KEY_VOICE_MUTED: &str = "voice_muted";
const KEY_AUDIO_OUTPUT_DEVICE: &str = "audio_output_device";

const KEY_BACKGROUND_SETTINGS: &str = "background_settings";

const KEY_CHAT_AUDIO_SETTINGS: &str = "chat_audio_settings";

const KEY_AVATAR_SERVER_SETTINGS: &str = "avatar_server_settings";

// v2: `roundness` cambió de `u8` (default 100, siempre "tocado") a
// `Option<u8>` (default `None`, "no tocado" = usar el radio de cada tema) —
// se bumpea la key para que un blob viejo con `roundness: 100` (guardado
// bajo la semántica anterior) no se reinterprete como "el usuario eligió
// 100% de override" y termine redondeando temas de esquinas cuadradas que
// nunca tocó.
const KEY_APPEARANCE_SETTINGS: &str = "appearance_settings_v2";

const KEY_ONBOARDING_COMPLETED: &str = "onboarding_completed";

/// Whether the local HTTP/WebSocket server that serves the reactive avatar
/// as an OBS Browser Source (`aivt-service-avatar`) is on, and which port it
/// binds to (loopback only — see that crate). Opt-in like the other
/// integrations (`enabled: false` by default) so a fresh install never binds
/// a port without the user asking for it, but unlike Twitch/Discord/Call it
/// auto-resumes on every future launch once turned on once — see
/// `src-tauri/src/lib.rs`'s `setup()`, which reads this once at boot to
/// decide whether to start the `ManagedService` right away.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarServerSettings {
    pub enabled: bool,
    pub port: u16,
}

impl Default for AvatarServerSettings {
    fn default() -> Self {
        Self { enabled: false, port: 47823 }
    }
}

/// Preferencias de apariencia globales de la app (no por personaje — ver
/// `Profile::theme`/`accent_color` para eso, que siguen intactos). Modo
/// Oscuro/Claro/Sistema, redondez y tamaño de fuente afectan las variables
/// CSS que ya escribe `useActiveProfileTheme`/`useAppearanceSettings` en el
/// frontend; `font_id` indexa el catálogo curado de Google Fonts
/// auto-hosteadas en `src/lib/theme/fonts.ts` (el backend no valida el id —
/// un id desconocido cae a Inter del lado del frontend).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceSettings {
    pub color_mode: ColorMode,
    /// Override GLOBAL de redondez, independiente del radio propio de cada
    /// tema (`ThemeTokens.radiusSm/Md/Lg` en el frontend) — `None` (default)
    /// significa "no lo tocó": cada tema se ve con su propio radio, sin
    /// cambios (un tema de esquinas cuadradas como Terminal/Noir/Cyberpunk/
    /// Industrial se queda cuadrado). En cuanto el usuario mueve el slider
    /// se guarda `Some(percent)` — 0 = esquinas totalmente cuadradas, 100 =
    /// redondez media, 200 = muy redondeado — y ESE valor pasa a aplicarse
    /// igual sin importar qué tema elija después, hasta que lo restablezca
    /// (vuelve a `None`). Ver `scaleRadius`/`applyRoundness` en
    /// `useActiveProfileTheme.ts` para el cálculo real.
    pub roundness: Option<u8>,
    pub density: MessageDensity,
    pub font_id: String,
    /// Porcentaje de escala del tamaño de fuente — 80 a 140, mismo rango
    /// que el slider de la referencia (OpChat).
    pub font_scale: u8,
    /// Decoraciones festivas: `"off"`, `"auto"` (según la fecha) o el id de una festividad para
    /// forzarla. El backend no valida el id: uno desconocido no muestra nada en el frontend.
    #[serde(default = "default_seasonal")]
    pub seasonal: String,
    /// Si las partículas de los adornos se mueven.
    #[serde(default = "default_seasonal_motion")]
    pub seasonal_motion: bool,
    /// Ids de las festividades que el usuario apagó: el modo automático las salta. Vacío = todas activas
    /// (así una festividad nueva en una versión futura aparece sin tocar nada).
    #[serde(default)]
    pub seasonal_disabled: Vec<String>,
}

fn default_seasonal() -> String {
    "off".to_string()
}

fn default_seasonal_motion() -> bool {
    true
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self {
            color_mode: ColorMode::Dark,
            roundness: None,
            density: MessageDensity::Normal,
            font_id: "inter".to_string(),
            font_scale: 100,
            seasonal: default_seasonal(),
            seasonal_motion: default_seasonal_motion(),
            seasonal_disabled: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColorMode {
    Dark,
    Light,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDensity {
    Compact,
    Normal,
    Spacious,
}

/// Controls the registro-de-personas feature (`aivt-memory`'s
/// `PeopleManager`) — whether Twitch chatters / Discord message authors get
/// auto-registered by name as they talk, feeding `CallService::
/// transcription_prompt`'s Whisper priming. Both on by default: the whole
/// point is recognizing names without the user having to configure
/// anything first, and everything stays local (see docs/security.md).
/// Neither `TwitchService` nor `DiscordService` reads this directly —
/// `PeopleService::observe_twitch_chatter`/`observe_discord_author` check
/// it and no-op when off, so each integration's own logic stays unaware
/// the toggle exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeoplePolicy {
    pub twitch_auto_learn_enabled: bool,
    /// `#[serde(default = "default_people_auto_learn")]` so a policy saved
    /// before Discord auto-registration existed (only
    /// `twitchAutoLearnEnabled` in the stored JSON) still loads, defaulting
    /// this newer toggle to on — same reasoning as every other
    /// `#[serde(default)]` field in this file.
    #[serde(default = "default_people_auto_learn")]
    pub discord_auto_learn_enabled: bool,
}

impl Default for PeoplePolicy {
    fn default() -> Self {
        Self { twitch_auto_learn_enabled: true, discord_auto_learn_enabled: true }
    }
}

fn default_people_auto_learn() -> bool {
    true
}

/// Default retention window, in days (~3 months) — used both as
/// `ChatAudioSettings`' own default and as the per-field fallback for a
/// settings blob saved before `retention_days` was renamed from
/// `retention_months`, see `a_chat_audio_settings_blob_saved_as_months_still_loads`.
fn default_chat_audio_retention_days() -> i64 {
    90
}

/// Dictado por voz en el cuadro de envío del chat (botón de micrófono).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DictationSettings {
    /// Si el texto dictado se envía solo al detener la grabación, en vez de
    /// quedar en el cuadro para revisarlo.
    #[serde(default)]
    pub auto_send: bool,
}

/// Whether Chat keeps the raw audio bytes it plays back for a message on
/// disk, so the message's "escuchar" button can replay the exact original
/// clip later without a second TTS call — pedido explícito del usuario.
/// Alcance limitado a Chat: Twitch y Llamada no persisten mensajes en
/// absoluto hoy, así que no hay nada ahí para adjuntarles audio (ver
/// docs/DEVELOPMENT_STATUS.md). Only ever populated for `Http`/
/// `ElevenLabs` — native OS voices have no capturable buffer to save, see
/// `TTSProvider::speak_and_capture`. Off by default: this is opt-in
/// storage, not a silent default that eats disk space for users who never
/// asked for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatAudioSettings {
    pub save_enabled: bool,
    /// How long a saved clip is kept before the startup sweep
    /// (`ChatService::prune_old_audio`) deletes it, in days — the Voz tab
    /// offers 1/14/30/60/90/180 (1 día, 2 semanas, 1/2/3/6 meses) as
    /// options, but nothing here enforces that; it's a UI-level
    /// constraint, not worth a dedicated enum for the DB layer.
    /// `#[serde(default)]` because this field used to be `retentionMonths`
    /// (× 30 to get days) — an older stored blob just falls back to the
    /// default rather than failing to decode.
    #[serde(default = "default_chat_audio_retention_days")]
    pub retention_days: i64,
}

impl Default for ChatAudioSettings {
    fn default() -> Self {
        Self { save_enabled: false, retention_days: default_chat_audio_retention_days() }
    }
}

/// Cómo se comporta la app cuando no está en primer plano — Configuración
/// > General. `minimize_to_tray` está apagado por defecto a propósito:
/// cerrar con la X sigue cerrando la app como siempre a menos que el
/// usuario prenda esto (pedido explícito, para no cambiarle el
/// comportamiento a nadie sin avisar). `voice_while_hidden` también
/// apagado por defecto — la voz se silencia mientras la ventana está
/// oculta en la bandeja salvo que se lo habiliten (ver
/// `AivtTtsProvider::set_voice_while_hidden_enabled`).
/// `notify_proactive` sí arranca prendido: es la razón de ser de la
/// función (avisar cuando el personaje escribe por su cuenta).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundSettings {
    pub minimize_to_tray: bool,
    pub notify_proactive: bool,
    pub voice_while_hidden: bool,
}

impl Default for BackgroundSettings {
    fn default() -> Self {
        Self { minimize_to_tray: false, notify_proactive: true, voice_while_hidden: false }
    }
}

impl SettingsService {
    pub async fn avatar_server_settings(&self) -> Result<AvatarServerSettings, SettingsError> {
        Ok(self.get_json(KEY_AVATAR_SERVER_SETTINGS).await?.unwrap_or_default())
    }

    pub async fn set_avatar_server_settings(&self, settings: AvatarServerSettings) -> Result<AvatarServerSettings, SettingsError> {
        self.set_json(KEY_AVATAR_SERVER_SETTINGS, &settings).await?;
        Ok(settings)
    }

    pub async fn appearance_settings(&self) -> Result<AppearanceSettings, SettingsError> {
        Ok(self.get_json(KEY_APPEARANCE_SETTINGS).await?.unwrap_or_default())
    }

    pub async fn set_appearance_settings(&self, settings: AppearanceSettings) -> Result<AppearanceSettings, SettingsError> {
        self.set_json(KEY_APPEARANCE_SETTINGS, &settings).await?;
        Ok(settings)
    }

    pub async fn people_policy(&self) -> Result<PeoplePolicy, SettingsError> {
        Ok(self.get_json(KEY_PEOPLE_POLICY).await?.unwrap_or_default())
    }

    pub async fn set_people_policy(&self, policy: PeoplePolicy) -> Result<PeoplePolicy, SettingsError> {
        self.set_json(KEY_PEOPLE_POLICY, &policy).await?;
        Ok(policy)
    }

    /// A single global mute covering every voice-producing channel (Chat,
    /// Twitch, Discord, Llamada) at once — independent of each profile's
    /// own `voice.enabled` and each integration's own policy. Off by
    /// default (unmuted) so nothing already working changes behavior for
    /// an existing install; the user mutes explicitly via the Sidebar
    /// button next to "mostrar ventana flotante". The persisted value here
    /// is only the source of truth *across restarts* — the live check on
    /// every `speak()` call reads `AivtTtsProvider`'s in-memory
    /// `AtomicBool` (seeded from this at startup), not this DB row, to
    /// keep the hot path free of an async DB read.
    pub async fn voice_muted(&self) -> Result<bool, SettingsError> {
        Ok(self.get_json(KEY_VOICE_MUTED).await?.unwrap_or(false))
    }

    pub async fn set_voice_muted(&self, muted: bool) -> Result<bool, SettingsError> {
        self.set_json(KEY_VOICE_MUTED, &muted).await?;
        Ok(muted)
    }

    /// Dispositivo de salida de las voces (`None` = el predeterminado del
    /// sistema). El valor guardado es `Option<String>` (borrarlo escribe
    /// `null`), así que se decodifica así y `flatten` junta "nunca se guardó"
    /// con "se borró" — mismo caso que `user_avatar_path`.
    pub async fn audio_output_device(&self) -> Result<Option<String>, SettingsError> {
        Ok(self.get_json::<Option<String>>(KEY_AUDIO_OUTPUT_DEVICE).await?.flatten())
    }

    pub async fn set_audio_output_device(&self, name: Option<String>) -> Result<Option<String>, SettingsError> {
        self.set_json(KEY_AUDIO_OUTPUT_DEVICE, &name).await?;
        Ok(name)
    }

    /// Si nunca se guardó, `false` significa "instalación nueva, mostrar el
    /// wizard de bienvenida" — pero `src-tauri`'s `run()` hace un backfill
    /// al arrancar: si ya existen perfiles y este flag nunca se guardó, lo
    /// marca completado ahí mismo antes de que el frontend llegue a
    /// preguntar, para no forzar el wizard en instalaciones que ya venían
    /// usando la app antes de que este flag existiera.
    pub async fn onboarding_completed(&self) -> Result<bool, SettingsError> {
        Ok(self.get_json(KEY_ONBOARDING_COMPLETED).await?.unwrap_or(false))
    }

    pub async fn set_onboarding_completed(&self, completed: bool) -> Result<bool, SettingsError> {
        self.set_json(KEY_ONBOARDING_COMPLETED, &completed).await?;
        Ok(completed)
    }

    /// See `BackgroundSettings`'s doc comment. Same "persisted value is
    /// only the source of truth across restarts" relationship `voice_muted`
    /// has with `AivtTtsProvider`'s in-memory flags — `src-tauri`'s `run()`
    /// seeds `minimize_to_tray_enabled`/`voice_while_hidden` from this once
    /// at startup, and `commands::settings::settings_set_background`
    /// updates those live flags on every change so it takes effect without
    /// a restart.
    pub async fn background_settings(&self) -> Result<BackgroundSettings, SettingsError> {
        Ok(self.get_json(KEY_BACKGROUND_SETTINGS).await?.unwrap_or_default())
    }

    pub async fn set_background_settings(&self, settings: BackgroundSettings) -> Result<BackgroundSettings, SettingsError> {
        self.set_json(KEY_BACKGROUND_SETTINGS, &settings).await?;
        Ok(settings)
    }

    pub async fn dictation_settings(&self) -> Result<DictationSettings, SettingsError> {
        Ok(self.get_json(KEY_DICTATION_SETTINGS).await?.unwrap_or_default())
    }

    pub async fn set_dictation_settings(&self, settings: DictationSettings) -> Result<DictationSettings, SettingsError> {
        self.set_json(KEY_DICTATION_SETTINGS, &settings).await?;
        Ok(settings)
    }

    pub async fn chat_audio_settings(&self) -> Result<ChatAudioSettings, SettingsError> {
        Ok(self.get_json(KEY_CHAT_AUDIO_SETTINGS).await?.unwrap_or_default())
    }

    pub async fn set_chat_audio_settings(&self, settings: ChatAudioSettings) -> Result<ChatAudioSettings, SettingsError> {
        self.set_json(KEY_CHAT_AUDIO_SETTINGS, &settings).await?;
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use aivt_database::AppSettingsRepository;
    

    use super::*;
    use crate::service::test_service;

    #[tokio::test]
    async fn audio_output_device_defaults_to_the_system_one_and_can_be_cleared() {
        let service = test_service().await;
        assert_eq!(service.audio_output_device().await.unwrap(), None);
        service.set_audio_output_device(Some("Altavoces (Realtek)".to_string())).await.unwrap();
        assert_eq!(service.audio_output_device().await.unwrap().as_deref(), Some("Altavoces (Realtek)"));
        service.set_audio_output_device(None).await.unwrap();
        assert_eq!(service.audio_output_device().await.unwrap(), None);
    }

    #[tokio::test]
    async fn voice_muted_defaults_to_false() {
        let service = test_service().await;
        assert!(!service.voice_muted().await.unwrap());
    }

    #[tokio::test]
    async fn voice_muted_round_trips() {
        let service = test_service().await;
        assert_eq!(service.set_voice_muted(true).await.unwrap(), true);
        assert!(service.voice_muted().await.unwrap());

        service.set_voice_muted(false).await.unwrap();
        assert!(!service.voice_muted().await.unwrap());
    }

    #[tokio::test]
    async fn onboarding_completed_defaults_to_false() {
        let service = test_service().await;
        assert!(!service.onboarding_completed().await.unwrap());
    }

    #[tokio::test]
    async fn onboarding_completed_round_trips() {
        let service = test_service().await;
        assert_eq!(service.set_onboarding_completed(true).await.unwrap(), true);
        assert!(service.onboarding_completed().await.unwrap());

        service.set_onboarding_completed(false).await.unwrap();
        assert!(!service.onboarding_completed().await.unwrap());
    }

    #[tokio::test]
    async fn background_settings_default_to_tray_off_notify_on_voice_off() {
        let service = test_service().await;
        let settings = service.background_settings().await.unwrap();
        assert!(!settings.minimize_to_tray);
        assert!(settings.notify_proactive);
        assert!(!settings.voice_while_hidden);
    }

    #[tokio::test]
    async fn background_settings_round_trip() {
        let service = test_service().await;
        let saved = service
            .set_background_settings(BackgroundSettings { minimize_to_tray: true, notify_proactive: false, voice_while_hidden: true })
            .await
            .unwrap();
        assert!(saved.minimize_to_tray);

        let loaded = service.background_settings().await.unwrap();
        assert_eq!(loaded, saved);
    }

    #[tokio::test]
    async fn appearance_settings_default_to_dark_normal_inter() {
        let service = test_service().await;
        let settings = service.appearance_settings().await.unwrap();
        assert_eq!(settings.color_mode, ColorMode::Dark);
        assert_eq!(settings.density, MessageDensity::Normal);
        assert_eq!(settings.roundness, None);
        assert_eq!(settings.font_id, "inter");
        assert_eq!(settings.font_scale, 100);
    }

    #[tokio::test]
    async fn appearance_settings_round_trip() {
        let service = test_service().await;
        let saved = service
            .set_appearance_settings(AppearanceSettings {
                color_mode: ColorMode::Light,
                roundness: Some(150),
                density: MessageDensity::Compact,
                font_id: "space-grotesk".to_string(),
                font_scale: 120,
                seasonal: "halloween".to_string(),
                seasonal_motion: false,
                seasonal_disabled: vec!["muertos".to_string()],
            })
            .await
            .unwrap();
        assert_eq!(saved.color_mode, ColorMode::Light);

        let loaded = service.appearance_settings().await.unwrap();
        assert_eq!(loaded, saved);
    }

    #[test]
    fn appearance_settings_saved_before_decorations_existed_still_load() {
        let old = r#"{"colorMode":"dark","roundness":null,"density":"normal","fontId":"inter","fontScale":100}"#;
        let loaded: AppearanceSettings = serde_json::from_str(old).unwrap();
        assert_eq!(loaded.seasonal, "off");
        assert!(loaded.seasonal_motion);
        assert!(loaded.seasonal_disabled.is_empty());
    }

    #[tokio::test]
    async fn chat_audio_settings_default_to_disabled_with_ninety_days_retention() {
        let service = test_service().await;
        let settings = service.chat_audio_settings().await.unwrap();
        assert!(!settings.save_enabled);
        assert_eq!(settings.retention_days, 90);
    }

    #[tokio::test]
    async fn chat_audio_settings_round_trip() {
        let service = test_service().await;
        let saved = service
            .set_chat_audio_settings(ChatAudioSettings { save_enabled: true, retention_days: 180 })
            .await
            .unwrap();
        assert!(saved.save_enabled);
        assert_eq!(service.chat_audio_settings().await.unwrap().retention_days, 180);
    }

    /// A blob saved before `retention_days` was renamed from
    /// `retention_months` (only `saveEnabled` + the old key in the stored
    /// JSON) must still load — same reasoning as
    /// `an_stt_model_saved_before_gpu_enabled_existed_still_loads`.
    #[tokio::test]
    async fn a_chat_audio_settings_blob_saved_as_months_still_loads() {
        let service = test_service().await;
        AppSettingsRepository::new(&service.pool)
            .set(KEY_CHAT_AUDIO_SETTINGS, r#"{"saveEnabled":true,"retentionMonths":6}"#)
            .await
            .unwrap();

        let loaded = service.chat_audio_settings().await.unwrap();
        assert!(loaded.save_enabled);
        assert_eq!(loaded.retention_days, 90);
    }

    #[tokio::test]
    async fn people_policy_defaults_to_both_auto_learn_toggles_enabled() {
        let service = test_service().await;
        let policy = service.people_policy().await.unwrap();
        assert!(policy.twitch_auto_learn_enabled);
        assert!(policy.discord_auto_learn_enabled);
    }

    #[tokio::test]
    async fn people_policy_round_trips() {
        let service = test_service().await;
        service.set_people_policy(PeoplePolicy { twitch_auto_learn_enabled: false, discord_auto_learn_enabled: false }).await.unwrap();
        let policy = service.people_policy().await.unwrap();
        assert!(!policy.twitch_auto_learn_enabled);
        assert!(!policy.discord_auto_learn_enabled);
    }

    #[tokio::test]
    async fn people_policy_saved_before_discord_auto_learn_existed_still_loads_with_it_defaulted_on() {
        let service = test_service().await;
        AppSettingsRepository::new(&service.pool).set(KEY_PEOPLE_POLICY, r#"{"twitchAutoLearnEnabled":false}"#).await.unwrap();

        let policy = service.people_policy().await.unwrap();
        assert!(!policy.twitch_auto_learn_enabled);
        assert!(policy.discord_auto_learn_enabled, "a newer field must default to on for a policy blob saved before it existed");
    }
}
