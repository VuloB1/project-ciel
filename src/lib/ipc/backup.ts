import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

// Mirrors `aivt_service_backup::{BackupSummary, RestorePreview}` (Rust).

const backupSummarySchema = z.object({
  path: z.string(),
  sizeBytes: z.number(),
  files: z.number(),
  createdAt: z.string(),
});
export type BackupSummary = z.infer<typeof backupSummarySchema>;

const restorePreviewSchema = z.object({
  createdAt: z.string(),
  appVersion: z.string(),
  files: z.number(),
  sizeBytes: z.number(),
});
export type RestorePreview = z.infer<typeof restorePreviewSchema>;

/** Copia completa (base de datos, adjuntos y avatares) en `path`. No incluye las API keys. */
export async function createBackup(path: string): Promise<BackupSummary> {
  return backupSummarySchema.parse(await invoke("backup_create", { path }));
}

/** Valida la copia y la deja preparada; NO cambia nada hasta `restartApp`. */
export async function prepareRestore(path: string): Promise<RestorePreview> {
  return restorePreviewSchema.parse(await invoke("backup_prepare_restore", { path }));
}

export async function cancelRestore(): Promise<void> {
  await invoke("backup_cancel_restore");
}

/** Reinicia la app: al arrancar aplica la restauración preparada. */
export async function restartApp(): Promise<void> {
  await invoke("backup_restart_app");
}
