/**
 * Owns everything mic-shaped during playback: device selection, pitch capture,
 * monitor toggle, reactive shader uniforms, and the pitch-scoring series/score.
 *
 * Reads playback state (isReady, isPlaying, paused) from the transport context
 * to gate hardware capture, and persists user toggles to the app config.
 */

import { useMicCapture, useMicPitch } from "@/hooks/use-mic-pitch";
import { useMicDevices } from "@/queries/use-mic-devices";
import { useMicReactive, type MicReactiveRef } from "@/hooks/use-mic-reactive";
import { usePitchScoring } from "@/hooks/use-pitch-scoring";
import { usePlaybackConfigPersist } from "@/hooks/playback/use-playback-config-persist";
import { DEFAULT_MIC_LATENCY_COMPENSATION_SEC } from "@/lib/pitch/constants";
import type { PitchSeries } from "@/lib/pitch/state";
import type { AppConfig } from "@/types/AppConfig";
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { toast } from "sonner";
import {
  usePlaybackTransportActions,
  usePlaybackTransportState,
} from "./playback-transport-context";

export interface PlaybackMicState {
  micUserEnabled: boolean;
  micMonitorUserEnabled: boolean;
  selectedMicId: string | null;
  micName: string;
  pitchScore: number | null;
  rawScore: number;
  series: PitchSeries;
  micCaptureActive: boolean;
  micPitchActive: boolean;
  micReady: boolean;
}

export interface PlaybackMicActions {
  reactiveRef: MicReactiveRef;
  handleToggleMic: () => void;
  handleCycleMic: () => void;
  handleToggleMicMonitor: () => void;
}

const MicStateContext = createContext<PlaybackMicState | null>(null);
const MicActionsContext = createContext<PlaybackMicActions | null>(null);

interface PlaybackMicProviderProps {
  config: AppConfig | null;
  children: ReactNode;
}

export function PlaybackMicProvider({ config, children }: PlaybackMicProviderProps) {
  const { isReady, isPlaying, paused, duration } = usePlaybackTransportState();
  const { subscribe, getVocalsBuffer } = usePlaybackTransportActions();

  const persistConfig = usePlaybackConfigPersist(config);

  const [micUserEnabled, setMicUserEnabled] = useState(config?.mic_active ?? true);
  const [micMonitorUserEnabled, setMicMonitorUserEnabled] = useState(
    config?.mic_monitoring ?? false,
  );
  const [selectedMicId, setSelectedMicId] = useState<string | null>(config?.preferred_mic ?? null);

  const micDevices = useMicDevices();

  const micPitchEnabled = isReady && isPlaying && !paused && micUserEnabled;
  const micMonitorEnabled = isReady && isPlaying && !paused && micMonitorUserEnabled;
  const captureEnabled = micPitchEnabled || micMonitorEnabled;

  const captureOptions = useMemo(() => ({ emit_audio: micMonitorEnabled }), [micMonitorEnabled]);

  const { active: micCaptureActive, error: micCaptureError } = useMicCapture(
    selectedMicId,
    captureEnabled,
    captureOptions,
  );
  const {
    latestPitch,
    active: micPitchActive,
    error: micPitchError,
  } = useMicPitch(micPitchEnabled);
  const reactiveRef = useMicReactive(micPitchEnabled);

  const { series, score } = usePitchScoring(
    { isReady, duration, getVocalsBuffer, subscribe },
    latestPitch,
    config?.mic_latency_compensation_sec ?? DEFAULT_MIC_LATENCY_COMPENSATION_SEC,
  );

  const micErrorShown = useRef(false);
  useEffect(() => {
    const micError = micCaptureError ?? micPitchError;
    if (micError && !micErrorShown.current) {
      micErrorShown.current = true;
      toast.error(`Microphone: ${micError}`);
    }
    if (!micError) {
      micErrorShown.current = false;
    }
  }, [micCaptureError, micPitchError]);

  const handleToggleMic = useCallback(() => {
    setMicUserEnabled((prev) => {
      const next = !prev;
      if (!next && micMonitorUserEnabled) {
        setMicMonitorUserEnabled(false);
        persistConfig({ mic_active: false, mic_monitoring: false });
      } else {
        persistConfig({ mic_active: next });
      }
      return next;
    });
  }, [persistConfig, micMonitorUserEnabled]);

  const handleCycleMic = useCallback(() => {
    if (micDevices.length <= 1) return;
    const currentIdx = micDevices.findIndex((d) => d.deviceId === selectedMicId);
    const nextIdx = (currentIdx + 1) % micDevices.length;
    const next = micDevices[nextIdx];
    setSelectedMicId(next.deviceId);
    persistConfig({ preferred_mic: next.deviceId });
  }, [micDevices, selectedMicId, persistConfig]);

  const handleToggleMicMonitor = useCallback(() => {
    setMicMonitorUserEnabled((prev) => {
      const next = !prev;
      persistConfig({ mic_monitoring: next });
      if (next && !micUserEnabled) {
        setMicUserEnabled(true);
        persistConfig({ mic_active: true });
      }
      return next;
    });
  }, [persistConfig, micUserEnabled]);

  const stateValue = useMemo<PlaybackMicState>(() => {
    const micReady = micCaptureActive && micPitchActive && micUserEnabled;
    return {
      micUserEnabled,
      micMonitorUserEnabled,
      selectedMicId,
      micName: selectedMicId ?? "Default",
      pitchScore: micReady ? score : null,
      rawScore: score,
      series,
      micCaptureActive,
      micPitchActive,
      micReady,
    };
  }, [
    micUserEnabled,
    micMonitorUserEnabled,
    selectedMicId,
    score,
    series,
    micCaptureActive,
    micPitchActive,
  ]);

  const actionsValue = useMemo<PlaybackMicActions>(
    () => ({
      reactiveRef,
      handleToggleMic,
      handleCycleMic,
      handleToggleMicMonitor,
    }),
    [reactiveRef, handleToggleMic, handleCycleMic, handleToggleMicMonitor],
  );

  return (
    <MicStateContext.Provider value={stateValue}>
      <MicActionsContext.Provider value={actionsValue}>{children}</MicActionsContext.Provider>
    </MicStateContext.Provider>
  );
}

export function usePlaybackMicState(): PlaybackMicState {
  const ctx = useContext(MicStateContext);
  if (!ctx) {
    throw new Error("usePlaybackMicState must be used within a PlaybackMicProvider");
  }
  return ctx;
}

export function usePlaybackMicActions(): PlaybackMicActions {
  const ctx = useContext(MicActionsContext);
  if (!ctx) {
    throw new Error("usePlaybackMicActions must be used within a PlaybackMicProvider");
  }
  return ctx;
}
