import { z } from "zod";

// Mirrors `aivt_database::{Conversation, Message}` (Rust) — camelCase on
// both sides, no transformation layer needed.

export const conversationSchema = z.object({
  id: z.string(),
  profileId: z.string(),
  title: z.string(),
  source: z.string(),
  model: z.string().nullable(),
  isArchived: z.boolean(),
  createdAt: z.string(),
  updatedAt: z.string(),
  isPinned: z.boolean(),
  pinOrder: z.number(),
});
export type Conversation = z.infer<typeof conversationSchema>;

export const messageRoleSchema = z.enum(["user", "assistant", "system"]);
export const messageStatusSchema = z.enum(["streaming", "complete", "error", "cancelled"]);
export type MessageStatus = z.infer<typeof messageStatusSchema>;

export const messageSchema = z.object({
  id: z.string(),
  conversationId: z.string(),
  role: messageRoleSchema,
  content: z.string(),
  status: messageStatusSchema,
  providerId: z.string().nullable(),
  model: z.string().nullable(),
  inputTokens: z.number().nullable(),
  outputTokens: z.number().nullable(),
  createdAt: z.string(),
  // Present when the message has an image attached (Fase 3: Multimodal) —
  // mirrors `aivt_service_chat::MessageView`'s flattened shape. `undefined`
  // when the IPC layer's own streaming placeholder builds a `Message`
  // client-side (see chatStore.ts), so both are optional rather than
  // nullable.
  attachmentId: z.string().nullable().optional(),
  attachmentMimeType: z.string().nullable().optional(),
});
export type Message = z.infer<typeof messageSchema>;
