import { Song } from "@/types/Song";
import { Stepper } from "./stepper";
import { onShiftKeyDone, onShiftTempoDone, shiftKey, shiftTempo } from "@/bridge/analysis";
import { useEffect, useRef } from "react";
import { calculateKeyShift } from "@/utils/shift-key";

export type ShiftType = "tempo" | "key";

interface Props {
  song: Song;
  status: Record<ShiftType, boolean>;
  onStart: (shiftType: ShiftType) => void;
  onSuccess: (message: string, shiftType: ShiftType) => void;
  onError: (message: string, shiftType: ShiftType) => void;
}

export const Shifts = ({ song, status, onSuccess, onError, onStart }: Props) => {
  const onSuccessRef = useRef(onSuccess);
  const onErrorRef = useRef(onError);
  onSuccessRef.current = onSuccess;
  onErrorRef.current = onError;

  const listeners: Record<
    ShiftType,
    {
      register: typeof onShiftKeyDone;
      successMessage: string;
      errorMessage: (error: string) => string;
    }
  > = {
    key: {
      register: onShiftKeyDone,
      successMessage: "Song key shifted successfully",
      errorMessage: (error) => `Error while shifting the key: ${error}`,
    },
    tempo: {
      register: onShiftTempoDone,
      successMessage: "Song tempo shifted successfully",
      errorMessage: (error) => `Error while shifting the tempo: ${error}`,
    },
  };

  const withOnStart = (callback: () => void, shiftType: ShiftType) => () => {
    onStart(shiftType);

    try {
      callback();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);

      onErrorRef.current(`Error while shifting the tempo: ${message}`, shiftType);
    }
  };

  useEffect(() => {
    let cancelled = false;
    const unlisteners: Partial<Record<ShiftType, () => void>> = {};

    (Object.entries(listeners) as Array<[ShiftType, (typeof listeners)[ShiftType]]>).forEach(
      ([shiftType, config]) => {
        config
          .register(({ file_hash, error }) => {
            if (file_hash !== song.file_hash) {
              return;
            }

            if (!error) {
              onSuccessRef.current(config.successMessage, shiftType);
            } else {
              onErrorRef.current(config.errorMessage(error), shiftType);
            }
          })
          .then((fn) => {
            if (cancelled) {
              fn();

              return;
            }

            unlisteners[shiftType] = fn;
          });
      },
    );

    return () => {
      cancelled = true;

      unlisteners.tempo?.();
      unlisteners.key?.();
    };
  }, [song.file_hash]);

  if (!song.is_analyzed || song.transcript_source === "Usdx") {
    return null;
  }

  const loading = status.key || status.tempo;

  const onShiftKey = (direction: "up" | "down") => {
    if (!song.key) {
      return;
    }

    const { key, keyOffset, pitchRatio } = calculateKeyShift(
      song.key,
      song.key_offset + (direction === "up" ? 1 : -1),
    );

    shiftKey(song.file_hash, key, pitchRatio, keyOffset);
  };

  const stepperConfigs = [
    {
      shiftType: "tempo" as const,
      label: song.tempo.toFixed(1),
      tooltip: "Click +/- to shift the song tempo",
      onPlus: () => shiftTempo(song.file_hash, song.tempo + 0.1),
      onMinus: () => shiftTempo(song.file_hash, song.tempo - 0.1),
      disabled: {
        plus: song.tempo >= 2,
        minus: song.tempo <= 0.5,
      },
    },
    {
      shiftType: "key" as const,
      label: song.override_key ?? song.key,
      tooltip: `Click +/- to shift the song key. ${song.key ? `Default key is ${song.key}` : "Reanalyze the song to identify the key"}`,
      onPlus: () => onShiftKey("up"),
      onMinus: () => onShiftKey("down"),
      disabled: {
        plus: song.key_offset >= 5 || !song.key,
        minus: song.key_offset <= -5 || !song.key,
      },
    },
  ];

  return (
    <div className="flex shrink-0 gap-1">
      {stepperConfigs.map(({ shiftType, label, tooltip, onPlus, onMinus, disabled }) => (
        <Stepper
          key={shiftType}
          loading={status[shiftType]}
          label={label}
          tooltip={tooltip}
          onClick={{
            plus: withOnStart(onPlus, shiftType),
            minus: withOnStart(onMinus, shiftType),
          }}
          disabled={{
            plus: loading || disabled.plus,
            minus: loading || disabled.minus,
          }}
        />
      ))}
    </div>
  );
};
