import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

import { conversationSchema, messageSchema, type Conversation, type Message } from "@/types/chat";
import { memorySchema, type Memory } from "@/types/memory";

export async function listConversations(profileId: string): Promise<Conversation[]> {
  const result = await invoke("chat_list_conversations", { profileId });
  return z.array(conversationSchema).parse(result);
}

export async function createConversation(profileId: string): Promise<Conversation> {
  const result = await invoke("chat_create_conversation", { profileId });
  return conversationSchema.parse(result);
}

export async function getMessages(conversationId: string): Promise<Message[]> {
  const result = await invoke("chat_get_messages", { conversationId });
  return z.array(messageSchema).parse(result);
}

export async function renameConversation(id: string, title: string): Promise<void> {
  await invoke("chat_rename_conversation", { id, title });
}

export async function archiveConversation(id: string): Promise<void> {
  await invoke("chat_archive_conversation", { id });
}

export async function deleteConversation(id: string): Promise<void> {
  await invoke("chat_delete_conversation", { id });
}

export async function cancelGeneration(conversationId: string): Promise<void> {
  await invoke("chat_cancel_generation", { conversationId });
}

export async function setConversationPinned(id: string, pinned: boolean): Promise<void> {
  await invoke("chat_set_conversation_pinned", { id, pinned });
}

/** `orderedIds` is the *whole* pinned list for the profile, front to back
 * — the backend rewrites every pinned conversation's order from this,
 * rather than trying to infer a move from a single (from, to) pair. */
export async function reorderPinnedConversations(orderedIds: string[]): Promise<void> {
  await invoke("chat_reorder_pinned_conversations", { orderedIds });
}

export interface CallTranscriptEntryInput {
  from: "user" | "character" | "error";
  text: string;
}

/** "Exportar a Chat" desde la pestaña Llamada — crea una conversación
 * nueva (`source: "call"`) con la transcripción ya escrita, sin pasar por
 * el LLM. Las líneas `from: "error"` se filtran del lado del backend. */
export async function importCallTranscript(profileId: string, entries: CallTranscriptEntryInput[]): Promise<Conversation> {
  const result = await invoke("chat_import_call_transcript", { profileId, entries });
  return conversationSchema.parse(result);
}

export interface ChatImageInput {
  base64: string;
  mimeType: string;
  extension: string;
}

export async function sendMessage(
  profileId: string,
  conversationId: string,
  content: string,
  image?: ChatImageInput,
): Promise<Message> {
  const result = await invoke("chat_send_message", { profileId, conversationId, content, image: image ?? null });
  return messageSchema.parse(result);
}

/** Base64 (no `data:` prefix) — same convention as `getProfileAvatarBase64`. */
export async function getAttachmentBase64(attachmentId: string): Promise<string> {
  const result = await invoke("chat_get_attachment_base64", { attachmentId });
  return z.string().parse(result);
}

// Mirrors `aivt_service_settings::ChatAudioSettings` (Rust).
export const chatAudioSettingsSchema = z.object({
  saveEnabled: z.boolean(),
  retentionDays: z.number(),
});
export type ChatAudioSettings = z.infer<typeof chatAudioSettingsSchema>;

export async function getChatAudioSettings(): Promise<ChatAudioSettings> {
  const result = await invoke("chat_get_audio_settings");
  return chatAudioSettingsSchema.parse(result);
}

export async function setChatAudioSettings(settings: ChatAudioSettings): Promise<ChatAudioSettings> {
  const result = await invoke("chat_set_audio_settings", { settings });
  return chatAudioSettingsSchema.parse(result);
}

/** Runs memory extraction immediately, ignoring the usual automatic
 * (every-few-messages) threshold — the Memoria tab / conversation menu's
 * "extract now" action. */
export async function extractMemories(conversationId: string, profileId: string): Promise<Memory[]> {
  const result = await invoke("chat_extract_memories", { conversationId, profileId });
  return z.array(memorySchema).parse(result);
}
