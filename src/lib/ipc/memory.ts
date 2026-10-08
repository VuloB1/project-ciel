import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";

import { memorySchema, type Memory, type MemoryScope } from "@/types/memory";

export async function listMemories(profileId: string): Promise<Memory[]> {
  const result = await invoke("memory_list", { profileId });
  return z.array(memorySchema).parse(result);
}

export async function searchMemories(profileId: string, query: string): Promise<Memory[]> {
  const result = await invoke("memory_search", { profileId, query });
  return z.array(memorySchema).parse(result);
}

export interface NewMemoryInput {
  profileId: string;
  title: string;
  content: string;
  summary?: string;
  category: string;
  importance: number;
}

export async function createMemory(input: NewMemoryInput): Promise<Memory> {
  const result = await invoke("memory_create", { input });
  return memorySchema.parse(result);
}

export async function setMemoryPinned(id: string, pinned: boolean): Promise<void> {
  await invoke("memory_set_pinned", { id, pinned });
}

export async function setMemoryScope(id: string, scope: MemoryScope): Promise<void> {
  await invoke("memory_set_scope", { id, scope });
}

export interface MemoryUpdateInput {
  title: string;
  content: string;
  summary?: string;
  category: string;
  importance: number;
}

export async function updateMemory(id: string, input: MemoryUpdateInput): Promise<Memory> {
  const result = await invoke("memory_update", { id, input });
  return memorySchema.parse(result);
}

export async function deleteMemory(id: string): Promise<void> {
  await invoke("memory_delete", { id });
}

/** Returns once the backfill has *started* — actual progress arrives via
 * `AppEvent` (`embedding_backfill_progress`/`_finished`/`_error`), not the
 * resolution of this promise. See `commands::memory::memory_backfill_embeddings`. */
export async function backfillMemoryEmbeddings(profileId: string): Promise<void> {
  await invoke("memory_backfill_embeddings", { profileId });
}

// Mirrors `aivt_memory::{FusionProposal, FusionGroupInput}` (Rust).

const fusionProposalSchema = z.object({
  originals: z.array(memorySchema),
  title: z.string(),
  summary: z.string().nullable(),
  content: z.string(),
  category: z.string(),
  importance: z.number(),
});
export type FusionProposal = z.infer<typeof fusionProposalSchema>;

export interface FusionGroupInput {
  originalIds: string[];
  title: string;
  summary?: string;
  content: string;
  category: string;
  importance: number;
}

/** Read-only — analyzes every one of `profileId`'s memories and proposes
 * groups worth merging, without writing anything. The Memoria settings
 * card shows these for review; only `applyMemoryFusion` actually changes
 * the database. */
export async function proposeMemoryFusion(profileId: string): Promise<FusionProposal[]> {
  const result = await invoke("memory_propose_fusion", { profileId });
  return z.array(fusionProposalSchema).parse(result);
}

export async function applyMemoryFusion(profileId: string, groups: FusionGroupInput[]): Promise<Memory[]> {
  const result = await invoke("memory_apply_fusion", { profileId, groups });
  return z.array(memorySchema).parse(result);
}
