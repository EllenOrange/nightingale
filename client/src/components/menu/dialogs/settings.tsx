import { Button } from "@/components/ui/button";
import { ButtonGroup } from "@/components/ui/button-group";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field, FieldDescription, FieldGroup } from "@/components/ui/field";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Slider } from "@/components/ui/slider";
import { useEffect, useRef, useState } from "react";
import { useDialogNav } from "@/hooks/navigation/use-dialog-nav";
import { setFullScreen, isFullScreen as tauriIsFullScreen } from "@/tauri-bridge/fullScreen";
import { useDialog } from "@/hooks/use-dialog";
import { useConfig } from "@/queries/use-config";
import { useConfigMutation } from "@/mutations/use-config-mutation";
import { useMicDevices } from "@/hooks/use-mic-pitch";
import { cn } from "@/lib/utils";

const SEPARATORS = [
  { value: "karaoke", label: "UVR Karaoke" },
  { value: "demucs", label: "Demucs" },
];

const ASR_ENGINES = [
  { value: "whisper", label: "Whisper" },
  { value: "parakeet", label: "Parakeet v3 (Experimental)" },
];

const MODELS = ["large-v3", "large-v3-turbo", "medium", "small", "base", "tiny"];

const DEFAULT_MODEL: (typeof MODELS)[number] = "large-v3";
const DEFAULT_SEPARATOR = "karaoke";
const DEFAULT_ASR_ENGINE = "whisper";

const DEFAULT_BEAM_BATCH_SIZE = 8;
const DEFAULT_MIC_MIRROR_GAIN = 0.65;
const MIC_MIRROR_GAIN_STEP = 0.01;
const MIC_MIRROR_GAIN_MAX = 2;

const MIC_MIRROR_GAIN_SEGMENT = 2;

const SETTINGS_STOPS_WHISPER = [2, 1, 1, 1, 1, 1, 16, 16, 2];
const SETTINGS_STOPS_PARAKEET = [2, 1, 1, 1, 1, 16, 2];

const RING = "ring-2 ring-primary";
const NO_FOCUS_RING = "focus-visible:ring-0 focus-visible:border-transparent";

