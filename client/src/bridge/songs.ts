import type { AnalysisQueue } from "@/types/AnalysisQueue";
import type { LoadSongsParams } from "@/types/LoadSongsParams";
import type { SongsMeta } from "@/types/SongsMeta";
import type { SongsStore } from "@/types/SongsStore";
import { invoke } from "./runtime";

export function getPreloadedSongsMeta(): SongsMeta | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }
  return window.__NIGHTINGALE_SONGS_META__;
}

export const loadSongs = async (params: LoadSongsParams): Promise<SongsStore> => {
  return await invoke<SongsStore>("load_songs", { params });
};

export const loadSongsMeta = async (): Promise<SongsMeta> => {
  return await invoke<SongsMeta>("load_songs_meta");
};

export const loadAnalysisQueue = async (): Promise<AnalysisQueue> => {
  return await invoke<AnalysisQueue>("load_analysis_queue");
};
