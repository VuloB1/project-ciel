//! Conexión, política y emotes de Twitch.

use serde::{Deserialize, Serialize};

use crate::service::SettingsService;
use crate::SettingsError;

const KEY_TWITCH_CONNECTION: &str = "twitch_connection";

const KEY_TWITCH_POLICY: &str = "twitch_policy";

const KEY_TWITCH_EMOTE_SETTINGS: &str = "twitch_emote_settings";

const SECRET_SERVICE_TWITCH: &str = "aivt.twitch";

const SECRET_KEY_ACCESS_TOKEN: &str = "access_token";

const SECRET_KEY_REFRESH_TOKEN: &str = "refresh_token";

/// Fase 8 — a single global Twitch connection: whichever account
/// completes Device Code Grant (`channel_login`/`broadcaster_id`) — that
/// account may be the streamer's own, or (Fase 8b) a dedicated bot
/// account, same idea as Discord's `StoredDiscordConnection`. Which
/// *character* answers in chat is whichever profile is active, same as
/// Chat. `has_tokens` is the only hint the frontend gets about whether
/// OAuth succeeded; the tokens themselves never leave `SecretStore`.
///
/// `target_channel_login`/`target_broadcaster_id` (Fase 8b) name a
/// *different* channel the connected account joins/posts in — empty by
/// default, meaning "the connected account's own channel" (today's exact
/// behavior, unchanged for anyone who doesn't set this). See
/// `aivt_service_twitch::service::effective_broadcaster_id`.
///
/// No client secret anywhere here on purpose — this app registers as a
/// "Public" Twitch client (the type Twitch's own console recommends for
/// desktop apps, which don't get a secret at all) and uses the Device
/// Code Grant flow, which never needs one. See
/// `aivt_service_twitch::DeviceCodeFlow`'s doc comment for how the first
/// version of this got that wrong.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredTwitchConnection {
    channel_login: String,
    broadcaster_id: String,
    has_tokens: bool,
    #[serde(default)]
    target_channel_login: String,
    #[serde(default)]
    target_broadcaster_id: String,
    /// URL del avatar de Twitch de la cuenta conectada (Helix `GET /users`'s
    /// `profile_image_url`) — puramente cosmético, para mostrarlo junto a
    /// "Conectado como" en el frontend. Vacío en conexiones guardadas antes
    /// de que este campo existiera.
    #[serde(default)]
    profile_image_url: String,
}

impl Default for StoredTwitchConnection {
    fn default() -> Self {
        Self {
            channel_login: String::new(),
            broadcaster_id: String::new(),
            has_tokens: false,
            target_channel_login: String::new(),
            target_broadcaster_id: String::new(),
            profile_image_url: String::new(),
        }
    }
}

impl StoredTwitchConnection {
    fn to_settings(&self) -> TwitchConnectionSettings {
        TwitchConnectionSettings {
            channel_login: self.channel_login.clone(),
            broadcaster_id: self.broadcaster_id.clone(),
            has_tokens: self.has_tokens,
            target_channel_login: self.target_channel_login.clone(),
            target_broadcaster_id: self.target_broadcaster_id.clone(),
            profile_image_url: self.profile_image_url.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchConnectionSettings {
    pub channel_login: String,
    pub broadcaster_id: String,
    pub has_tokens: bool,
    pub target_channel_login: String,
    pub target_broadcaster_id: String,
    pub profile_image_url: String,
}

/// When the character replies in Twitch chat (sección 8 del brief:
/// "nunca se asume, el usuario decide"). `cooldown_seconds` applies
/// regardless of the other two flags — a hard floor between any two
/// replies so a lively chat can't turn the character into a spam bot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchPolicy {
    pub enabled: bool,
    pub respond_to_all: bool,
    pub mentioned_only: bool,
    pub cooldown_seconds: i64,
}

impl Default for TwitchPolicy {
    fn default() -> Self {
        Self { enabled: false, respond_to_all: false, mentioned_only: true, cooldown_seconds: 30 }
    }
}

/// Opt-in toggle for the 7TV/BTTV/FFZ emote feature — the fetched/cached
/// list itself lives in `aivt_database::TwitchEmotesRepository`, not here;
/// this is just whether `TwitchService::build_extra_instructions` should
/// bother reading it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchEmoteSettings {
    pub enabled: bool,
}

impl Default for TwitchEmoteSettings {
    fn default() -> Self {
        Self { enabled: false }
    }
}

impl SettingsService {
    pub async fn twitch_connection(&self) -> Result<TwitchConnectionSettings, SettingsError> {
        let stored: StoredTwitchConnection = self.get_json(KEY_TWITCH_CONNECTION).await?.unwrap_or_default();
        Ok(stored.to_settings())
    }