export const SettingsDialog = () => {
  const micDevices = useMicDevices();
  const { mode, close } = useDialog();
  const { data: config } = useConfig();
  const { mutate } = useConfigMutation();

  const containerRef = useRef<HTMLDivElement>(null);
  const [isFullScreen, setIsFullScreen] = useState<boolean | null | undefined>(config?.fullscreen);

  const open = mode === "settings";

  const micMirrorGainRef = useRef(config?.mic_mirror_gain ?? DEFAULT_MIC_MIRROR_GAIN);
  useEffect(() => {
    micMirrorGainRef.current = config?.mic_mirror_gain ?? DEFAULT_MIC_MIRROR_GAIN;
  }, [config?.mic_mirror_gain]);

  const asrEngine = config?.asr_engine ?? DEFAULT_ASR_ENGINE;
  const isParakeet = asrEngine === "parakeet";

  const stops = isParakeet ? SETTINGS_STOPS_PARAKEET : SETTINGS_STOPS_WHISPER;
  const itemCount = stops.reduce((sum, n) => sum + n, 0);
  const footerSegment = stops.length - 1;

  const { isFocused } = useDialogNav({
    open,
    itemCount,
    stops,
    onBack: close,
    containerRef,
    onAction: (segment, _slot, action) => {
      if (segment !== MIC_MIRROR_GAIN_SEGMENT) return false;
      if (!action.left && !action.right) return false;
      const delta = action.right ? MIC_MIRROR_GAIN_STEP : -MIC_MIRROR_GAIN_STEP;
      const next = Math.min(MIC_MIRROR_GAIN_MAX, Math.max(0, micMirrorGainRef.current + delta));
      micMirrorGainRef.current = next;
      mutate({ mic_mirror_gain: next });
      return true;
    },
  });

  useEffect(() => {
    const updateIsFullScreen = async () => {
      setIsFullScreen(await tauriIsFullScreen());
    };

    updateIsFullScreen();
  }, []);

  const toggleWindowMode = (fullscreen: boolean) => {
    setIsFullScreen(fullscreen);
    setFullScreen(fullscreen);
    mutate({ fullscreen });
  };

  const generateRingClassName = (segment: number, slot?: number) => {
    return cn(NO_FOCUS_RING, isFocused(segment, slot) && RING);
  };

  const generateNumberSelect = (
    settingName: "beam_size" | "batch_size",
    value: number,
    segment: number,
  ) => {
    return Array.from({ length: 16 })
      .fill(null)
      .map((_, idx) => {
        const idxToRender = idx + 1;

        return (
          <Button
            onClick={() => mutate({ [settingName]: idxToRender })}
            variant={value === idxToRender ? "default" : "outline"}
            className={generateRingClassName(segment, idx)}
          >
            {idx + 1}
          </Button>
        );
      });
  };

  const batchSegment = isParakeet ? 5 : 7;

  const batchSize = config?.batch_size ?? DEFAULT_BEAM_BATCH_SIZE;
  const beamSize = config?.beam_size ?? DEFAULT_BEAM_BATCH_SIZE;
  const micMirrorGainPct = Math.round((config?.mic_mirror_gain ?? DEFAULT_MIC_MIRROR_GAIN) * 100);

  return (
    <Dialog open={open} onOpenChange={close}>
      <DialogContent className="sm:max-w-lg max-h-[90vh] overflow-y-auto scrollbar-hide">
        <div ref={containerRef} className="contents">
          <DialogHeader>
            <DialogTitle>Settings</DialogTitle>
            <DialogDescription>
              You can modify the preferred model to use for the stem separation and transcript and
              tweak model parameters
            </DialogDescription>
          </DialogHeader>
          <FieldGroup>
            <Field>
              <Label>Window</Label>
              <ButtonGroup>
                <Button
                  variant={isFullScreen === true ? "outline" : "default"}
                  onClick={() => toggleWindowMode(false)}
                  className={generateRingClassName(0, 0)}
                >
                  Windowed
                </Button>
                <Button
                  variant={isFullScreen === false ? "outline" : "default"}
                  onClick={() => toggleWindowMode(true)}
                  className={generateRingClassName(0, 1)}
                >
                  Fullscreen
                </Button>
              </ButtonGroup>
            </Field>
          </FieldGroup>
          <FieldGroup>
            <Field>
              <Label>Microphone</Label>
              <FieldDescription>Select which microphone to use for pitch scoring</FieldDescription>
              <Select
                onValueChange={(value) =>
                  mutate({
                    preferred_mic: value === "__default__" ? null : value,
                  })
                }
                value={config?.preferred_mic ?? "__default__"}
              >
                <SelectTrigger className={generateRingClassName(1)}>
                  <SelectValue placeholder="Default microphone" />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectLabel>Microphone</SelectLabel>
                    <SelectItem value="__default__">Default</SelectItem>
                    {micDevices.map(({ deviceId, label }) => (
                      <SelectItem key={deviceId} value={deviceId}>
                        {label}
                      </SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
            <Field>
              <Label>Mic mirror gain</Label>
              <FieldDescription>
                Volume of your microphone played back through the speakers when mirroring (
                {micMirrorGainPct}%)
              </FieldDescription>
              <Slider
                min={0}
                max={200}
                step={1}
                value={[micMirrorGainPct]}
                onValueChange={([pct]) => mutate({ mic_mirror_gain: pct / 100 })}
                className={generateRingClassName(2)}
              />
            </Field>
            <Field>
              <Label htmlFor="model-1">Separator</Label>
              <FieldDescription>
                Karaoke removes backing vocals for cleaner lyrics; Demucs is faster
              </FieldDescription>
              <Select
                onValueChange={(value) => mutate({ separator: value })}
                value={config?.separator ?? DEFAULT_SEPARATOR}
              >
                <SelectTrigger id="separator-1" className={generateRingClassName(3)}>
                  <SelectValue placeholder="Select a separator" />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectLabel>Separator</SelectLabel>
                    {SEPARATORS.map(({ value, label }) => (
                      <SelectItem value={value}>{label}</SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
            <Field>
              <Label htmlFor="asr-engine-1">ASR Engine</Label>
              <FieldDescription>
                Whisper is multilingual and supports custom model sizes; Parakeet v3 is faster and
                covers 25 European languages (falls back to Whisper otherwise)
              </FieldDescription>
              <Select onValueChange={(value) => mutate({ asr_engine: value })} value={asrEngine}>
                <SelectTrigger id="asr-engine-1" className={generateRingClassName(4)}>
                  <SelectValue placeholder="Select an engine" />
                </SelectTrigger>
                <SelectContent>
                  <SelectGroup>
                    <SelectLabel>ASR Engine</SelectLabel>
                    {ASR_ENGINES.map(({ value, label }) => (
                      <SelectItem value={value}>{label}</SelectItem>
                    ))}
                  </SelectGroup>
                </SelectContent>
              </Select>
            </Field>
            {!isParakeet && (
              <>
                <Field>
                  <Label htmlFor="model-1">Model</Label>
                  <FieldDescription>
                    Smaller models are faster but produce worse results
                  </FieldDescription>
                  <Select
                    onValueChange={(value) => mutate({ whisper_model: value })}
                    value={config?.whisper_model ?? DEFAULT_MODEL}
                  >
                    <SelectTrigger id="model-1" className={generateRingClassName(5)}>
                      <SelectValue placeholder="Select a model" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectGroup>
                        <SelectLabel>Model</SelectLabel>
                        {MODELS.map((model) => (
                          <SelectItem value={model}>{model}</SelectItem>
                        ))}
                      </SelectGroup>
                    </SelectContent>
                  </Select>
                </Field>
                <Field>
                  <Label>Beam Size</Label>
                  <FieldDescription>
                    Higher values improve accuracy at the cost of speed
                  </FieldDescription>
                  <ButtonGroup>{generateNumberSelect("beam_size", beamSize, 6)}</ButtonGroup>
                </Field>
              </>
            )}
            <Field>
              <Label>Batch Size</Label>
              <FieldDescription>Higher values use more memory but process faster</FieldDescription>
              <ButtonGroup>
                {generateNumberSelect("batch_size", batchSize, batchSegment)}
              </ButtonGroup>
            </Field>
          </FieldGroup>
          <DialogFooter>
            <Button
              variant="ghost"
              onClick={() =>
                mutate({
                  separator: DEFAULT_SEPARATOR,
                  asr_engine: DEFAULT_ASR_ENGINE,
                  whisper_model: DEFAULT_MODEL,
                  beam_size: DEFAULT_BEAM_BATCH_SIZE,
                  batch_size: DEFAULT_BEAM_BATCH_SIZE,
                  mic_mirror_gain: DEFAULT_MIC_MIRROR_GAIN,
                })
              }
              className={generateRingClassName(footerSegment, 0)}
            >
              Restore Defaults
            </Button>
            <Button
              variant="outline"
              onClick={close}
              className={generateRingClassName(footerSegment, 1)}
            >
              Close
            </Button>
          </DialogFooter>
        </div>
      </DialogContent>
    </Dialog>
  );
};
