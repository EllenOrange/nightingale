import type { LrclibCandidate } from "@/types/LrclibCandidate";
import type { LyricsFile } from "@/types/LyricsFile";
import { invoke } from "./runtime";

export const loadLyrics = async (fileHash: string): Promise<LyricsFile | null> => {
  return await invoke<LyricsFile | null>("load_lyrics", { fileHash });
};

export const searchLrclibLyrics = async (fileHash: string): Promise<LrclibCandidate[]> => {
  return await invoke<LrclibCandidate[]>("search_lrclib_lyrics", { fileHash });
};

export const saveLyrics = async (fileHash: string, lines: string[]): Promise<void> => {
  return await invoke<void>("save_lyrics", { fileHash, lines });
};
