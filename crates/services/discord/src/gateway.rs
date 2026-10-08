use twilight_gateway::{Event, EventTypeFlags, Intents, Shard, ShardId, StreamExt as _};

use crate::error::DiscordError;
use crate::types::DiscordChatMessage;

/// A `Shard` is a single Gateway WebSocket connection — plenty for a
/// companion-app bot (Discord only requires sharding once a bot joins
/// thousands of guilds). `Shard::new` never touches the network itself;
/// the actual handshake happens lazily on the first `next_message` call,
/// which is why a bad token surfaces there and not here — see
/// `DiscordService::connect_bot`'s separate `GET /users/@me` check for
/// validating the token up front instead.
pub struct DiscordGateway {
    shard: Shard,
    own_user_id: u64,
}

impl DiscordGateway {
    pub fn connect(token: String, own_user_id: u64) -> Self {
        let intents = Intents::GUILDS | Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT | Intents::DIRECT_MESSAGES;
        let shard = Shard::new(ShardId::ONE, token, intents);
        Self { shard, own_user_id }
    }

    /// Blocks until the next chat message arrives — every other Gateway
    /// event (presence updates, typing indicators, voice state, etc.) is
    /// silently skipped. `twilight_gateway::Shard` handles heartbeating,
    /// resume, and reconnection internally, so unlike Twitch's hand-rolled
    /// `EventSubConnection` this never needs a manual reconnect branch —
    /// but that internal healing only happens across *repeated* polls: a
    /// live test found a slow tool-calling response (Fase 10b) blocking
    /// this loop for 20-30s was enough for Discord to consider the
    /// connection a zombie and force-close it, and treating that
    /// `GatewayClose` as fatal (as an earlier version of this method did)
    /// killed the whole listening loop for good, requiring the user to
    /// manually reconnect — instead of just quietly resuming, which is
    /// what `Shard` does on its own if you keep polling it. Same reasoning
    /// for `Err` on an individual received message (decompression/
    /// deserializing hiccups, a failed reconnect attempt) — transient,
    /// logged and retried rather than killing the loop; the shard's own
    /// `Queue` already paces reconnect attempts, so this doesn't busy-loop.
    /// `None` from `next_event` is the one case that stays fatal: per
    /// twilight's source (`Shard::poll_next`), that only happens once the
    /// shard's internal state reaches `FatallyClosed` (bad token, invalid
    /// intents, or another unrecoverable close code) — retrying can't help.
    pub async fn next_message(&mut self) -> Result<DiscordChatMessage, DiscordError> {
        loop {
            let Some(item) = self.shard.next_event(EventTypeFlags::MESSAGE_CREATE).await else {
                return Err(DiscordError::GatewayClosed);
            };
            let event = match item {
                Ok(event) => event,
                Err(err) => {
                    tracing::warn!(target: "system", "discord: error del Gateway, reintentando: {err}");
                    continue;
                }
            };

            match event {
                Event::MessageCreate(message) => {
                    let mentions_bot = message.mentions.iter().any(|mention| mention.id.get() == self.own_user_id);
                    let author_display_name = message.author.global_name.clone().unwrap_or_else(|| message.author.name.clone());
                    return Ok(DiscordChatMessage {
                        channel_id: message.channel_id.to_string(),
                        author_id: message.author.id.to_string(),
                        author_display_name,
                        author_is_bot: message.author.bot,
                        text: message.content.clone(),
                        mentions_bot,
                    });
                }
                Event::GatewayClose(_) => {
                    tracing::info!(target: "system", "discord: el Gateway cerró la conexión, reconectando solo (comportamiento normal de twilight)");
                    continue;
                }
                _ => continue,
            }
        }
    }
}
