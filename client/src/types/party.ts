// Party-layer types. Hand-written to mirror the Rust serde shapes in
// client/src-server/src/party/queue.rs (camelCase on the wire).

export type QueueStatus =
  | "queued"
  | "downloading"
  | "analyzing"
  | "ready"
  | "playing"
  | "done"
  | "error";

export interface QueueEntry {
  id: string;
  query: string | null;
  fileHash: string | null;
  title: string;
  artist: string;
  requestedBy: string;
  status: QueueStatus;
  addedAt: number;
  error: string | null;
}

export interface PartyQueue {
  entries: QueueEntry[];
}
