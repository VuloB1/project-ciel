use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;

use aivt_eventbus::EventBus;
use aivt_service_profiles::ProfileService;

use crate::overlay_html::OVERLAY_HTML;
use crate::reducer::{Reducer, VisualState};

#[derive(Clone)]
pub struct ServerState {
    pub profiles: Arc<ProfileService>,
    pub event_bus: EventBus,
}

pub fn router(state: ServerState) -> Router {
    Router::new()
        .route("/overlay", get(overlay_page))
        .route("/avatar/{profile_id}/{state}", get(avatar_image))
        .route("/ws", get(ws_upgrade))
        .with_state(state)
}

async fn overlay_page() -> Html<&'static str> {
    // Query params (`profileId`) are read client-side by the page's own JS
    // (`location.search`) — same split the old overlay window used between
    // `index.html?window=avatar&profileId=...` and `AvatarOverlay.tsx`
    // reading it via `window.location.search`.
    Html(OVERLAY_HTML)
}

/// `GET /avatar/{profile_id}/{state}` — raw image bytes for one of the
/// profile's three reactive states, falling back to the profile's regular
/// picture when that state has no dedicated upload. This fallback only used
/// to exist in the frontend (`useAvatarStateImage.ts`'s `loadOne`) since the
/// old `avatar_get_state_image` Tauri command deliberately returned `None`
/// and let the (Tauri-only) frontend decide — an OBS Browser Source has no
/// such frontend to hand off to, so this is a from-scratch Rust port of that
/// same two-step resolution.
async fn avatar_image(State(state): State<ServerState>, Path((profile_id, state_param)): Path<(String, String)>) -> Response {
    let Some(visual_state) = VisualState::from_url_param(&state_param) else {
        return (StatusCode::BAD_REQUEST, "unknown avatar state").into_response();
    };

    let profile = match state.profiles.get(&profile_id).await {
        Ok(Some(profile)) => profile,
        Ok(None) => return (StatusCode::NOT_FOUND, "unknown profile").into_response(),
        Err(err) => {
            tracing::warn!(target: "system", "avatar_server: no se pudo leer el perfil {profile_id}: {err}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "profile lookup failed").into_response();
        }
    };

    let state_path = match visual_state {
        VisualState::Idle => profile.avatar_idle_path,
        VisualState::Thinking => profile.avatar_thinking_path,
        VisualState::Talking => profile.avatar_talking_path,
    };
    let Some(path) = state_path.or(profile.avatar_path) else {
        return (StatusCode::NOT_FOUND, "no image for this state").into_response();
    };

    let bytes = match state.profiles.read_avatar(&path).await {
        Ok(bytes) => bytes,
        Err(err) => {
            tracing::warn!(target: "system", "avatar_server: no se pudo leer la imagen '{path}': {err}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "image read failed").into_response();
        }
    };

    ([(header::CONTENT_TYPE, content_type_for_path(&path))], bytes).into_response()
}

fn content_type_for_path(path: &str) -> &'static str {
    let extension = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
}

#[derive(Deserialize)]
struct WsParams {
    #[serde(rename = "profileId")]
    profile_id: String,
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<ServerState>, Query(params): Query<WsParams>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state, params.profile_id))
}

/// One `Reducer` per connection (see `reducer.rs`'s doc comment) — sends the
/// current state immediately on connect (so the page shows something before
/// the first real event arrives), then pushes one message per actual
/// transition for as long as the socket and the event bus both stay open.
async fn handle_socket(mut socket: WebSocket, state: ServerState, profile_id: String) {
    let mut reducer = Reducer::new(profile_id);
    if socket.send(Message::Text(state_message(reducer.state()).into())).await.is_err() {
        return;
    }

    let mut rx = state.event_bus.subscribe();
    loop {
        tokio::select! {
            event = rx.recv() => match event {
                Ok(event) => {
                    if let Some(next) = reducer.apply(&event) {
                        if socket.send(Message::Text(state_message(next).into())).await.is_err() {
                            return;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            },
            // Nothing the client sends is meaningful (the overlay page never
            // sends anything) — this branch exists purely to notice the
            // socket closing (OBS removing the source, the tab navigating
            // away) and stop the loop instead of leaking it.
            incoming = socket.recv() => {
                if !matches!(incoming, Some(Ok(_))) {
                    return;
                }
            }
        }
    }
}

fn state_message(state: VisualState) -> String {
    serde_json::json!({ "state": state }).to_string()
}
