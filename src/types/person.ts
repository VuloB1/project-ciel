import { z } from "zod";

// Mirrors `aivt_database::Person` (Rust) — camelCase on both sides.

export const personRoleSchema = z.enum(["self", "nickname", "ai", "friend", "twitch_chatter", "discord_chatter"]);
export type PersonRole = z.infer<typeof personRoleSchema>;

export const personSchema = z.object({
  id: z.string(),
  canonicalName: z.string(),
  aliases: z.array(z.string()),
  // Backend roles are open text — fall back rather than rejecting the
  // whole row if it doesn't match the known set (same reasoning as
  // `memoryCategorySchema.catch("other")`).
  role: personRoleSchema.catch("friend"),
  source: z.string(),
  externalKey: z.string().nullable(),
  mentionCount: z.number(),
  lastMentionedAt: z.string(),
  createdAt: z.string(),
  updatedAt: z.string(),
});
export type Person = z.infer<typeof personSchema>;
