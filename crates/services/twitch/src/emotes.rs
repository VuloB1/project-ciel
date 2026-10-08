use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use futures::StreamExt;

use aivt_database::{DbPool, NewEmote, TwitchEmotesRepository};
use aivt_eventbus::{AppEvent, EventBus};
use aivt_provider_llm::{ChatMessage, ChatRequest, LLMProvider, MessageRole};

use crate::error::TwitchError;

/// Fetches a channel's 7TV/BTTV/FFZ emote sets and caches them —
/// `TwitchService::refresh_emotes` is the public entry point. Each
/// platform is best-effort: a channel with no account on one of the three
/// (or a network hiccup) just contributes nothing from that platform
/// rather than failing the whole refresh — most channels only have some
/// subset of the three set up, if any.
pub(crate) async fn refresh(pool: &DbPool, http: &reqwest::Client, broadcaster_id: &str) -> Result<(i64, i64), TwitchError> {
    let (seventv, bttv, ffz) = tokio::join!(fetch_7tv(http, broadcaster_id), fetch_bttv(http, broadcaster_id), fetch_ffz(http, broadcaster_id));

    let mut fetched = seventv;
    fetched.extend(bttv);
    fetched.extend(ffz);

    let repo = TwitchEmotesRepository::new(pool);
    repo.upsert_and_reconcile(broadcaster_id, &fetched).await?;
    Ok(repo.count_status(broadcaster_id).await?)
}

/// `GET https://7tv.io/v3/users/twitch/{id}` → `.emote_set.emotes[]`, each
/// with `.name`, `.data.tags` (a plain-English hint at what the emote
/// means — the one platform of the three that offers this) and
/// `.data.host` (a protocol-relative base URL + a list of file variants;
/// prefers the static variant so an animated emote doesn't get captioned
/// mid-frame). Shape verified live against a real channel while designing
/// this feature.
async fn fetch_7tv(client: &reqwest::Client, broadcaster_id: &str) -> Vec<NewEmote> {
    let url = format!("https://7tv.io/v3/users/twitch/{broadcaster_id}");
    let body = match fetch_json(client, &url, "7TV").await {
        Some(body) => body,
        None => return Vec::new(),
    };

    let Some(emotes) = body.get("emote_set").and_then(|s| s.get("emotes")).and_then(|e| e.as_array()) else {
        return Vec::new();
    };

    emotes
        .iter()
        .filter_map(|emote| {
            let id = emote.get("id")?.as_str()?.to_string();
            let name = emote.get("name")?.as_str()?.to_string();
            let data = emote.get("data")?;

            let tags: Vec<&str> = data.get("tags").and_then(|t| t.as_array()).map(|arr| arr.iter().filter_map(|t| t.as_str()).collect()).unwrap_or_default();

            let host = data.get("host")?;
            let host_url = host.get("url")?.as_str()?;
            let files = host.get("files").and_then(|f| f.as_array());
            let filename = files
                .and_then(|files| {
                    files.iter().find(|f| f.get("name").and_then(|n| n.as_str()) == Some("1x_static.webp")).or_else(|| files.first())
                })
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())?;

            Some(NewEmote {
                source: "7tv".to_string(),
                emote_id: id,
                name,
                tags: if tags.is_empty() { None } else { Some(tags.join(", ")) },
                image_url: format!("https:{host_url}/{filename}"),
            })
        })
        .collect()
}

/// `GET https://api.betterttv.net/3/cached/users/twitch/{id}` →
/// `.channelEmotes[]` + `.sharedEmotes[]`, each `{id, code, ...}` — `code`
/// is the emote's name. Image at `https://cdn.betterttv.net/emote/{id}/1x`
/// (no extension needed — verified live, the CDN picks a format itself).
async fn fetch_bttv(client: &reqwest::Client, broadcaster_id: &str) -> Vec<NewEmote> {
    let url = format!("https://api.betterttv.net/3/cached/users/twitch/{broadcaster_id}");
    let body = match fetch_json(client, &url, "BTTV").await {
        Some(body) => body,
        None => return Vec::new(),
    };

    ["channelEmotes", "sharedEmotes"]
        .iter()
        .filter_map(|key| body.get(key).and_then(|v| v.as_array()))
        .flatten()
        .filter_map(|emote| {
            let id = emote.get("id")?.as_str()?.to_string();
            let code = emote.get("code")?.as_str()?.to_string();
            Some(NewEmote {
                image_url: format!("https://cdn.betterttv.net/emote/{id}/1x"),
                source: "bttv".to_string(),
                emote_id: id,
                name: code,
                tags: None,
            })
        })
        .collect()
}

