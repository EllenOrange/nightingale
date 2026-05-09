import {
  tauriMicrophoneAdapter,
  type MicReactiveEvent,
  type MicrophoneAdapter,
  type StopListening,
} from "@/adapters/microphone";
import { useEffect, useRef, type MutableRefObject } from "react";

const defaultAdapter = tauriMicrophoneAdapter;

export type MicReactiveRef = MutableRefObject<MicReactiveEvent | null>;

export function useMicReactive(
  enabled: boolean,
  adapter: MicrophoneAdapter = defaultAdapter,
): MicReactiveRef {
  const ref = useRef<MicReactiveEvent | null>(null);

  useEffect(() => {
    if (!enabled) {
      ref.current = null;
      return;
    }

    let cancelled = false;
    let stopListening: StopListening | null = null;

    const run = async () => {
      try {
        stopListening = await adapter.onReactive((event) => {
          if (!cancelled) ref.current = event;
        });
        if (cancelled) {
          stopListening();
        }
      } catch {
        ref.current = null;
      }
    };

    void run();

    return () => {
      cancelled = true;
      stopListening?.();
      ref.current = null;
    };
  }, [enabled, adapter]);

  return ref;
}
