import type { MicrophoneInfo } from "@/types/MicrophoneInfo";
import type { MicCaptureOptions } from "@/types/MicCaptureOptions";
import type { MicReactiveEvent } from "@/types/MicReactiveEvent";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type { MicCaptureOptions };
export type { MicReactiveEvent };

export const listMicrophones = async (): Promise<MicrophoneInfo[]> => {
  return await invoke<MicrophoneInfo[]>("list_microphones");
};

export const startMicCapture = async (
  preferred: string | null,
  options: MicCaptureOptions,
): Promise<string> => {
  return await invoke<string>("start_mic_capture", {
    preferred,
    options,
  });
};

export const stopMicCapture = async (): Promise<void> => {
  await invoke("stop_mic_capture");
};

export const onMicPitch = async (cb: (pitch: number | null) => void): Promise<UnlistenFn> => {
  return await listen<{ pitch: number | null }>("mic-pitch", (event) => {
    cb(event.payload.pitch);
  });
};

export const onMicReactive = async (cb: (event: MicReactiveEvent) => void): Promise<UnlistenFn> => {
  return await listen<MicReactiveEvent>("mic-reactive", (event) => {
    cb(event.payload);
  });
};
