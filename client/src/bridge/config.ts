import { AppConfig } from "@/types/AppConfig";
import { updateMicMirrorGain } from "./microphone";
import { invoke } from "./runtime";

export function getPreloadedConfig(): AppConfig | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }

  return window.__NIGHTINGALE_APP_CONFIG__;
}

export const loadConfig = async (): Promise<AppConfig> => {
  return await invoke<AppConfig>("load_config");
};

export const saveConfig = async (config: AppConfig): Promise<AppConfig> => {
  const saved = await invoke<AppConfig>("save_config", { config });

  if (saved.mic_mirror_gain != null) {
    updateMicMirrorGain(saved.mic_mirror_gain);
  }

  return saved;
};
