import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

const languageSchema = z.string();

export async function getLanguage(): Promise<string> {
  const result = await invoke("settings_get_language");
  return languageSchema.parse(result);
}

export async function setLanguage(language: string): Promise<void> {
  await invoke("settings_set_language", { language });
}

const userNameSchema = z.string().nullable();

/** The name the user wants characters to call them; `null` when not set. */
export async function getUserName(): Promise<string | null> {
  return userNameSchema.parse(await invoke("settings_get_user_name"));
}

/** Saves the name (blank clears it) and returns it as stored: trimmed and capped. */
export async function setUserName(name: string): Promise<string | null> {
  return userNameSchema.parse(await invoke("settings_set_user_name", { name }));
}

// Mirrors `aivt_service_settings::BackgroundSettings` (Rust) — cómo se
// comporta la app fuera de primer plano (Configuración > General).
export interface BackgroundSettings {
  minimizeToTray: boolean;
  notifyProactive: boolean;
  voiceWhileHidden: boolean;
}

const backgroundSettingsSchema = z.object({
  minimizeToTray: z.boolean(),
  notifyProactive: z.boolean(),
  voiceWhileHidden: z.boolean(),
});

export async function getBackgroundSettings(): Promise<BackgroundSettings> {
  const result = await invoke("settings_get_background");
  return backgroundSettingsSchema.parse(result);
}

export async function setBackgroundSettings(next: BackgroundSettings): Promise<BackgroundSettings> {
  const result = await invoke("settings_set_background", { settings: next });
  return backgroundSettingsSchema.parse(result);
}

// Mirrors `aivt_service_settings::{AppearanceSettings, ColorMode, MessageDensity}`
// (Rust) — preferencias de apariencia GLOBALES de la app (no por personaje;
// eso sigue siendo `profile.theme`/`profile.accentColor`). Pestaña Apariencia.
export const colorModeSchema = z.enum(["dark", "light", "system"]);
export type ColorMode = z.infer<typeof colorModeSchema>;

export const messageDensitySchema = z.enum(["compact", "normal", "spacious"]);
export type MessageDensity = z.infer<typeof messageDensitySchema>;

const appearanceSettingsSchema = z.object({
  colorMode: colorModeSchema,
  // `null`/`undefined` = el usuario nunca tocó el slider de redondez: cada
  // tema conserva su propio radio (ver `applyRoundness` en
  // `useActiveProfileTheme.ts`). Un número = override explícito que se
  // mantiene sin importar qué tema elija después.
  roundness: z.number().nullable().optional(),
  density: messageDensitySchema,
  fontId: z.string(),
  fontScale: z.number(),
  // Festive decorations: "off", "auto" (by date) or a festivity id; the particles' own switch.
  // Optional so older stored settings and test fixtures keep working (undefined = off / moving).
  seasonal: z.string().optional(),
  seasonalMotion: z.boolean().optional(),
});
export type AppearanceSettings = z.infer<typeof appearanceSettingsSchema>;

export async function getAppearanceSettings(): Promise<AppearanceSettings> {
  const result = await invoke("settings_get_appearance");
  return appearanceSettingsSchema.parse(result);
}

export async function setAppearanceSettings(next: AppearanceSettings): Promise<AppearanceSettings> {
  const result = await invoke("settings_set_appearance", { settings: next });
  return appearanceSettingsSchema.parse(result);
}

// Flag global "¿ya pasó por el wizard de bienvenida?" — ver
// `aivt_service_settings::SettingsService::onboarding_completed`.
export async function getOnboardingCompleted(): Promise<boolean> {
  const result = await invoke("settings_get_onboarding_completed");
  return z.boolean().parse(result);
}

export async function setOnboardingCompleted(completed: boolean): Promise<boolean> {
  const result = await invoke("settings_set_onboarding_completed", { completed });
  return z.boolean().parse(result);
}

// Abre un enlace en el navegador del sistema (no en la webview de la app) —
// mismo comando Rust que usa el flujo de OAuth de Twitch, expuesto acá de
// forma genérica para la pestaña Créditos.
export async function openExternalUrl(url: string): Promise<void> {
  await invoke("open_external_url", { url });
}

// Abre con el programa predeterminado un documento legal que viaja con la app:
// el acuerdo de uso (en el idioma pedido) o las licencias de terceros.
export async function openLegalDocument(kind: "eula" | "third_party", lang: string): Promise<void> {
  await invoke("open_legal_document", { kind, lang });
}

// Abre el inspector web (DevTools) de la ventana — usado por la opción
// "Inspeccionar" del menú contextual propio (`AppContextMenu.tsx`).
export async function openDevtools(): Promise<void> {
  await invoke("open_devtools");
}

// Foto propia del usuario (no de un personaje) — comandos Rust en
// `commands/profiles.rs` (reusan `ProfileService`'s file I/O) pero
// expuestos acá porque conceptualmente es un setting global, no un
// campo de perfil. Mismo shape que `setProfileAvatar`/`getProfileAvatarBase64`.
export async function setUserAvatar(bytesBase64: string, extension: string): Promise<void> {
  await invoke("user_avatar_set", { bytesBase64, extension });
}

