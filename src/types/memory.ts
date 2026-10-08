import { z } from "zod";

// Mirrors `aivt_database::Memory` (Rust) — camelCase on both sides.

export const memoryCategorySchema = z.enum(["person", "preference", "event", "fact", "other"]);
export type MemoryCategory = z.infer<typeof memoryCategorySchema>;

export const memoryScopeSchema = z.enum(["private", "public"]);
export type MemoryScope = z.infer<typeof memoryScopeSchema>;

export const memorySchema = z.object({
  id: z.string(),
  profileId: z.string(),
  title: z.string(),
  content: z.string(),
  summary: z.string().nullable(),
  // Backend categories are open text ("other" catches anything the model
  // returns that isn't one of the known values) — fall back rather than
  // rejecting the whole memory if it doesn't match the known set.
  category: memoryCategorySchema.catch("other"),
  importance: z.number(),
  confidence: z.number(),
  source: z.string(),
  sourceId: z.string().nullable(),
  isPinned: z.boolean(),
  // "private": solo Chat y Llamada la usan. "public": también Twitch y Discord.
  // Un valor desconocido se trata como privado: ante la duda, no se expone.
  scope: memoryScopeSchema.catch("private"),
  createdAt: z.string(),
  updatedAt: z.string(),
  lastAccessedAt: z.string().nullable(),
  accessCount: z.number(),
});
export type Memory = z.infer<typeof memorySchema>;