    /// Called once OAuth + the Helix `GET /users` lookup both succeed —
    /// records which channel the app is now allowed to read/post to.
    pub async fn mark_twitch_connected(&self, channel_login: &str, broadcaster_id: &str, profile_image_url: &str) -> Result<TwitchConnectionSettings, SettingsError> {
        let mut stored: StoredTwitchConnection = self.get_json(KEY_TWITCH_CONNECTION).await?.unwrap_or_default();
        stored.channel_login = channel_login.to_string();
        stored.broadcaster_id = broadcaster_id.to_string();
        stored.has_tokens = true;
        stored.profile_image_url = profile_image_url.to_string();
        self.set_json(KEY_TWITCH_CONNECTION, &stored).await?;
        Ok(stored.to_settings())
    }

    /// Forgets the account (tokens + resolved channel).
    pub async fn disconnect_twitch(&self) -> Result<TwitchConnectionSettings, SettingsError> {
        let mut stored: StoredTwitchConnection = self.get_json(KEY_TWITCH_CONNECTION).await?.unwrap_or_default();
        stored.channel_login = String::new();
        stored.broadcaster_id = String::new();
        stored.has_tokens = false;
        stored.profile_image_url = String::new();
        self.set_json(KEY_TWITCH_CONNECTION, &stored).await?;
        self.secrets.delete(SECRET_SERVICE_TWITCH, SECRET_KEY_ACCESS_TOKEN).await?;
        self.secrets.delete(SECRET_SERVICE_TWITCH, SECRET_KEY_REFRESH_TOKEN).await?;
        Ok(stored.to_settings())
    }

    /// Persists which channel the connected account joins/posts in — pure
    /// storage, no Helix lookup (that happens one layer up, in
    /// `TwitchService::set_target_channel`, before this is called). Empty
    /// strings mean "not set": every reader falls back to the connected
    /// account's own `channel_login`/`broadcaster_id`, reproducing the
    /// pre-Fase-8b behavior exactly. Deliberately *not* cleared by
    /// `disconnect_twitch` — it's a statement of intent, not a credential
    /// that needs re-verifying.
    pub async fn set_twitch_target_channel(&self, channel_login: &str, broadcaster_id: &str) -> Result<TwitchConnectionSettings, SettingsError> {
        let mut stored: StoredTwitchConnection = self.get_json(KEY_TWITCH_CONNECTION).await?.unwrap_or_default();
        stored.target_channel_login = channel_login.to_string();
        stored.target_broadcaster_id = broadcaster_id.to_string();
        self.set_json(KEY_TWITCH_CONNECTION, &stored).await?;
        Ok(stored.to_settings())
    }

    pub async fn set_twitch_tokens(&self, access_token: &str, refresh_token: &str) -> Result<(), SettingsError> {
        self.secrets.set(SECRET_SERVICE_TWITCH, SECRET_KEY_ACCESS_TOKEN, access_token).await?;
        self.secrets.set(SECRET_SERVICE_TWITCH, SECRET_KEY_REFRESH_TOKEN, refresh_token).await?;
        Ok(())
    }

    pub async fn twitch_tokens(&self) -> Result<Option<(String, String)>, SettingsError> {
        let access = self.secrets.get(SECRET_SERVICE_TWITCH, SECRET_KEY_ACCESS_TOKEN).await?;
        let refresh = self.secrets.get(SECRET_SERVICE_TWITCH, SECRET_KEY_REFRESH_TOKEN).await?;
        Ok(match (access, refresh) {
            (Some(a), Some(r)) => Some((a, r)),
            _ => None,
        })
    }

    pub async fn twitch_policy(&self) -> Result<TwitchPolicy, SettingsError> {
        Ok(self.get_json(KEY_TWITCH_POLICY).await?.unwrap_or_default())
    }

    pub async fn set_twitch_policy(&self, policy: TwitchPolicy) -> Result<TwitchPolicy, SettingsError> {
        self.set_json(KEY_TWITCH_POLICY, &policy).await?;
        Ok(policy)
    }

    pub async fn twitch_emote_settings(&self) -> Result<TwitchEmoteSettings, SettingsError> {
        Ok(self.get_json(KEY_TWITCH_EMOTE_SETTINGS).await?.unwrap_or_default())
    }

