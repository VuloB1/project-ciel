use twilight_http::Client;
use twilight_model::id::marker::ChannelMarker;
use twilight_model::id::Id;

use crate::error::DiscordError;

/// Thin wrapper over `twilight_http::Client` — the two calls this service
/// actually needs (resolve the bot's own identity, post a reply), kept
/// separate from `gateway.rs`'s `Shard` the same way Twitch splits
/// `HelixClient` (REST) from `EventSubConnection` (WebSocket).
pub struct DiscordHttpClient {
    client: Client,
}

impl DiscordHttpClient {
    pub fn new(token: String) -> Self {
        Self { client: Client::new(token) }
    }

    /// `GET /users/@me` — used both to validate a freshly-pasted token
    /// (`DiscordService::connect_bot`, before ever opening the Gateway)
    /// and, on `start()`, to resolve the bot's own id for
    /// `DiscordGateway::connect`'s mention-matching. The third element is
    /// the bot's avatar CDN URL (empty if it has no custom avatar) — built
    /// from `avatar`'s image hash per Discord's own image-formatting rules
    /// (the `a_` prefix `ImageHash::Display` already adds for animated
    /// avatars doubles as the "use .gif" signal).
    pub async fn current_user(&self) -> Result<(String, u64, String), DiscordError> {
        let response = self.client.current_user().await.map_err(|err| DiscordError::InvalidToken(err.to_string()))?;
        let user = response.model().await.map_err(|err| DiscordError::Network(err.to_string()))?;
        let avatar_url = user.avatar.map(|hash| {
            let ext = if hash.is_animated() { "gif" } else { "png" };
            format!("https://cdn.discordapp.com/avatars/{}/{hash}.{ext}", user.id)
        });
        Ok((user.name, user.id.get(), avatar_url.unwrap_or_default()))
    }

    pub async fn send_message(&self, channel_id: &str, text: &str) -> Result<(), DiscordError> {
        let channel_id: Id<ChannelMarker> = channel_id.parse().map_err(|_| DiscordError::Network(format!("id de canal inválido: {channel_id}")))?;
        self.client.create_message(channel_id).content(text).await.map_err(|err| DiscordError::Network(err.to_string()))?;
        Ok(())
    }
}
