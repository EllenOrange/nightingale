import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";

export const selectFolderRaw = async (): Promise<string | undefined> => {
  const folder = await open({
    directory: true,
    multiple: false,
  });

  if (!folder) {
    toast.error("Folder was not selected! Please try again.");

    return;
  }

  return folder;
};

export const selectFolder = async (): Promise<void> => {
  const folder = await selectFolderRaw();

  if (!folder) {
    return;
  }

  triggerScan(folder);
};

export const triggerScan = async (folder: string): Promise<void> => {
  await invoke("trigger_scan", { folder });
};