    pub async fn set_twitch_emote_settings(&self, settings: TwitchEmoteSettings) -> Result<TwitchEmoteSettings, SettingsError> {
        self.set_json(KEY_TWITCH_EMOTE_SETTINGS, &settings).await?;
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use aivt_database::AppSettingsRepository;
    

    use super::*;
    use crate::service::test_service;

    #[tokio::test]
    async fn twitch_connection_defaults_to_empty_and_disconnected() {
        let service = test_service().await;
        let connection = service.twitch_connection().await.unwrap();
        assert!(!connection.has_tokens);
    }

    #[tokio::test]
    async fn mark_twitch_connected_records_channel() {
        let service = test_service().await;

        let saved = service.mark_twitch_connected("mi_canal", "999", "https://static-cdn.jtvnw.net/user-default-pictures/mi_canal.png").await.unwrap();
        assert_eq!(saved.channel_login, "mi_canal");
        assert_eq!(saved.broadcaster_id, "999");
        assert!(saved.has_tokens);
        assert_eq!(saved.profile_image_url, "https://static-cdn.jtvnw.net/user-default-pictures/mi_canal.png");
    }

    #[tokio::test]
    async fn twitch_tokens_round_trip_through_the_secret_store() {
        let service = test_service().await;
        assert!(service.twitch_tokens().await.unwrap().is_none());

        service.set_twitch_tokens("access-1", "refresh-1").await.unwrap();
        let (access, refresh) = service.twitch_tokens().await.unwrap().unwrap();
        assert_eq!(access, "access-1");
        assert_eq!(refresh, "refresh-1");
    }

    #[tokio::test]
    async fn disconnect_twitch_clears_tokens_and_channel() {
        let service = test_service().await;
        service.mark_twitch_connected("mi_canal", "999", "https://example.com/avatar.png").await.unwrap();
        service.set_twitch_tokens("access-1", "refresh-1").await.unwrap();

        let saved = service.disconnect_twitch().await.unwrap();
        assert_eq!(saved.channel_login, "");
        assert!(!saved.has_tokens);
        assert_eq!(saved.profile_image_url, "");
        assert!(service.twitch_tokens().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn twitch_connection_defaults_to_no_target_channel() {
        let service = test_service().await;
        let connection = service.twitch_connection().await.unwrap();
        assert_eq!(connection.target_channel_login, "");
        assert_eq!(connection.target_broadcaster_id, "");
    }

    #[tokio::test]
    async fn set_twitch_target_channel_round_trips() {
        let service = test_service().await;
        let saved = service.set_twitch_target_channel("otro_canal", "555").await.unwrap();
        assert_eq!(saved.target_channel_login, "otro_canal");
        assert_eq!(saved.target_broadcaster_id, "555");
        assert_eq!(service.twitch_connection().await.unwrap().target_channel_login, "otro_canal");
    }

    #[tokio::test]
    async fn set_twitch_target_channel_with_empty_strings_clears_it() {
        let service = test_service().await;
        service.set_twitch_target_channel("otro_canal", "555").await.unwrap();

        let saved = service.set_twitch_target_channel("", "").await.unwrap();
        assert_eq!(saved.target_channel_login, "");
        assert_eq!(saved.target_broadcaster_id, "");
    }

    #[tokio::test]
    async fn disconnect_twitch_keeps_the_target_channel() {
        let service = test_service().await;
        service.mark_twitch_connected("mi_bot", "999", "").await.unwrap();
        service.set_twitch_target_channel("otro_canal", "555").await.unwrap();

        let saved = service.disconnect_twitch().await.unwrap();
        assert_eq!(saved.target_channel_login, "otro_canal");
        assert_eq!(saved.target_broadcaster_id, "555");
    }

    /// A connection saved before `targetChannelLogin`/`targetBroadcasterId`
    /// existed (only the original four keys in the stored JSON) must still
    /// load — same reasoning as `an_stt_model_saved_before_gpu_enabled_existed_still_loads`.
    #[tokio::test]
    async fn a_twitch_connection_saved_before_the_target_channel_existed_still_loads() {
        let service = test_service().await;
        AppSettingsRepository::new(&service.pool)
            .set(KEY_TWITCH_CONNECTION, r#"{"clientId":"abc123","channelLogin":"mi_canal","broadcasterId":"999","hasTokens":true}"#)
            .await
            .unwrap();

        let loaded = service.twitch_connection().await.unwrap();
        assert_eq!(loaded.channel_login, "mi_canal");
        assert_eq!(loaded.target_channel_login, "");
        assert_eq!(loaded.target_broadcaster_id, "");
    }

    #[tokio::test]
    async fn twitch_policy_defaults_to_mentioned_only_with_a_cooldown() {
        let service = test_service().await;
        let policy = service.twitch_policy().await.unwrap();
        assert!(!policy.enabled);
        assert!(policy.mentioned_only);
        assert!(!policy.respond_to_all);
        assert_eq!(policy.cooldown_seconds, 30);
    }

    #[tokio::test]
    async fn twitch_policy_round_trips() {
        let service = test_service().await;
        let saved = service
            .set_twitch_policy(TwitchPolicy { enabled: true, respond_to_all: true, mentioned_only: false, cooldown_seconds: 10 })
            .await
            .unwrap();
        assert!(saved.respond_to_all);
        assert_eq!(service.twitch_policy().await.unwrap().cooldown_seconds, 10);
    }
}
