use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};

use crate::error::TwitchError;
use crate::types::TwitchChatMessage;

const EVENTSUB_WS_URL: &str = "wss://eventsub.wss.twitch.tv/ws";

type WsStream = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

/// One parsed EventSub frame, trimmed to what `TwitchService`'s loop
/// actually branches on — the raw envelope has several other message
/// types (`session_disconnect`, `revocation`, ...) that this app currently
/// has no use for and just falls through as `Other`.
pub enum EventSubMessage {
    ChatMessage(TwitchChatMessage),
    /// Twitch is retiring this connection gracefully — reconnect to the
    /// given URL *before* this one closes, so no events are missed;
    /// existing subscriptions transfer automatically to the new session.
    Reconnect(String),
    Other,
}

pub struct EventSubConnection {
    socket: WsStream,
    /// The connected account's own Twitch user id — used to detect an
    /// `@`-mention of *that* account specifically (`mentions_bot`),
    /// distinct from `mentions_broadcaster` (a mention of whichever
    /// channel a message came from). Threaded in at connect time, same
    /// pattern as `aivt_service_discord::gateway::DiscordGateway::connect`.
    own_user_id: String,
}

impl EventSubConnection {
    /// Connects and waits for the mandatory `session_welcome` frame,
    /// returning both the live connection and the `session_id` the caller
    /// needs to create Helix subscriptions against.
    pub async fn connect(own_user_id: String) -> Result<(Self, String), TwitchError> {
        Self::connect_to(EVENTSUB_WS_URL, own_user_id).await
    }

    pub async fn connect_to(url: &str, own_user_id: String) -> Result<(Self, String), TwitchError> {
        let (socket, _response) = connect_async(url).await.map_err(|err| TwitchError::EventSubConnect(err.to_string()))?;
        let mut connection = Self { socket, own_user_id };

        loop {
            let Some(frame) = connection.socket.next().await else {
                return Err(TwitchError::EventSubClosed);
            };
            let text = match frame.map_err(|err| TwitchError::EventSubConnect(err.to_string()))? {
                WsMessage::Text(text) => text,
                WsMessage::Close(_) => return Err(TwitchError::EventSubClosed),
                _ => continue,
            };
            let envelope: Envelope = serde_json::from_str(&text)
                .map_err(|err| TwitchError::EventSubConnect(format!("mensaje de bienvenida ilegible: {err}")))?;
            if envelope.metadata.message_type == "session_welcome" {
                let session_id = envelope
                    .payload
                    .session
                    .and_then(|s| s.id)
                    .ok_or_else(|| TwitchError::EventSubConnect("session_welcome sin session id".to_string()))?;
                return Ok((connection, session_id));
            }
            // Anything else this early is unexpected but not fatal — keep
            // waiting for the welcome frame Twitch guarantees is first.
        }
    }

    /// Blocks until the next frame worth surfacing arrives. Keepalives are
    /// consumed silently (they only exist to prove the connection is
    /// still alive — there's nothing for the caller to act on).
    pub async fn next_message(&mut self) -> Result<EventSubMessage, TwitchError> {
        loop {
            let Some(frame) = self.socket.next().await else {
                return Err(TwitchError::EventSubClosed);
            };
            let text = match frame.map_err(|err| TwitchError::EventSubConnect(err.to_string()))? {
                WsMessage::Text(text) => text,
                WsMessage::Ping(payload) => {
                    let _ = self.socket.send(WsMessage::Pong(payload)).await;
                    continue;
                }
                WsMessage::Close(_) => return Err(TwitchError::EventSubClosed),
                _ => continue,
            };

            let envelope: Envelope = match serde_json::from_str(&text) {
                Ok(envelope) => envelope,
                // A frame we don't understand shouldn't kill the whole
                // connection — Twitch's payload shapes evolve; skip it.
                Err(_) => continue,
            };

            match envelope.metadata.message_type.as_str() {
                "session_keepalive" => continue,
                "session_reconnect" => {
                    if let Some(url) = envelope.payload.session.and_then(|s| s.reconnect_url) {
                        return Ok(EventSubMessage::Reconnect(url));
                    }
                    continue;
                }
                "notification" => {
                    if envelope.payload.subscription.as_ref().map(|s| s.subscription_type.as_str()) == Some("channel.chat.message") {
                        if let Some(event) = envelope.payload.event {
                            return Ok(EventSubMessage::ChatMessage(parse_chat_event(event, &self.own_user_id)));
                        }
                    }
                    return Ok(EventSubMessage::Other);
                }
                _ => return Ok(EventSubMessage::Other),
            }
        }
    }
}

fn parse_chat_event(event: ChatMessageEvent, own_user_id: &str) -> TwitchChatMessage {
    let is_mention_of = |user_id: &str| event.message.fragments.iter().any(|f| f.fragment_type == "mention" && f.mention.as_ref().is_some_and(|m| m.user_id == user_id));
    let mentions_broadcaster = is_mention_of(&event.broadcaster_user_id);
    let mentions_bot = is_mention_of(own_user_id);

    TwitchChatMessage {
        message_id: event.message_id,
        chatter_user_id: event.chatter_user_id,
        chatter_login: event.chatter_user_login,
        chatter_display_name: event.chatter_user_name,
        text: event.message.text,
        mentions_broadcaster,
        mentions_bot,
    }
}

