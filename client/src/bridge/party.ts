// Typed wrappers for the party-layer commands. Thin over `invoke` so the pages
// don't repeat command-name strings.

import type { PartyQueue, QueueEntry } from "@/types/party";
import { invoke } from "./runtime";

export const partyQueueList = (): Promise<PartyQueue> => invoke<PartyQueue>("party_queue_list");

export interface AddToQueueArgs {
  query?: string;
  fileHash?: string;
  requestedBy?: string;
}

export const partyQueueAdd = (args: AddToQueueArgs): Promise<{ id: string; entry: QueueEntry }> =>
  invoke("party_queue_add", { ...args } as Record<string, unknown>);

export const partyQueueRemove = (id: string): Promise<{ removed: boolean }> =>
  invoke("party_queue_remove", { id });

export const partyQueueReorder = (id: string, position: number): Promise<{ reordered: boolean }> =>
  invoke("party_queue_reorder", { id, position });

export const partyQueueClear = (): Promise<{ cleared: boolean }> => invoke("party_queue_clear");

export const partySkip = (): Promise<{ skipped: boolean }> => invoke("party_skip");

// ── Admin transport + audio controls ────────────────────────────────────────

export const partyPause = (): Promise<unknown> => invoke("party_control_pause");
export const partyResume = (): Promise<unknown> => invoke("party_control_resume");
export const partyRestart = (): Promise<unknown> => invoke("party_control_restart");
export const partySetGuideVocal = (value: number): Promise<unknown> =>
  invoke("party_set_guide_vocal", { value });
export const partySetVolume = (value: number): Promise<unknown> =>
  invoke("party_set_volume", { value });
export const partySetKey = (offset: number): Promise<unknown> =>
  invoke("party_set_key", { offset });
