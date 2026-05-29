import type { AppConfig } from "@/types/AppConfig";

export type SettingsTab = "general" | "analysis";
export type SettingsOption = { value: string; label: string };

export const SETTINGS_TABS: { value: SettingsTab; label: string }[] = [
  { value: "general", label: "General" },
  { value: "analysis", label: "Analysis" },
];

export const SEPARATORS: SettingsOption[] = [
  { value: "karaoke", label: "UVR Karaoke" },
  { value: "demucs", label: "Demucs" },
];

export const ASR_ENGINES: SettingsOption[] = [
  { value: "whisper", label: "Whisper" },
  { value: "parakeet", label: "Parakeet v3 (Experimental)" },
];

export const MODELS = ["large-v3", "large-v3-turbo", "medium", "small", "base", "tiny"];

export const DEFAULTS = {
  separator: "karaoke",
  asr_engine: "whisper",
  whisper_model: "large-v3",
  beam_size: 8,
  batch_size: 8,
  mic_monitor_gain: 0.65,
} satisfies Pick<
  AppConfig,
  "separator" | "asr_engine" | "whisper_model" | "beam_size" | "batch_size" | "mic_monitor_gain"
>;

export const MIC_MONITOR_GAIN_STEP = 0.01;
export const MIC_MONITOR_GAIN_MAX = 2;
export const NUMBER_PICKER_SIZE = 16;

export const NAV = {
  tabSegment: 0,
  general: {
    window: 1,
    microphone: 2,
    micMonitorGain: 3,
  },
  analysis: {
    separator: 1,
    asrEngine: 2,
    whisperModel: 3,
    beamSize: 4,
    parakeetBatchSize: 3,
    whisperBatchSize: 5,
  },
} as const;

export function getSettingsStops(tab: SettingsTab, isParakeet: boolean) {
  if (tab === "general") {
    return [2, 2, 1, 1, 2];
  }

  return isParakeet
    ? [2, 1, 1, NUMBER_PICKER_SIZE, 2]
    : [2, 1, 1, 1, NUMBER_PICKER_SIZE, NUMBER_PICKER_SIZE, 2];
}

export function getBatchSizeSegment(isParakeet: boolean) {
  return isParakeet ? NAV.analysis.parakeetBatchSize : NAV.analysis.whisperBatchSize;
}
