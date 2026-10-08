use serde::{Deserialize, Serialize};

use crate::error::TwitchError;

const API_BASE: &str = "https://api.twitch.tv/helix";

/// Thin Helix REST wrapper — every call needs both the app's `Client-Id`
/// and the connected user's OAuth token, so this bundles them instead of
/// threading two extra parameters through every call site.
pub struct HelixClient<'a> {
    http: &'a reqwest::Client,
    client_id: &'a str,
    access_token: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TwitchUser {
    pub id: String,
    pub login: String,
    #[allow(dead_code)]
    pub display_name: String,
    #[serde(default)]
    pub profile_image_url: String,
}

#[derive(Deserialize)]
struct UsersResponse {
    data: Vec<TwitchUser>,
}

#[derive(Serialize)]
struct CreateSubscriptionBody<'a> {
    #[serde(rename = "type")]
    subscription_type: &'a str,
    version: &'a str,
    condition: serde_json::Value,
    transport: TransportBody<'a>,
}

#[derive(Serialize)]
struct TransportBody<'a> {
    method: &'a str,
    session_id: &'a str,
}

#[derive(Serialize)]
struct SendChatMessageBody<'a> {
    broadcaster_id: &'a str,
    sender_id: &'a str,
    message: &'a str,
}

impl<'a> HelixClient<'a> {
    pub fn new(http: &'a reqwest::Client, client_id: &'a str, access_token: &'a str) -> Self {
        Self { http, client_id, access_token }
    }

    fn authorized(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder.header("Client-Id", self.client_id).bearer_auth(self.access_token)
    }

    /// No query params → Twitch resolves "the user this token belongs to".
    pub async fn get_authorized_user(&self) -> Result<TwitchUser, TwitchError> {
        let response = self.authorized(self.http.get(format!("{API_BASE}/users"))).send().await?;
        let body: UsersResponse = parse_json(response).await?;
        body.data.into_iter().next().ok_or_else(|| TwitchError::Api { status: 200, body: "sin usuario en la respuesta".to_string() })
    }

    /// Resolves a typed channel name to its Twitch user id — used by
    /// `TwitchService::set_target_channel` to let the connected account
    /// (typically a dedicated bot) join/post in a channel other than its
    /// own. Any valid token can look up any login; it doesn't need to
    /// belong to that user. Twitch answers with HTTP 200 and an empty
    /// `data` array for an unknown login (not a 404), so the "not found"
    /// case is a normal, successful response with nothing in it.
    pub async fn get_user_by_login(&self, login: &str) -> Result<TwitchUser, TwitchError> {
        let response = self.authorized(self.http.get(format!("{API_BASE}/users")).query(&[("login", login)])).send().await?;
        let body: UsersResponse = parse_json(response).await?;
        body.data.into_iter().next().ok_or_else(|| TwitchError::ChannelNotFound(login.to_string()))
    }

    /// Subscribes an already-open EventSub WebSocket session (`session_id`,
    /// from that connection's `session_welcome` message) to one event type.
    pub async fn create_eventsub_subscription(
        &self,
        subscription_type: &str,
        version: &str,
        condition: serde_json::Value,
        session_id: &str,
    ) -> Result<(), TwitchError> {
        let body = CreateSubscriptionBody {
            subscription_type,
            version,
            condition,
            transport: TransportBody { method: "websocket", session_id },
        };
        let response = self.authorized(self.http.post(format!("{API_BASE}/eventsub/subscriptions"))).json(&body).send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(TwitchError::Api { status: status.as_u16(), body });
        }
        Ok(())
    }

    pub async fn send_chat_message(&self, broadcaster_id: &str, sender_id: &str, message: &str) -> Result<(), TwitchError> {
        let body = SendChatMessageBody { broadcaster_id, sender_id, message };
        let response = self.authorized(self.http.post(format!("{API_BASE}/chat/messages"))).json(&body).send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(TwitchError::Api { status: status.as_u16(), body });
        }
        Ok(())
    }
}

async fn parse_json<T: for<'de> Deserialize<'de>>(response: reqwest::Response) -> Result<T, TwitchError> {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(TwitchError::Api { status: status.as_u16(), body });
    }
    serde_json::from_str(&body).map_err(|err| TwitchError::Api { status: status.as_u16(), body: err.to_string() })
}
