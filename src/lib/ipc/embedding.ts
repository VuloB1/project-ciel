import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

// Mirrors `aivt_provider_embedding` + `commands::embedding` (Rust). Unlike
// STT there's only ever one model — no size/GPU choice — so the shape here
// is simpler than `stt.ts`: just whether it's downloaded.

const embeddingModelStatusSchema = z.object({ downloaded: z.boolean(), approxSizeMb: z.number() });
export type EmbeddingModelStatus = z.infer<typeof embeddingModelStatusSchema>;

export async function getEmbeddingModelStatus(): Promise<EmbeddingModelStatus> {
  const result = await invoke("embedding_get_model_status");
  return embeddingModelStatusSchema.parse(result);
}

/** Returns once the download has *started* — actual progress arrives via
 * `AppEvent` (`embedding_model_download_progress`/`_finished`/`_error`),
 * not the resolution of this promise. See
 * `commands::embedding::embedding_download_model`. */
export async function downloadEmbeddingModel(): Promise<void> {
  await invoke("embedding_download_model");
}

export async function deleteEmbeddingModel(): Promise<void> {
  await invoke("embedding_delete_model");
}
