import type { AudioPaths } from "@/types/Transcript";
import {
  ensureMp3Stems as tauriEnsureMp3Stems,
  ensurePlayableSourceVideo as tauriEnsurePlayableSourceVideo,
  fetchPixabayVideos as tauriFetchPixabayVideos,
  getAudioPaths as tauriRawGetAudioPaths,
  getMediaPort as tauriGetMediaPort,
  loadTranscript as tauriLoadTranscript,
  onPixabayVideoDownloaded as tauriOnPixabayVideoDownloaded,
  onStemsReady as tauriOnStemsReady,
} from "./playback.tauri";
import { isTauri } from "./runtime";

export type { PixabayVideoDownloaded, StemsReadyEvent } from "./playback.tauri";

export interface PlaybackAdapter {
  /**
   * Performs any one-time async setup (e.g. fetching the Tauri media port).
   * Must be awaited before `toMediaUrl` is called synchronously.
   */
  init(): Promise<void>;
  /** Build a fetchable URL for an absolute filesystem path returned by core. */
  toMediaUrl(absolutePath: string): string;
  /** Stem URLs for a song, already encoded for the active transport. */
  getAudioPaths(fileHash: string): Promise<AudioPaths>;
}

// ─── Tauri implementation ─────────────────────────────────────────────────

let cachedTauriPort: number | null = null;

const tauriToMediaUrl = (absolutePath: string): string => {
  if (cachedTauriPort === null) {
    throw new Error("playbackAdapter.init() must be awaited before toMediaUrl");
  }
  return `http://127.0.0.1:${cachedTauriPort}/${encodeURIComponent(absolutePath)}`;
};

export const tauriPlaybackAdapter: PlaybackAdapter = {
  async init() {
    if (cachedTauriPort === null) {
      cachedTauriPort = await tauriGetMediaPort();
    }
  },
  toMediaUrl: tauriToMediaUrl,
  async getAudioPaths(fileHash) {
    if (cachedTauriPort === null) {
      cachedTauriPort = await tauriGetMediaPort();
    }
    const paths = await tauriRawGetAudioPaths(fileHash);
    return {
      instrumental: tauriToMediaUrl(paths.instrumental),
      vocals: tauriToMediaUrl(paths.vocals),
    };
  },
};

// ─── Web implementation ───────────────────────────────────────────────────

/**
 * Wraps a server-side absolute path in the asset proxy so the browser can
 * fetch it through the same origin without leaking filesystem layout. The
 * server canonicalises and rejects anything outside the data dir.
 */
const webToMediaUrl = (absolutePath: string): string =>
  `/api/asset?path=${encodeURIComponent(absolutePath)}`;

export const webPlaybackAdapter: PlaybackAdapter = {
  async init() {
    // Nothing to do; the server lives at the page origin.
  },
  toMediaUrl: webToMediaUrl,
  async getAudioPaths(fileHash) {
    // Resolve the server-side path so the URL encodes the active key/tempo
    // variant. A stable `/media/<hash>/<kind>` route would let the browser
    // cache the response and replay the original stems after a shift.
    const paths = await tauriRawGetAudioPaths(fileHash);
    return {
      instrumental: webToMediaUrl(paths.instrumental),
      vocals: webToMediaUrl(paths.vocals),
    };
  },
};

export const playbackAdapter: PlaybackAdapter = isTauri ? tauriPlaybackAdapter : webPlaybackAdapter;

// ─── Re-exports for command/event call sites ─────────────────────────────

export const loadTranscript = tauriLoadTranscript;
export const ensureMp3Stems = tauriEnsureMp3Stems;
export const ensurePlayableSourceVideo = tauriEnsurePlayableSourceVideo;
export const fetchPixabayVideos = tauriFetchPixabayVideos;
export const onStemsReady = tauriOnStemsReady;
export const onPixabayVideoDownloaded = tauriOnPixabayVideoDownloaded;