export async function getUserAvatarBase64(): Promise<string | null> {
  const result = await invoke("user_avatar_get");
  return z.string().nullable().parse(result);
}

// Mirrors `aivt_service_settings::{LlmProviderKind, LlmProviderEntry, LlmProvidersSettings,
// AddLlmProviderInput, UpdateLlmProviderInput}` and `aivt_provider_llm::{ConnectionStatus, ModelInfo}` (Rust).

const modelInfoSchema = z.object({ id: z.string() });

export const llmProviderKindSchema = z.enum(["openai", "anthropic", "gemini", "groq", "openrouter", "cerebras", "mistral", "deepseek", "together", "xai", "sambanova", "generic"]);
export type LlmProviderKind = z.infer<typeof llmProviderKindSchema>;

/** Qué recibe un proveedor además de la conversación (espejo de `aivt_provider_llm::PrivacyMode`). */
export const privacyModeSchema = z.enum(["full", "public_only", "minimal"]);
export type PrivacyMode = z.infer<typeof privacyModeSchema>;

const llmProviderEntrySchema = z.object({
  id: z.string(),
  kind: llmProviderKindSchema,
  label: z.string(),
  baseUrl: z.string().nullable(),
  model: z.string(),
  temperature: z.number(),
  maxTokens: z.number(),
  hasApiKey: z.boolean(),
  visionEnabled: z.boolean(),
  enabled: z.boolean(),
  pinned: z.boolean(),
  privacyMode: privacyModeSchema,
  /** `true` si el servidor corre en tu propia red: lo que se le manda no sale a internet. */
  isLocal: z.boolean(),
});
export type LlmProviderEntry = z.infer<typeof llmProviderEntrySchema>;

const llmProvidersSettingsSchema = z.object({
  providers: z.array(llmProviderEntrySchema),
  /** El que responde ahora (puede ser uno temporal tras un cambio automático). */
  currentProviderId: z.string().nullable(),
  /** El que el usuario eligió: se vuelve a él solo cuando responda de nuevo. */
  preferredProviderId: z.string().nullable(),
});
export type LlmProvidersSettings = z.infer<typeof llmProvidersSettingsSchema>;

/** `base_url` solo se usa (y se guarda) para `kind: "generic"`. */
export interface AddLlmProviderInput {
  kind: LlmProviderKind;
  label?: string | null;
  baseUrl?: string | null;
}

export interface UpdateLlmProviderInput {
  label: string;
  baseUrl?: string | null;
  model: string;
  temperature: number;
  maxTokens: number;
  /** `undefined`/omitted leaves the stored key untouched; `""` clears it. */
  apiKey?: string | null;
  visionEnabled: boolean;
  /** `undefined` deja el modo como estaba. */
  privacyMode?: PrivacyMode;
}

const connectionStatusSchema = z.object({
  connected: z.boolean(),
  latencyMs: z.number().nullable(),
  models: z.array(modelInfoSchema),
});
export type ConnectionStatus = z.infer<typeof connectionStatusSchema>;

export async function listLlmProviders(): Promise<LlmProvidersSettings> {
  const result = await invoke("settings_list_llm_providers");
  return llmProvidersSettingsSchema.parse(result);
}

export async function addLlmProvider(input: AddLlmProviderInput): Promise<LlmProviderEntry> {
  const result = await invoke("settings_add_llm_provider", { input });
  return llmProviderEntrySchema.parse(result);
}

export async function updateLlmProvider(id: string, input: UpdateLlmProviderInput): Promise<LlmProviderEntry> {
  const result = await invoke("settings_update_llm_provider", { id, input });
  return llmProviderEntrySchema.parse(result);
}

export async function removeLlmProvider(id: string): Promise<void> {
  await invoke("settings_remove_llm_provider", { id });
}

export async function setLlmProviderEnabled(id: string, enabled: boolean): Promise<LlmProviderEntry> {
  const result = await invoke("settings_set_llm_provider_enabled", { id, enabled });
  return llmProviderEntrySchema.parse(result);
}

export async function setLlmProviderPinned(id: string, pinned: boolean): Promise<LlmProviderEntry> {
  const result = await invoke("settings_set_llm_provider_pinned", { id, pinned });
  return llmProviderEntrySchema.parse(result);
}

export async function setCurrentLlmProvider(id: string): Promise<LlmProvidersSettings> {
  const result = await invoke("settings_set_current_llm_provider", { id });
  return llmProvidersSettingsSchema.parse(result);
}

export async function testLlmProvider(
  kind: LlmProviderKind,
  baseUrl: string | null,
  apiKey: string | null,
  /** Con `apiKey` vacío, el backend usa la key ya guardada de este proveedor. */
  id?: string,
): Promise<ConnectionStatus> {
  const result = await invoke("settings_test_llm_provider", { input: { id, kind, baseUrl, apiKey } });
  return connectionStatusSchema.parse(result);
}

// Mirrors `aivt_service_settings::{TtsBackendKind, TtsProviderEntry, TtsProvidersSettings,
// AddTtsProviderInput, UpdateTtsProviderInput}` (Rust).

