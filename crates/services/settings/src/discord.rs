//! Conexión y política del bot de Discord.

use serde::{Deserialize, Serialize};

use crate::service::SettingsService;
use crate::SettingsError;

const KEY_DISCORD_CONNECTION: &str = "discord_connection";

const KEY_DISCORD_POLICY: &str = "discord_policy";

const SECRET_SERVICE_DISCORD: &str = "aivt.discord";

const SECRET_KEY_BOT_TOKEN: &str = "bot_token";

/// Fase 9 — a single global Discord connection, same "one account for the
/// whole app" shape as `TwitchConnectionSettings`. Unlike Twitch, this is a
/// dedicated bot account (not the user's own) so the character can answer
/// anyone in any server the bot is invited to — see
/// `aivt_service_discord::DiscordService`'s doc comment. `has_token` is the
/// only hint the frontend gets about whether a token is saved; the token
/// itself never leaves `SecretStore`. No OAuth here at all: a bot token is
/// generated once in the Discord Developer Portal and pasted in, so there's
/// no `client_id`/Device Code Grant equivalent to store.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredDiscordConnection {
    bot_username: String,
    bot_user_id: String,
    has_token: bool,
    /// URL del avatar del bot en el CDN de Discord (`GET /users/@me`'s
    /// `avatar` hash) — puramente cosmético, para mostrarlo junto a
    /// "Conectado como" en el frontend. Vacío si el bot no tiene avatar
    /// propio o en conexiones guardadas antes de que este campo existiera.
    #[serde(default)]
    avatar_url: String,
}

impl Default for StoredDiscordConnection {
    fn default() -> Self {
        Self { bot_username: String::new(), bot_user_id: String::new(), has_token: false, avatar_url: String::new() }
    }
}