#[derive(Deserialize)]
struct Envelope {
    metadata: Metadata,
    #[serde(default)]
    payload: Payload,
}

#[derive(Deserialize)]
struct Metadata {
    message_type: String,
}

#[derive(Deserialize, Default)]
struct Payload {
    session: Option<Session>,
    subscription: Option<Subscription>,
    event: Option<ChatMessageEvent>,
}

#[derive(Deserialize)]
struct Session {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    reconnect_url: Option<String>,
}

#[derive(Deserialize)]
struct Subscription {
    #[serde(rename = "type")]
    subscription_type: String,
}

#[derive(Deserialize)]
struct ChatMessageEvent {
    broadcaster_user_id: String,
    chatter_user_id: String,
    chatter_user_login: String,
    chatter_user_name: String,
    message_id: String,
    message: ChatMessageBody,
}

#[derive(Deserialize)]
struct ChatMessageBody {
    text: String,
    #[serde(default)]
    fragments: Vec<ChatMessageFragment>,
}

#[derive(Deserialize)]
struct ChatMessageFragment {
    #[serde(rename = "type")]
    fragment_type: String,
    #[serde(default)]
    mention: Option<ChatMessageMention>,
}

#[derive(Deserialize)]
struct ChatMessageMention {
    user_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_plain_chat_event_without_mentions() {
        let event: ChatMessageEvent = serde_json::from_str(
            r#"{
                "broadcaster_user_id": "1",
                "chatter_user_id": "2",
                "chatter_user_login": "vulito",
                "chatter_user_name": "Vulito",
                "message_id": "m1",
                "message": {"text": "hola!", "fragments": []}
            }"#,
        )
        .unwrap();
        let parsed = parse_chat_event(event, "1");
        assert_eq!(parsed.text, "hola!");
        assert!(!parsed.mentions_broadcaster);
        assert!(!parsed.mentions_bot);
    }

    #[test]
    fn detects_a_mention_of_the_broadcaster() {
        let event: ChatMessageEvent = serde_json::from_str(
            r#"{
                "broadcaster_user_id": "1",
                "chatter_user_id": "2",
                "chatter_user_login": "vulito",
                "chatter_user_name": "Vulito",
                "message_id": "m1",
                "message": {
                    "text": "@ciel hola!",
                    "fragments": [
                        {"type": "mention", "mention": {"user_id": "1"}},
                        {"type": "text"}
                    ]
                }
            }"#,
        )
        .unwrap();
        let parsed = parse_chat_event(event, "1");
        assert!(parsed.mentions_broadcaster);
        assert!(parsed.mentions_bot);
    }

    #[test]
    fn a_mention_of_someone_else_does_not_count() {
        let event: ChatMessageEvent = serde_json::from_str(
            r#"{
                "broadcaster_user_id": "1",
                "chatter_user_id": "2",
                "chatter_user_login": "vulito",
                "chatter_user_name": "Vulito",
                "message_id": "m1",
                "message": {
                    "text": "@otro_usuario hola",
                    "fragments": [{"type": "mention", "mention": {"user_id": "999"}}]
                }
            }"#,
        )
        .unwrap();
        let parsed = parse_chat_event(event, "1");
        assert!(!parsed.mentions_broadcaster);
        assert!(!parsed.mentions_bot);
    }

    /// The bot posts in a channel other than its own (a target channel is
    /// set) — the broadcaster (channel owner) and the bot account are now
    /// two different Twitch ids, so a mention of one must not count as a
    /// mention of the other.
    #[test]
    fn mentioning_the_broadcaster_of_a_target_channel_does_not_count_as_mentioning_the_bot() {
        let event: ChatMessageEvent = serde_json::from_str(
            r#"{
                "broadcaster_user_id": "1",
                "chatter_user_id": "2",
                "chatter_user_login": "vulito",
                "chatter_user_name": "Vulito",
                "message_id": "m1",
                "message": {
                    "text": "@streamer hola!",
                    "fragments": [{"type": "mention", "mention": {"user_id": "1"}}]
                }
            }"#,
        )
        .unwrap();
        let parsed = parse_chat_event(event, "2");
        assert!(parsed.mentions_broadcaster);
        assert!(!parsed.mentions_bot);
    }

    #[test]
    fn mentioning_the_bot_in_a_target_channel_does_not_count_as_mentioning_the_broadcaster() {
        let event: ChatMessageEvent = serde_json::from_str(
            r#"{
                "broadcaster_user_id": "1",
                "chatter_user_id": "3",
                "chatter_user_login": "vulito",
                "chatter_user_name": "Vulito",
                "message_id": "m1",
                "message": {
                    "text": "@mi_bot hola!",
                    "fragments": [{"type": "mention", "mention": {"user_id": "2"}}]
                }
            }"#,
        )
        .unwrap();
        let parsed = parse_chat_event(event, "2");
        assert!(!parsed.mentions_broadcaster);
        assert!(parsed.mentions_bot);
    }
}