export const ttsBackendKindSchema = z.enum(["native", "elevenlabs", "openai", "google", "azure", "fishaudio", "deepgram", "groq", "together", "http"]);
export type TtsBackendKind = z.infer<typeof ttsBackendKindSchema>;

const ttsProviderEntrySchema = z.object({
  id: z.string(),
  kind: ttsBackendKindSchema,
  label: z.string(),
  baseUrl: z.string().nullable(),
  region: z.string().nullable(),
  model: z.string().nullable(),
  hasApiKey: z.boolean(),
  enabled: z.boolean(),
});
export type TtsProviderEntry = z.infer<typeof ttsProviderEntrySchema>;

const ttsProvidersSettingsSchema = z.object({
  providers: z.array(ttsProviderEntrySchema),
  currentProviderId: z.string().nullable(),
});
export type TtsProvidersSettings = z.infer<typeof ttsProvidersSettingsSchema>;

/** `baseUrl` solo se usa (y se guarda) para `kind: "http"`; `region` solo
 * para `kind: "azure"`. Agregar `kind: "native"` se rechaza del lado Rust
 * — ya existe siempre. */
export interface AddTtsProviderInput {
  kind: TtsBackendKind;
  label?: string | null;
  baseUrl?: string | null;
  region?: string | null;
}

export interface UpdateTtsProviderInput {
  label: string;
  baseUrl?: string | null;
  region?: string | null;
  /** Solo `fishaudio`. */
  model?: string | null;
  /** `undefined`/omitted leaves the stored key untouched; `""` clears it. */
  apiKey?: string | null;
}

export async function listTtsProviders(): Promise<TtsProvidersSettings> {
  const result = await invoke("settings_list_tts_providers");
  return ttsProvidersSettingsSchema.parse(result);
}

export async function addTtsProvider(input: AddTtsProviderInput): Promise<TtsProviderEntry> {
  const result = await invoke("settings_add_tts_provider", { input });
  return ttsProviderEntrySchema.parse(result);
}

export async function updateTtsProvider(id: string, input: UpdateTtsProviderInput): Promise<TtsProviderEntry> {
  const result = await invoke("settings_update_tts_provider", { id, input });
  return ttsProviderEntrySchema.parse(result);
}

export async function removeTtsProvider(id: string): Promise<void> {
  await invoke("settings_remove_tts_provider", { id });
}

export async function setTtsProviderEnabled(id: string, enabled: boolean): Promise<TtsProviderEntry> {
  const result = await invoke("settings_set_tts_provider_enabled", { id, enabled });
  return ttsProviderEntrySchema.parse(result);
}

export async function setCurrentTtsProvider(id: string): Promise<TtsProvidersSettings> {
  const result = await invoke("settings_set_current_tts_provider", { id });
  return ttsProvidersSettingsSchema.parse(result);
}

const ttsConnectionTestResultSchema = z.object({
  /** Only populated for `elevenlabs`/`google`/`azure` (each has a real
   * read-only "list voices" probe) — `null` for `openai`/`http`, whose
   * test instead synthesizes and plays `text` out loud, since neither has
   * a listing endpoint. */
  voicesFound: z.number().nullable(),
});
export type TtsConnectionTestResult = z.infer<typeof ttsConnectionTestResultSchema>;

/** Tests a possibly not-yet-saved provider directly, same "typed, not
 * stored" contract as `testLlmProvider` above — never called for
 * `kind: "native"` (nothing to test). */
export async function testTtsProvider(
  kind: TtsBackendKind,
  baseUrl: string | null,
  region: string | null,
  apiKey: string | null,
  text: string,
  /** Con `apiKey` vacío, el backend usa la key ya guardada de este proveedor. */
  id?: string,
  /** Solo `fishaudio`: valor de la cabecera `model`. */
  model?: string | null,
): Promise<TtsConnectionTestResult> {
  const result = await invoke("settings_test_tts_provider", { input: { id, kind, baseUrl, region, model, apiKey, text } });
  return ttsConnectionTestResultSchema.parse(result);
}

// Mirrors `aivt_service_settings::{LlmUsageEntry, UsagePeriod}` (Rust).

export const usagePeriodSchema = z.enum(["today", "last7_days", "last30_days", "all"]);
export type UsagePeriod = z.infer<typeof usagePeriodSchema>;

const llmUsageEntrySchema = z.object({
  providerId: z.string(),
  requests: z.number(),
  errors: z.number(),
  /** Estimados a partir del texto (~4 caracteres por token), no el consumo facturado. */
  inputTokensEstimate: z.number(),
  outputTokensEstimate: z.number(),
  lastUsedAt: z.string().nullable(),
  lastError: z.string().nullable(),
});
export type LlmUsageEntry = z.infer<typeof llmUsageEntrySchema>;

export async function getLlmUsage(period: UsagePeriod): Promise<LlmUsageEntry[]> {
  const result = await invoke("settings_get_llm_usage", { period });
  return z.array(llmUsageEntrySchema).parse(result);
}

export async function resetLlmUsage(): Promise<void> {
  await invoke("settings_reset_llm_usage");
}
