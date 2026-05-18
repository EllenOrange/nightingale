import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { invoke, isTauri } from "./runtime";

const promptForServerPath = (): string | null => {
  // Browsers don't expose absolute filesystem paths via file pickers, so the
  // server-hosted build asks the user to type a path the server can see.
  const input = window.prompt("Songs folder path (visible to the server)", "/songs");

  if (!input) return null;

  const trimmed = input.trim();

  return trimmed.length > 0 ? trimmed : null;
};

export const selectFolderRaw = async (): Promise<string | undefined> => {
  if (!isTauri) {
    const folder = promptForServerPath();

    if (!folder) {
      toast.error("Folder was not selected! Please try again.");
      return;
    }

    return folder;
  }

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
