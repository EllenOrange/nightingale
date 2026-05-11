import { platform } from "@tauri-apps/plugin-os";

export const UPDATES_SUPPORTED: boolean = platform() !== "linux";
