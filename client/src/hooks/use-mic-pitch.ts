import {
  microphoneAdapter,
  type MicCaptureOptions,
  type MicrophoneAdapter,
} from "@/bridge/microphone";
import { useMicSamples } from "@/hooks/use-mic-samples";
import { SampleRing } from "@/lib/mic/sample-ring";
import { PITCH_WINDOW_SAMPLES } from "@/lib/pitch/constants";
import { createMicPitchDetector, detectPitchFromSamplesMic } from "@/lib/pitch/detect";
import { useEffect, useRef, useState } from "react";

/** ~30 Hz pitch updates: more than enough for vocal pitch tracking. */
const PITCH_TICK_MS = 33;
const RING_CAPACITY = PITCH_WINDOW_SAMPLES * 2;

const defaultAdapter = microphoneAdapter;

export function useMicPitch(enabled: boolean) {
  const [latestPitch, setLatestPitch] = useState<number | null>(null);
  const [active, setActive] = useState(false);
  const ringRef = useRef<SampleRing | null>(null);
  const sampleRateRef = useRef(0);

  if (ringRef.current === null) {
    ringRef.current = new SampleRing(RING_CAPACITY);
  }

  useMicSamples((frame) => {
    sampleRateRef.current = frame.sample_rate;
    ringRef.current?.push(frame.samples);
  }, enabled);

  useEffect(() => {
    if (!enabled) {
      setLatestPitch(null);
      setActive(false);
      ringRef.current?.reset();
      sampleRateRef.current = 0;
      return;
    }

    setActive(true);
    const detector = createMicPitchDetector();
    const window = new Float32Array(PITCH_WINDOW_SAMPLES);

    const tick = () => {
      const ring = ringRef.current;
      const sr = sampleRateRef.current;
      if (!ring || sr === 0) return;
      if (!ring.readMostRecent(window)) return;
      const hz = detectPitchFromSamplesMic(detector, window, sr);
      setLatestPitch(hz);
    };

    const id = setInterval(tick, PITCH_TICK_MS);

    return () => {
      clearInterval(id);
      setLatestPitch(null);
      setActive(false);
    };
  }, [enabled]);

  return { latestPitch, active, error: null as string | null };
}

export function useMicCapture(
  deviceId: string | null,
  enabled: boolean,
  options: MicCaptureOptions,
  adapter: MicrophoneAdapter = defaultAdapter,
) {
  const [active, setActive] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const startedRef = useRef(false);

  useEffect(() => {
    if (!enabled) {
      if (startedRef.current) {
        adapter.stopCapture().catch(() => {});
        startedRef.current = false;
      }
      setError(null);
      setActive(false);
      return;
    }

    let cancelled = false;

    const run = async () => {
      try {
        await adapter.startCapture(deviceId, options);

        if (cancelled) {
          await adapter.stopCapture().catch(() => {});
          return;
        }

        startedRef.current = true;
        setActive(true);
        setError(null);
      } catch (e) {
        void adapter.stopCapture().catch(() => {});
        if (!cancelled) {
          const msg = e instanceof Error ? e.message : String(e);
          setError(msg);
          setActive(false);
        }
      }
    };

    void run();

    return () => {
      cancelled = true;
      if (startedRef.current) {
        adapter.stopCapture().catch(() => {});
        startedRef.current = false;
      }
      setActive(false);
    };
  }, [enabled, options.emit_audio, deviceId, adapter]);

  return { active, error };
}