impl StoredDiscordConnection {
    fn to_settings(&self) -> DiscordConnectionSettings {
        DiscordConnectionSettings {
            bot_username: self.bot_username.clone(),
            bot_user_id: self.bot_user_id.clone(),
            has_token: self.has_token,
            avatar_url: self.avatar_url.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordConnectionSettings {
    pub bot_username: String,
    pub bot_user_id: String,
    pub has_token: bool,
    pub avatar_url: String,
}

/// When the character replies in Discord — same shape as `TwitchPolicy`.
/// `mentioned_only` here checks a *real* @-mention of the bot (Discord's
/// own structured mentions), not a plain-text name match, so it's a
/// stronger signal than Twitch's equivalent — see
/// `aivt_service_discord::policy::should_respond`. Whether a reply is also
/// spoken isn't decided here — every voice channel (Chat/Twitch/Discord/
/// Llamada) shares the same global mute, see `SettingsService::voice_muted`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordPolicy {
    pub enabled: bool,
    pub respond_to_all: bool,
    pub mentioned_only: bool,
    pub cooldown_seconds: i64,
}

impl Default for DiscordPolicy {
    fn default() -> Self {
        Self { enabled: false, respond_to_all: false, mentioned_only: true, cooldown_seconds: 30 }
    }
}

impl SettingsService {
    pub async fn discord_connection(&self) -> Result<DiscordConnectionSettings, SettingsError> {
        let stored: StoredDiscordConnection = self.get_json(KEY_DISCORD_CONNECTION).await?.unwrap_or_default();
        Ok(stored.to_settings())
    }

    /// Saves the bot token (via `SecretStore`) and, once
    /// `DiscordService::connect_bot` has confirmed it's valid by resolving
    /// the bot's own identity through `GET /users/@me`, records
    /// `bot_username`/`bot_user_id` — mirrors `mark_twitch_connected`, but
    /// in one step since there's no separate OAuth round trip here.
    pub async fn set_discord_token(&self, token: &str, bot_username: &str, bot_user_id: &str, avatar_url: &str) -> Result<DiscordConnectionSettings, SettingsError> {
        self.secrets.set(SECRET_SERVICE_DISCORD, SECRET_KEY_BOT_TOKEN, token).await?;
        let stored = StoredDiscordConnection {
            bot_username: bot_username.to_string(),
            bot_user_id: bot_user_id.to_string(),
            has_token: true,
            avatar_url: avatar_url.to_string(),
        };
        self.set_json(KEY_DISCORD_CONNECTION, &stored).await?;
        Ok(stored.to_settings())
    }

    pub async fn discord_token(&self) -> Result<Option<String>, SettingsError> {
        Ok(self.secrets.get(SECRET_SERVICE_DISCORD, SECRET_KEY_BOT_TOKEN).await?)
    }

    /// Forgets the bot account entirely (token + resolved identity) — unlike
    /// Twitch's `disconnect_twitch`, there's no separately-saved `client_id`
    /// to keep around, since a Discord bot token *is* the credential.
    pub async fn disconnect_discord(&self) -> Result<DiscordConnectionSettings, SettingsError> {
        let stored = StoredDiscordConnection::default();
        self.set_json(KEY_DISCORD_CONNECTION, &stored).await?;
        self.secrets.delete(SECRET_SERVICE_DISCORD, SECRET_KEY_BOT_TOKEN).await?;
        Ok(stored.to_settings())
    }

    pub async fn discord_policy(&self) -> Result<DiscordPolicy, SettingsError> {
        Ok(self.get_json(KEY_DISCORD_POLICY).await?.unwrap_or_default())
    }

    pub async fn set_discord_policy(&self, policy: DiscordPolicy) -> Result<DiscordPolicy, SettingsError> {
        self.set_json(KEY_DISCORD_POLICY, &policy).await?;
        Ok(policy)
    }
}

#[cfg(test)]
mod tests {
    use aivt_database::AppSettingsRepository;
    

    use super::*;
    use crate::service::test_service;

    #[tokio::test]
    async fn discord_connection_defaults_to_empty_and_disconnected() {
        let service = test_service().await;
        let connection = service.discord_connection().await.unwrap();
        assert_eq!(connection.bot_username, "");
        assert!(!connection.has_token);
    }

    #[tokio::test]
    async fn set_discord_token_records_the_bot_identity_and_stores_the_token_in_the_secret_store() {
        let service = test_service().await;

        let saved = service.set_discord_token("bot-token-1", "MiPersonajeBot", "555", "https://cdn.discordapp.com/avatars/555/abc.png").await.unwrap();
        assert_eq!(saved.bot_username, "MiPersonajeBot");
        assert_eq!(saved.bot_user_id, "555");
        assert!(saved.has_token);
        assert_eq!(saved.avatar_url, "https://cdn.discordapp.com/avatars/555/abc.png");
        assert_eq!(service.discord_token().await.unwrap().as_deref(), Some("bot-token-1"));

        let raw = AppSettingsRepository::new(&service.pool).get(KEY_DISCORD_CONNECTION).await.unwrap().unwrap();
        assert!(!raw.contains("bot-token-1"));
    }

    #[tokio::test]
    async fn disconnect_discord_clears_the_token_and_identity() {
        let service = test_service().await;
        service.set_discord_token("bot-token-1", "MiPersonajeBot", "555", "https://cdn.discordapp.com/avatars/555/abc.png").await.unwrap();

        let saved = service.disconnect_discord().await.unwrap();
        assert_eq!(saved.bot_username, "");
        assert!(!saved.has_token);
        assert_eq!(saved.avatar_url, "");
        assert!(service.discord_token().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn discord_policy_defaults_to_mentioned_only_with_a_cooldown() {
        let service = test_service().await;
        let policy = service.discord_policy().await.unwrap();
        assert!(!policy.enabled);
        assert!(policy.mentioned_only);
        assert!(!policy.respond_to_all);
        assert_eq!(policy.cooldown_seconds, 30);
    }

    #[tokio::test]
    async fn discord_policy_round_trips() {
        let service = test_service().await;
        let saved = service
            .set_discord_policy(DiscordPolicy { enabled: true, respond_to_all: true, mentioned_only: false, cooldown_seconds: 10 })
            .await
            .unwrap();
        assert!(saved.respond_to_all);
        assert_eq!(service.discord_policy().await.unwrap().cooldown_seconds, 10);
    }
}
