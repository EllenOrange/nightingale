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

// A lyric-available track from LRCLIB (snake_case, matching the Rust struct).
export interface LrclibSearchResult {
  track_name: string;
  artist_name: string;
  album_name: string;
  duration_secs: number;
  has_synced: boolean;
  has_plain: boolean;
}

// A YouTube search hit (camelCase, matching the Rust serde rename).
export interface YoutubeCandidate {
  videoId: string;
  url: string;
  title: string;
  channel: string;
  durationSecs: number | null;
  thumbnail: string;
}
