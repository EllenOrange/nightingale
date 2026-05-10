import type { MicrophoneInfo } from "@/types/MicrophoneInfo";
import {
  type MicCaptureOptions,
  type MicSamplesCallback,
  listMicrophones as tauriListMicrophones,
  startMicCapture as tauriStartMicCapture,
  stopMicCapture as tauriStopMicCapture,
  subscribeMicSamples as tauriSubscribeMicSamples,
} from "@/tauri-bridge/microphone";

export type {
  MicCaptureOptions,
  MicSampleFrame,
  MicSamplesCallback,
} from "@/tauri-bridge/microphone";

export type StopListening = () => void;

export interface MicrophoneAdapter {
  listDevices(): Promise<MicrophoneInfo[]>;
  startCapture(preferred: string | null, options: MicCaptureOptions): Promise<string>;
  stopCapture(): Promise<void>;
  onSamples(cb: MicSamplesCallback): Promise<StopListening>;
}

export const tauriMicrophoneAdapter: MicrophoneAdapter = {
  listDevices: tauriListMicrophones,
  startCapture: tauriStartMicCapture,
  stopCapture: tauriStopMicCapture,
  onSamples: async (cb) => tauriSubscribeMicSamples(cb),
};
