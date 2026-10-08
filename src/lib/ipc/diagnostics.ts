import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

// Mirrors `aivt_service_diagnostics::DiagnosticsReport` (Rust).

export const severitySchema = z.enum(["ok", "info", "warning", "error"]);
export type Severity = z.infer<typeof severitySchema>;

const checkSchema = z.object({
  id: z.string(),
  severity: severitySchema,
  params: z.record(z.string(), z.string()),
});
export type DiagnosticCheck = z.infer<typeof checkSchema>;

const reportSchema = z.object({
  overall: severitySchema,
  generatedAt: z.string(),
  system: z.object({ appVersion: z.string(), os: z.string(), arch: z.string(), dataDir: z.string() }),
  database: z.object({
    schemaVersion: z.number(),
    latestSchemaVersion: z.number(),
    sizeBytes: z.number(),
    integrityOk: z.boolean(),
    integrityMessage: z.string().nullable(),
    counts: z.array(z.object({ table: z.string(), count: z.number() })),
  }),
  storage: z.object({
    attachmentsBytes: z.number(),
    avatarsBytes: z.number(),
    modelsBytes: z.number(),
    logsBytes: z.number(),
    preRestoreBytes: z.number(),
  }),
  services: z.array(z.object({ name: z.string(), status: z.string() })),
  ai: z.object({
    llm: z.array(
      z.object({
        label: z.string(),
        kind: z.string(),
        enabled: z.boolean(),
        pinned: z.boolean(),
        hasApiKey: z.boolean(),
        ready: z.boolean(),
        current: z.boolean(),
        preferred: z.boolean(),
        cooldownSecs: z.number().nullable(),
      }),
    ),
    tts: z.array(
      z.object({ label: z.string(), kind: z.string(), enabled: z.boolean(), hasApiKey: z.boolean(), current: z.boolean() }),
    ),
    sttModels: z.array(z.object({ size: z.string(), downloaded: z.boolean(), selected: z.boolean() })),
    embeddingDownloaded: z.boolean(),
  }),
  problems: z.array(
    z.object({ timestamp: z.string(), level: z.string(), category: z.string(), module: z.string(), summary: z.string() }),
  ),
  checks: z.array(checkSchema),
});
export type DiagnosticsReport = z.infer<typeof reportSchema>;

export async function runDiagnostics(): Promise<DiagnosticsReport> {
  return reportSchema.parse(await invoke("diagnostics_run"));
}

/** El informe como texto plano ya sin secretos, para copiar o compartir. */
export async function getDiagnosticsText(): Promise<string> {
  return z.string().parse(await invoke("diagnostics_text"));
}

export async function saveDiagnostics(path: string): Promise<void> {
  await invoke("diagnostics_save", { path });
}
