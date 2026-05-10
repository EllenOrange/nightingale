import type { MicCaptureOptions } from "@/types/MicCaptureOptions";
import type { MicrophoneInfo } from "@/types/MicrophoneInfo";
import type { MicSampleFrame } from "@/types/MicSampleFrame";
import { Channel, invoke } from "@tauri-apps/api/core";

export type { MicCaptureOptions };
export type { MicSampleFrame };

export type MicSamplesCallback = (frame: MicSampleFrame) => void;

const subscribers = new Set<MicSamplesCallback>();

/**
 * Serializes start/stop so React's stop-then-start on song change can't race
 * at the Tauri IPC layer (commands run on a worker pool and would otherwise
 * be free to reorder).
 */
let opChain: Promise<unknown> = Promise.resolve();

const dispatch = (frame: MicSampleFrame): void => {
  for (const cb of subscribers) {
    try {
      cb(frame);
    } catch {
      // Subscribers must not break the dispatch loop.
    }
  }
};

const enqueue = <T>(op: () => Promise<T>): Promise<T> => {
  const next = opChain.catch(() => undefined).then(op);
  opChain = next;
  return next;
};

export const listMicrophones = async (): Promise<MicrophoneInfo[]> => {
  return await invoke<MicrophoneInfo[]>("list_microphones");
};

export const startMicCapture = (
  preferred: string | null,
  options: MicCaptureOptions,
): Promise<string> =>
  enqueue(async () => {
    /**
     * Always allocate a fresh Channel: when Rust drops the previous one in
     * `stop_mic_capture` it sends an `end` message that unregisters the JS
     * callback id. Reusing the cached Channel would hand Rust a dead id and
     * spam "Couldn't find callback id ..." for every frame.
     */
    const channel = new Channel<MicSampleFrame>();
    channel.onmessage = dispatch;
    return await invoke<string>("start_mic_capture", {
      preferred,
      options,
      onSamples: channel,
    });
  });

export const stopMicCapture = (): Promise<void> =>
  enqueue(async () => {
    await invoke("stop_mic_capture");
  });

export const subscribeMicSamples = (cb: MicSamplesCallback): (() => void) => {
  subscribers.add(cb);
  return () => {
    subscribers.delete(cb);
  };
};
