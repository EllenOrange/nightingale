import type { MicCaptureOptions } from "@/types/MicCaptureOptions";
import type { MicrophoneInfo } from "@/types/MicrophoneInfo";
import type { MicSampleFrame } from "@/types/MicSampleFrame";
import { Channel, invoke } from "@tauri-apps/api/core";

export type { MicCaptureOptions };
export type { MicSampleFrame };

export type MicSamplesCallback = (frame: MicSampleFrame) => void;

let activeChannel: Channel<MicSampleFrame> | null = null;
const subscribers = new Set<MicSamplesCallback>();

const dispatch = (frame: MicSampleFrame): void => {
  for (const cb of subscribers) {
    try {
      cb(frame);
    } catch {
      // Subscribers must not break the dispatch loop.
    }
  }
};

const ensureChannel = (): Channel<MicSampleFrame> => {
  if (activeChannel) return activeChannel;
  const channel = new Channel<MicSampleFrame>();
  channel.onmessage = dispatch;
  activeChannel = channel;
  return channel;
};

export const listMicrophones = async (): Promise<MicrophoneInfo[]> => {
  return await invoke<MicrophoneInfo[]>("list_microphones");
};

export const startMicCapture = async (
  preferred: string | null,
  options: MicCaptureOptions,
): Promise<string> => {
  const channel = ensureChannel();
  return await invoke<string>("start_mic_capture", {
    preferred,
    options,
    onSamples: channel,
  });
};

export const stopMicCapture = async (): Promise<void> => {
  await invoke("stop_mic_capture");
};

export const subscribeMicSamples = (cb: MicSamplesCallback): (() => void) => {
  subscribers.add(cb);
  return () => {
    subscribers.delete(cb);
  };
};
