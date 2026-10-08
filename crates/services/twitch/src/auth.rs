use std::time::Duration;

use serde::Deserialize;
use tokio::time::sleep;

use crate::error::TwitchError;
use crate::types::TokenPair;

const DEVICE_ENDPOINT: &str = "https://id.twitch.tv/oauth2/device";
const TOKEN_ENDPOINT: &str = "https://id.twitch.tv/oauth2/token";

/// `user:read:chat` — read chat via EventSub as this user; reading a
/// channel's chat needs no permission from that channel's broadcaster,
/// only this scope on the reader's own token.
/// `user:write:chat` — post the character's replies. Posting *in the
/// connected account's own channel* (`broadcaster_id == sender_id`) needs
/// nothing else; posting in a *different* channel (Fase 8b's dedicated
/// bot account — see `SettingsService::set_twitch_target_channel`) also
/// needs the connected account to be a moderator there (`/mod <bot>`,
/// done once by that channel's streamer — no extra OAuth grant required).
/// `user:bot`/`channel:bot` are kept for completeness but are inert on a
/// user access token like this one — Twitch documents them for the *app*
/// access token path, which this client never uses.
const SCOPES: &str = "user:read:chat user:write:chat user:bot channel:bot";

/// How long this app itself is willing to poll before giving up, even if
/// Twitch's own `expires_in` (typically 1800s) allows longer — an
/// abandoned browser tab shouldn't keep a Tauri command hanging for half
/// an hour.
const MAX_POLL_DURATION: Duration = Duration::from_secs(300);

/// Twitch's [Device Code Grant Flow](https://dev.twitch.tv/docs/authentication/getting-tokens-oauth/#device-code-grant-flow)
/// — the flow Twitch's own developer console steers "Public" client apps
/// (desktop/mobile/game-engine, anything that can't keep a secret) toward.
/// Confirmed the hard way: this app's Twitch application was registered as
/// "Público", which doesn't even have a client secret to give out, and the
/// Authorization Code flow this originally used unconditionally requires
/// one — "Invalid client credentials" for a public client isn't about a
/// *missing* secret, it's that the flow itself is wrong for that client
/// type. Device Code needs no secret, no redirect URI, and no local HTTP
/// listener at all — just request a code, show the user where to enter
/// it, and poll.
pub struct DeviceCodeFlow {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

pub async fn request_device_code(client: &reqwest::Client, client_id: &str) -> Result<DeviceCodeFlow, TwitchError> {
    let response = client.post(DEVICE_ENDPOINT).form(&[("client_id", client_id), ("scopes", SCOPES)]).send().await?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(TwitchError::Api { status: status.as_u16(), body });
    }
    let parsed: DeviceCodeResponse =
        serde_json::from_str(&body).map_err(|err| TwitchError::Api { status: status.as_u16(), body: err.to_string() })?;
    Ok(DeviceCodeFlow {
        device_code: parsed.device_code,
        user_code: parsed.user_code,
        verification_uri: parsed.verification_uri,
        expires_in: parsed.expires_in,
        interval: parsed.interval,
    })
}

/// Polls the token endpoint until the user finishes authorizing on
/// `verification_uri` (or the flow is denied/expires/this app's own
/// `MAX_POLL_DURATION` runs out first).
pub async fn poll_for_token(client: &reqwest::Client, client_id: &str, flow: &DeviceCodeFlow) -> Result<TokenPair, TwitchError> {
    let deadline = tokio::time::Instant::now() + MAX_POLL_DURATION.min(Duration::from_secs(flow.expires_in));
    let interval = Duration::from_secs(flow.interval.max(1));

    loop {
        if tokio::time::Instant::now() >= deadline {
            return Err(TwitchError::AuthorizationTimedOut);
        }

        let response = client
            .post(TOKEN_ENDPOINT)
            .form(&[
                ("client_id", client_id),
                ("scopes", SCOPES),
                ("device_code", flow.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if status.is_success() {
            let parsed: token_response::TokenResponse =
                serde_json::from_str(&body).map_err(|err| TwitchError::Api { status: status.as_u16(), body: err.to_string() })?;
            return Ok(TokenPair { access_token: parsed.access_token, refresh_token: parsed.refresh_token });
        }

        if body.contains("authorization_pending") {
            sleep(interval).await;
            continue;
        }
        if body.contains("access_denied") {
            return Err(TwitchError::AuthorizationDenied(body));
        }
        return Err(TwitchError::Api { status: status.as_u16(), body });
    }
}

/// Public clients (no client secret — see `DeviceCodeFlow`'s doc comment)
/// can refresh without one; this app never registers as a confidential
/// client, so `client_secret` is never sent here.
pub async fn refresh_tokens(client: &reqwest::Client, client_id: &str, refresh_token: &str) -> Result<TokenPair, TwitchError> {
    let response = client
        .post(TOKEN_ENDPOINT)
        .form(&[("client_id", client_id), ("refresh_token", refresh_token), ("grant_type", "refresh_token")])
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(TwitchError::Api { status: status.as_u16(), body });
    }
    let parsed: token_response::TokenResponse =
        serde_json::from_str(&body).map_err(|err| TwitchError::Api { status: status.as_u16(), body: err.to_string() })?;
    Ok(TokenPair { access_token: parsed.access_token, refresh_token: parsed.refresh_token })
}

mod token_response {
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub struct TokenResponse {
        pub access_token: String,
        pub refresh_token: String,
    }
}