/// `GET https://api.frankerfacez.com/v1/room/id/{id}` →
/// `.sets[room.set].emoticons[]`, each `{id, name, urls: {"1": url, ...}}`.
/// `urls` came back as full `https://` URLs in live testing, but a
/// protocol-relative `//cdn...` is handled too in case a smaller size ever
/// omits it.
async fn fetch_ffz(client: &reqwest::Client, broadcaster_id: &str) -> Vec<NewEmote> {
    let url = format!("https://api.frankerfacez.com/v1/room/id/{broadcaster_id}");
    let body = match fetch_json(client, &url, "FFZ").await {
        Some(body) => body,
        None => return Vec::new(),
    };

    let Some(set_id) = body.get("room").and_then(|r| r.get("set")).and_then(|s| s.as_i64()) else {
        return Vec::new();
    };
    let Some(emoticons) = body.get("sets").and_then(|sets| sets.get(set_id.to_string().as_str())).and_then(|s| s.get("emoticons")).and_then(|e| e.as_array())
    else {
        return Vec::new();
    };

    emoticons
        .iter()
        .filter_map(|emote| {
            let id = emote.get("id")?.as_i64()?;
            let name = emote.get("name")?.as_str()?.to_string();
            let urls = emote.get("urls")?;
            let raw_url = urls.get("2").or_else(|| urls.get("1")).and_then(|u| u.as_str())?;
            let image_url = if let Some(rest) = raw_url.strip_prefix("//") { format!("https://{rest}") } else { raw_url.to_string() };
            Some(NewEmote { source: "ffz".to_string(), emote_id: id.to_string(), name, tags: None, image_url })
        })
        .collect()
}

async fn fetch_json(client: &reqwest::Client, url: &str, platform: &str) -> Option<serde_json::Value> {
    match client.get(url).send().await {
        Ok(response) if response.status().is_success() => match response.json().await {
            Ok(body) => Some(body),
            Err(err) => {
                tracing::warn!(target: "system", "twitch: {platform} devolvió una respuesta inesperada: {err}");
                None
            }
        },
        Ok(response) => {
            tracing::info!(target: "system", "twitch: {platform} no tiene nada para este canal (status {})", response.status());
            None
        }
        Err(err) => {
            tracing::warn!(target: "system", "twitch: no se pudo consultar {platform}: {err}");
            None
        }
    }
}

const CAPTION_PROMPT: &str =
    "Describí este emote de Twitch en una frase breve y concreta: qué gesto, cara o idea representa. Sin preámbulos, sin decir \"esta imagen muestra\".";
const PROGRESS_THROTTLE: Duration = Duration::from_millis(250);

