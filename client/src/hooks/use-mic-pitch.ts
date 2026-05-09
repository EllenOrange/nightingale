import {
  tauriMicrophoneAdapter,
  type MicCaptureOptions,
  type MicrophoneAdapter,
  type StopListening,
} from "@/adapters/microphone";
import type { MicrophoneInfo } from "@/types/MicrophoneInfo";
import { useCallback, useEffect, useRef, useState } from "react";

export interface MicDevice {
  deviceId: string;
  label: string;
}

const defaultAdapter = tauriMicrophoneAdapter;

export function useMicDevices(adapter: MicrophoneAdapter = defaultAdapter) {
  const [devices, setDevices] = useState<MicDevice[]>([]);

  const refresh = useCallback(async () => {
    try {
      const mics = await adapter.listDevices();
      setDevices(
        mics.map((m: MicrophoneInfo) => ({
          deviceId: m.name,
          label: m.name,
        })),
      );
    } catch {
      setDevices([]);
    }
  }, [adapter]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return devices;
}

export function useMicPitch(enabled: boolean, adapter: MicrophoneAdapter = defaultAdapter) {
  const [latestPitch, setLatestPitch] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [active, setActive] = useState(false);

  useEffect(() => {
    if (!enabled) {
      setLatestPitch(null);
      setError(null);
      setActive(false);
      return;
    }

    let cancelled = false;
    let stopListening: StopListening | null = null;

    const run = async () => {
      try {
        stopListening = await adapter.onPitch((pitch) => {
          if (!cancelled) setLatestPitch(pitch);
        });
        if (cancelled) {
          stopListening();
          return;
        }
        setError(null);
        setActive(true);
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : String(e));
          setLatestPitch(null);
          setActive(false);
        }
      }
    };

    void run();

    return () => {
      cancelled = true;
      stopListening?.();
      setLatestPitch(null);
      setActive(false);
    };
  }, [enabled, adapter]);

  return { latestPitch, active, error };
}

export function useMicCapture(
  deviceId: string | null,
  options: MicCaptureOptions,
  adapter: MicrophoneAdapter = defaultAdapter,
) {
  const [active, setActive] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const startedRef = useRef(false);
  const enabled = options.emit_pitch || options.emit_audio || options.emit_reactive;

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
  }, [enabled, options.emit_pitch, options.emit_audio, options.emit_reactive, deviceId, adapter]);

  return { active, error };
}
