import { invoke, listen } from "./runtime";
import type { SetupProgress } from "@/types/SetupProgress";

export const isAppReady = async (): Promise<boolean> => {
  return await invoke<boolean>("is_ready");
};

export const triggerSetup = async (dataFolder?: string): Promise<void> => {
  return await invoke<void>("trigger_setup", { dataPath: dataFolder });
};

export const onSetupProgress = async (
  cb: (progress: SetupProgress) => void,
): Promise<() => void> => {
  return await listen<SetupProgress>("setup-progress", ({ payload }) => cb(payload));
};

export const onSetupError = async (cb: (error: string) => void): Promise<() => void> => {
  return await listen<string>("setup-error", ({ payload }) => cb(payload));
};