/// Describes every emote still missing a `description` for `broadcaster_id`
/// by vision, one at a time, against the same AI provider connection
/// already configured for everyday chat (Configuración > Proveedores IA) —
/// no separate connection to set up. Runs to completion;
/// `TwitchService::start_emote_captioning` is what detaches this into the
/// background so the Tauri command returns right away — real progress
/// travels only through the `TwitchEmoteCaption*` events published here,
/// same shape as `MemoryService::backfill_embeddings`.
///
/// One failed call to the vision provider is treated as the connection
/// itself being broken (every subsequent emote would fail identically), so
/// it publishes `TwitchEmoteCaptionError` and stops rather than burning
/// time on the rest. A single emote's image failing to download is
/// tolerated individually — logged, skipped, the loop continues.
pub(crate) async fn caption(pool: DbPool, http: reqwest::Client, event_bus: EventBus, broadcaster_id: String, provider: Arc<dyn LLMProvider>, model: String) {
    let repo = TwitchEmotesRepository::new(&pool);
    let pending = match repo.list_uncaptioned(&broadcaster_id).await {
        Ok(pending) => pending,
        Err(err) => {
            event_bus.publish(AppEvent::TwitchEmoteCaptionError { broadcaster_id, message: err.to_string() });
            return;
        }
    };

    let total = pending.len();
    event_bus.publish(AppEvent::TwitchEmoteCaptionProgress { broadcaster_id: broadcaster_id.clone(), captioned: 0, total });
    if total == 0 {
        event_bus.publish(AppEvent::TwitchEmoteCaptionFinished { broadcaster_id, captioned: 0, total: 0 });
        return;
    }

    let mut captioned = 0usize;
    let mut last_emit = Instant::now();

    for emote in pending {
        let Some(bytes) = download_image(&http, &emote.image_url).await else {
            tracing::warn!(target: "system", "twitch: no se pudo descargar la imagen del emote \"{}\", se omite", emote.name);
            continue;
        };

        let data_url = match tokio::task::spawn_blocking(move || encode_png_data_url(&bytes)).await {
            Ok(Ok(data_url)) => data_url,
            Ok(Err(err)) => {
                tracing::warn!(target: "system", "twitch: no se pudo decodificar la imagen del emote \"{}\", se omite: {err}", emote.name);
                continue;
            }
            Err(err) => {
                tracing::warn!(target: "system", "twitch: la tarea de decodificar la imagen del emote \"{}\" falló: {err}", emote.name);
                continue;
            }
        };

        let mut message = ChatMessage::text(MessageRole::User, format!("{CAPTION_PROMPT} (nombre del emote: {})", emote.name));
        message.images.push(data_url);
        let request = ChatRequest { model: model.clone(), messages: vec![message], temperature: None, max_tokens: Some(100), tools: Vec::new(), context: Vec::new(), contains_stored_memories: false };

        let mut stream = provider.stream_chat(request);
        let mut description = String::new();
        let mut provider_failed = None;
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    description.push_str(&chunk.delta);
                    if chunk.finished {
                        break;
                    }
                }
                Err(err) => {
                    provider_failed = Some(err.to_string());
                    break;
                }
            }
        }

        if let Some(message) = provider_failed {
            event_bus.publish(AppEvent::TwitchEmoteCaptionError { broadcaster_id, message });
            return;
        }

        let description = description.trim();
        if !description.is_empty() {
            if let Err(err) = repo.set_description(&broadcaster_id, &emote.source, &emote.emote_id, description).await {
                tracing::warn!(target: "system", "twitch: no se pudo guardar la descripción del emote \"{}\": {err}", emote.name);
            }
        }

        captioned += 1;
        if last_emit.elapsed() >= PROGRESS_THROTTLE || captioned == total {
            event_bus.publish(AppEvent::TwitchEmoteCaptionProgress { broadcaster_id: broadcaster_id.clone(), captioned, total });
            last_emit = Instant::now();
        }
    }

    event_bus.publish(AppEvent::TwitchEmoteCaptionFinished { broadcaster_id, captioned, total });
}

async fn download_image(client: &reqwest::Client, url: &str) -> Option<Vec<u8>> {
    let response = client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.bytes().await.ok().map(|b| b.to_vec())
}

/// Decodes the downloaded emote and re-encodes it as PNG rather than
/// forwarding whatever format the CDN served — 7TV emotes ship as WebP
/// (BTTV picks its own format per emote too), and at least one real local
/// OpenAI-compatible server (LM Studio, verified live) rejects a WebP
/// `image_url` outright ("'url' field must be a base64 encoded image"),
/// failing every single emote. PNG decodes on effectively everything, so
/// this sidesteps the format question instead of guessing which servers
/// support what.
fn encode_png_data_url(bytes: &[u8]) -> Result<String, image::ImageError> {
    let image = image::load_from_memory(bytes)?;
    let mut png_bytes = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut png_bytes), image::ImageFormat::Png)?;
    Ok(format!("data:image/png;base64,{}", BASE64.encode(&png_bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real 7TV emote (animated WebP, "WAYTOODANK", fetched live while
    /// diagnosing this) — the exact shape of file that made LM Studio's
    /// vision endpoint reject every captioning request before this
    /// function existed.
    const SAMPLE_WEBP: &[u8] = include_bytes!("testdata/sample_emote.webp");

    #[test]
    fn encode_png_data_url_decodes_real_webp_emote() {
        let data_url = encode_png_data_url(SAMPLE_WEBP).expect("a real 7TV emote should decode");
        assert!(data_url.starts_with("data:image/png;base64,"));

        let b64 = data_url.strip_prefix("data:image/png;base64,").unwrap();
        let png_bytes = BASE64.decode(b64).expect("valid base64");
        assert_eq!(&png_bytes[..8], b"\x89PNG\r\n\x1a\n", "output should be a real PNG file");
    }

    #[test]
    fn encode_png_data_url_rejects_garbage() {
        assert!(encode_png_data_url(b"not an image").is_err());
    }
}
