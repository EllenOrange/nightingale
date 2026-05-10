import { check, type Update, type DownloadEvent } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

const CHECK_TIMEOUT_MS = 30_000;

export const checkForUpdate = async (): Promise<Update | null> => {
  return await check({ timeout: CHECK_TIMEOUT_MS });
};

export const downloadAndInstallUpdate = async (
  update: Update,
  onProgress: (event: DownloadEvent) => void,
): Promise<void> => {
  await update.downloadAndInstall(onProgress);
};

export const relaunchApp = async (): Promise<void> => {
  await relaunch();
};
