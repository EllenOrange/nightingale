import type { DialogMode } from "@/hooks/use-dialog";
import type { Song } from "@/types/Song";
import type { Transcript } from "@/types/Transcript";

export type EditLyricsDialogMode = { mode: "edit-lyrics"; song: Song };

export function isEditLyricsDialogMode(mode: DialogMode): mode is EditLyricsDialogMode {
  return mode !== null && typeof mode === "object" && mode.mode === "edit-lyrics";
}

export function formatSeconds(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "?";
  const minutes = Math.floor(seconds / 60);
  const secs = Math.floor(seconds) % 60;
  return `${minutes}:${secs.toString().padStart(2, "0")}`;
}

export function normalizeLines(text: string): string[] {
  return text
    .split("\n")
    .map((l) => l.trim())
    .filter((l) => l.length > 0);
}

export function linesFromTranscript(transcript: Transcript): string {
  return transcript.segments
    .map((s) => s.text.trim())
    .filter((s) => s.length > 0)
    .join("\n");
}
