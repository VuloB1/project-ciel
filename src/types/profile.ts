import { z } from "zod";

// Mirrors `aivt_database::{Profile, ProfileIdentity, ProfilePersonality}`
// (Rust) — both sides use `#[serde(rename_all = "camelCase")]` / these
// camelCase keys, so no transformation layer is needed at the IPC boundary.

export const profileIdentitySchema = z.object({
  lore: z.string(),
  relationshipToUser: z.string(),
  userRole: z.string(),
  speakingStyle: z.string(),
  specialInstructions: z.string(),
});

export const profilePersonalitySchema = z.object({
  freeDescription: z.string(),
  warmth: z.number(),
  empathy: z.number(),
  humor: z.number(),
  sarcasm: z.number(),
  formality: z.number(),
  curiosity: z.number(),
  energy: z.number(),
  spontaneity: z.number(),
  initiative: z.number(),
  affection: z.number(),
  shyness: z.number(),
  confidence: z.number(),
  expressiveness: z.number(),
  verbosity: z.number(),
  flirtiness: z.number(),
  emotionalIntensity: z.number(),
  teasingTendency: z.number(),
  questionTendency: z.number(),
  initiateConversationTendency: z.number(),
});

// Fase 5 — mirrors `aivt_database::ProfileVoice`.
export const profileVoiceSchema = z.object({
  enabled: z.boolean(),
  voiceId: z.string(),
  rate: z.number(),
  pitch: z.number(),
  volume: z.number(),
});

export const profileSchema = z.object({
  id: z.string(),
  name: z.string(),
  nickname: z.string().nullable(),
  avatarPath: z.string().nullable(),
  description: z.string().nullable(),
  ageFictional: z.string().nullable(),
  pronouns: z.string().nullable(),
  occupation: z.string().nullable(),
  species: z.string().nullable(),
  accentColor: z.string().nullable(),
  theme: z.string().nullable(),
  isArchived: z.boolean(),
  createdAt: z.string(),
  updatedAt: z.string(),
  identity: profileIdentitySchema,
  personality: profilePersonalitySchema,
  voice: profileVoiceSchema,
  avatarIdlePath: z.string().nullable(),
  avatarThinkingPath: z.string().nullable(),
  avatarTalkingPath: z.string().nullable(),
});

export type ProfileIdentity = z.infer<typeof profileIdentitySchema>;
export type ProfilePersonality = z.infer<typeof profilePersonalitySchema>;
export type ProfileVoice = z.infer<typeof profileVoiceSchema>;
export type Profile = z.infer<typeof profileSchema>;

/** Payload for `profiles_update` / `profiles_preview_prompt` — everything
 * editable on the Identidad page except server-assigned fields. */
export type ProfileUpdateInput = Pick<
  Profile,
  | "name"
  | "nickname"
  | "description"
  | "ageFictional"
  | "pronouns"
  | "occupation"
  | "species"
  | "accentColor"
  | "theme"
  | "identity"
  | "personality"
>;

/** Builds the `ProfileUpdateInput` a save call needs from a full `Profile`,
 * changing only what the caller explicitly overrides — the shared "read the
 * current profile, patch a couple of fields, submit the whole thing" shape
 * both `IdentityEditor`'s form and Configuración > Apariencia's per-field
 * autosave need, since `profiles_update` always replaces every editable
 * field rather than patching. */
export function profileToUpdateInput(profile: Profile): ProfileUpdateInput {
  return {
    name: profile.name,
    nickname: profile.nickname,
    description: profile.description,
    ageFictional: profile.ageFictional,
    pronouns: profile.pronouns,
    occupation: profile.occupation,
    species: profile.species,
    accentColor: profile.accentColor,
    theme: profile.theme,
    identity: profile.identity,
    personality: profile.personality,
  };
}

export const PERSONALITY_TRAIT_KEYS = [
  "warmth",
  "empathy",
  "humor",
  "sarcasm",
  "formality",
  "curiosity",
  "energy",
  "spontaneity",
  "initiative",
  "affection",
  "shyness",
  "confidence",
  "expressiveness",
  "verbosity",
  "flirtiness",
  "emotionalIntensity",
  "teasingTendency",
  "questionTendency",
  "initiateConversationTendency",
] as const satisfies readonly (keyof Omit<ProfilePersonality, "freeDescription">)[];

export const DEFAULT_PERSONALITY: ProfilePersonality = {
  freeDescription: "",
  warmth: 50,
  empathy: 50,
  humor: 50,
  sarcasm: 20,
  formality: 30,
  curiosity: 50,
  energy: 50,
  spontaneity: 50,
  initiative: 40,
  affection: 50,
  shyness: 30,
  confidence: 50,
  expressiveness: 50,
  verbosity: 50,
  flirtiness: 0,
  emotionalIntensity: 50,
  teasingTendency: 30,
  questionTendency: 40,
  initiateConversationTendency: 30,
};

export const DEFAULT_IDENTITY: ProfileIdentity = {
  lore: "",
  relationshipToUser: "",
  userRole: "",
  speakingStyle: "",
  specialInstructions: "",
};

export const DEFAULT_VOICE: ProfileVoice = {
  enabled: false,
  voiceId: "",
  rate: 1,
  pitch: 1,
  volume: 1,
};
