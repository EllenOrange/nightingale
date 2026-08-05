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

export const partySkip = (): Promise<{ skipped: boolean }> => invoke("party_skip");
