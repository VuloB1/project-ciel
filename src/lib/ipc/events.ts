import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";

export type ServiceStatus = "stopped" | "starting" | "running" | "error" | "stopping";

/** Mirrors `aivt_eventbus::CallState` (Rust) — what's happening *inside* an
 * active call, distinct from `ServiceStatus` (mic open/closed lifecycle). */
export type CallState = "idle" | "listening" | "transcribing" | "thinking" | "speaking" | "error";

/**
 * Mirrors `aivt_eventbus::AppEvent` (Rust). Grows one variant at a time,
 * in lockstep with the backend enum — see docs/architecture.md. The tag
 * (`type`) is snake_case but every variant's fields are camelCase
 * (`#[serde(rename_all_fields = "camelCase")]` on the Rust side), matching
 * every other DTO that crosses IPC.
 */
export type AppEvent =
  | { type: "service_status_changed"; service: string; status: ServiceStatus }
  | { type: "heartbeat"; timestamp: string }
  | { type: "ai_response_started"; requestId: string; profileId: string; conversationId: string; messageId: string }
  | { type: "ai_response_token"; requestId: string; token: string }
  | { type: "ai_response_finished"; requestId: string }
  | { type: "ai_response_cancelled"; requestId: string }
  | { type: "ai_response_error"; requestId: string; message: string }
  | { type: "memories_updated"; profileId: string }
  | { type: "dictation_partial"; text: string }
  | { type: "voice_playback_started"; profileId: string; messageId: string | null }
  | { type: "voice_playback_finished"; profileId: string; messageId: string | null }
  | { type: "voice_playback_error"; profileId: string; messageId: string | null; message: string }
  | { type: "twitch_message_received"; chatterDisplayName: string; text: string }
  | { type: "twitch_message_sent"; characterName: string; text: string }
  | { type: "twitch_emote_caption_progress"; broadcasterId: string; captioned: number; total: number }
  | { type: "twitch_emote_caption_finished"; broadcasterId: string; captioned: number; total: number }
  | { type: "twitch_emote_caption_error"; broadcasterId: string; message: string }
  | { type: "discord_message_received"; authorDisplayName: string; text: string }
  | { type: "discord_message_sent"; characterName: string; text: string }
  | { type: "call_state_changed"; state: CallState }
  | { type: "call_user_utterance"; text: string }
  | { type: "call_character_reply"; text: string }
  | { type: "call_mic_level"; level: number }
  | { type: "call_error"; message: string }
  | { type: "stt_model_download_progress"; modelSize: string; downloadedBytes: number; totalBytes: number | null }
  | { type: "stt_model_download_finished"; modelSize: string }
  | { type: "stt_model_download_error"; modelSize: string; message: string }
  | { type: "embedding_model_download_progress"; downloadedBytes: number; totalBytes: number | null }
  | { type: "embedding_model_download_finished" }
  | { type: "embedding_model_download_error"; message: string }
  | { type: "obscura_download_progress"; downloadedBytes: number; totalBytes: number | null }
  | { type: "obscura_download_finished" }
  | { type: "obscura_download_error"; message: string }
  | { type: "embedding_backfill_progress"; profileId: string; embedded: number; total: number }
  | { type: "embedding_backfill_finished"; profileId: string; embedded: number; total: number }
  | { type: "embedding_backfill_error"; profileId: string; message: string }
  | { type: "mood_updated"; profileId: string; valence: number; energy: number }
  | { type: "proactive_message_sent"; profileId: string; conversationId: string; content: string }
  | { type: "chat_audio_saved"; conversationId: string; messageId: string; attachmentId: string; mimeType: string }
  | { type: "llm_provider_switched"; fromProviderId: string; toProviderId: string; reason: string }
  | { type: "profile_import_requested"; path: string };

const EVENT_BRIDGE_CHANNEL = "app://event";

/** Subscribes to the backend EventBus bridge for the lifetime of the component. */
export function useAppEvent(handler: (event: AppEvent) => void): void {
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    listen<AppEvent>(EVENT_BRIDGE_CHANNEL, (event) => {
      handler(event.payload);
    })
      .then((fn) => {
        // The effect may already have been cleaned up (React StrictMode's
        // dev-mode double-invoke, or a fast unmount) before `listen`
        // resolved — unlisten immediately instead of leaking it.
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((err: unknown) => {
        // Expected outside a real Tauri webview (e.g. `vite dev` opened in
        // a plain browser) — never let it surface as an unhandled rejection.
        console.warn("[AIVT] no se pudo suscribir a los eventos de la app", err);
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
    // Intentionally subscribing once per mount rather than re-subscribing
    // whenever the caller passes a new `handler` reference.
  }, []);
}
