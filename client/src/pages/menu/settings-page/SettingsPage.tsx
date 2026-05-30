import { Button } from "@/components/ui/button";
import { ButtonGroup } from "@/components/ui/button-group";
import { Field, FieldGroup } from "@/components/ui/field";
import { Label } from "@/components/ui/label";
import { Slider } from "@/components/ui/slider";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { setFullScreen, isFullScreen as tauriIsFullScreen } from "@/bridge/fullScreen";
import { useMicDevices } from "@/hooks/use-mic-pitch";
import { useConfigMutation } from "@/mutations/use-config-mutation";
import { useConfig } from "@/queries/use-config";
import type { AppConfig } from "@/types/AppConfig";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router";
import {
  ASR_ENGINES,
  DEFAULTS,
  LYRICS_HORIZONTAL_POSITIONS,
  LYRICS_VERTICAL_POSITIONS,
  MODELS,
  NAV,
  SEPARATORS,
  SETTINGS_TABS,
  getBatchSizeSegment,
  type SettingsTab,
} from "./constants";
import { MicLatencyField } from "./mic-latency-field";
import { Hint, NumberButtonGroup, PageHeader, SettingsSelect } from "./settings-controls";
import { useSettingsNavigation } from "./use-settings-navigation";

const DEFAULT_MIC_ID = "__default__";

export const SettingsPage = () => {
  const micDevices = useMicDevices();
  const navigate = useNavigate();
  const { data: config } = useConfig();
  const { mutate } = useConfigMutation();

  const containerRef = useRef<HTMLDivElement>(null);
  const [tab, setTab] = useState<SettingsTab>("general");
  const [isFullScreen, setIsFullScreen] = useState<boolean | null | undefined>(config?.fullscreen);
  const [micMonitorGain, setMicMonitorGain] = useState(
    config?.mic_monitor_gain ?? DEFAULTS.mic_monitor_gain,
  );
  const [micLatencySec, setMicLatencySec] = useState(
    config?.mic_latency_compensation_sec ?? DEFAULTS.mic_latency_compensation_sec,
  );

  const close = () => navigate("/");
  const asrEngine = config?.asr_engine ?? DEFAULTS.asr_engine;
  const isParakeet = asrEngine === "parakeet";
  const batchSegment = getBatchSizeSegment(isParakeet);

  const micOptions = useMemo(
    () => [
      { value: DEFAULT_MIC_ID, label: "Default" },
      ...micDevices.map(({ deviceId, label }) => ({ value: deviceId, label })),
    ],
    [micDevices],
  );
  const modelOptions = useMemo(() => MODELS.map((model) => ({ value: model, label: model })), []);
  const micMonitorGainPct = Math.round(micMonitorGain * 100);
  const batchSize = config?.batch_size ?? DEFAULTS.batch_size;
  const beamSize = config?.beam_size ?? DEFAULTS.beam_size;

  useEffect(() => {
    setMicMonitorGain(config?.mic_monitor_gain ?? DEFAULTS.mic_monitor_gain);
  }, [config?.mic_monitor_gain]);

  useEffect(() => {
    setMicLatencySec(config?.mic_latency_compensation_sec ?? DEFAULTS.mic_latency_compensation_sec);
  }, [config?.mic_latency_compensation_sec]);

  useEffect(() => {
    const updateIsFullScreen = async () => {
      setIsFullScreen(await tauriIsFullScreen());
    };

    updateIsFullScreen();
  }, []);

  const updateMicMonitorGain = (gain: number) => {
    setMicMonitorGain(gain);
    mutate({ mic_monitor_gain: gain });
  };

  const updateMicLatency = (latencySec: number) => {
    setMicLatencySec(latencySec);
    mutate({ mic_latency_compensation_sec: latencySec });
  };

  const toggleWindowMode = (fullscreen: boolean) => {
    setIsFullScreen(fullscreen);
    setFullScreen(fullscreen);
    mutate({ fullscreen });
  };

  const resetDefaults = () => {
    mutate(DEFAULTS);
    setMicMonitorGain(DEFAULTS.mic_monitor_gain);
    setMicLatencySec(DEFAULTS.mic_latency_compensation_sec);
  };

  const { footerSegment, getFocusClassName, syncFocusFromElement } = useSettingsNavigation({
    containerRef,
    tab,
    isParakeet,
    micMonitorGain,
    micLatencySec,
    onBack: close,
    onTabChange: setTab,
    onMicMonitorGainChange: updateMicMonitorGain,
    onMicLatencyChange: updateMicLatency,
  });

  return (
    <div
      ref={containerRef}
      className="h-full overflow-y-auto px-4 pb-5 pt-14 sm:px-6 md:pt-5 lg:px-8"
      onMouseMoveCapture={(event) => syncFocusFromElement(event.target)}
      onFocusCapture={(event) => syncFocusFromElement(event.target)}
    >
      <div className="mx-auto flex max-w-4xl flex-col gap-5">
        <PageHeader />

        <Tabs value={tab} onValueChange={(value) => setTab(value as SettingsTab)}>
          <TabsList className="scrollbar-hide max-w-full overflow-x-auto overflow-y-hidden sm:w-fit">
            {SETTINGS_TABS.map((settingsTab, slot) => (
              <TabsTrigger
                key={settingsTab.value}
                value={settingsTab.value}
                className={getFocusClassName(NAV.tabSegment, slot)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") setTab(settingsTab.value);
                }}
              >
                {settingsTab.label}
              </TabsTrigger>
            ))}
          </TabsList>

          <TabsContent value="general" className="mt-4">
            <FieldGroup>
              <Field>
                <Label>Window</Label>
                <ButtonGroup>
                  <Button
                    variant={isFullScreen === true ? "outline" : "default"}
                    onClick={() => toggleWindowMode(false)}
                    className={getFocusClassName(NAV.general.window, 0)}
                  >
                    Windowed
                  </Button>
                  <Button
                    variant={isFullScreen === false ? "outline" : "default"}
                    onClick={() => toggleWindowMode(true)}
                    className={getFocusClassName(NAV.general.window, 1)}
                  >
                    Fullscreen
                  </Button>
                </ButtonGroup>
              </Field>

              <Field>
                <Label>Microphone</Label>
                <Hint>Select which microphone to use for pitch scoring</Hint>
                <SettingsSelect
                  label="Microphone"
                  placeholder="Default microphone"
                  value={config?.preferred_mic ?? DEFAULT_MIC_ID}
                  options={micOptions}
                  triggerClassName={getFocusClassName(NAV.general.microphone)}
                  onValueChange={(value) =>
                    mutate({ preferred_mic: value === DEFAULT_MIC_ID ? null : value })
                  }
                />
              </Field>

              <Field>
                <Label>Mic monitor gain</Label>
                <Hint>
                  Volume of your microphone played back through the speakers while monitoring (
                  {micMonitorGainPct}%)
                </Hint>
                <Slider
                  min={0}
                  max={200}
                  step={1}
                  value={[micMonitorGainPct]}
                  onValueChange={([pct]) => updateMicMonitorGain(pct / 100)}
                  className={getFocusClassName(NAV.general.micMonitorGain)}
                />
              </Field>

              <MicLatencyField
                selectedMicId={config?.preferred_mic ?? null}
                latencySec={micLatencySec}
                sliderClassName={getFocusClassName(NAV.general.micLatency, 0)}
                buttonClassName={getFocusClassName(NAV.general.micLatency, 1)}
                onLatencyChange={updateMicLatency}
              />

              <Field>
                <Label htmlFor="lyrics-vertical-position-1">Lyrics vertical position</Label>
                <Hint>Top moves playback HUD and pitch graph to the bottom</Hint>
                <SettingsSelect
                  id="lyrics-vertical-position-1"
                  label="Lyrics vertical position"
                  placeholder="Select vertical position"
                  value={config?.lyrics_vertical_position ?? DEFAULTS.lyrics_vertical_position}
                  options={LYRICS_VERTICAL_POSITIONS}
                  triggerClassName={getFocusClassName(NAV.general.lyricsVerticalPosition)}
                  onValueChange={(lyrics_vertical_position) =>
                    mutate({
                      lyrics_vertical_position:
                        lyrics_vertical_position as AppConfig["lyrics_vertical_position"],
                    })
                  }
                />
              </Field>

              <Field>
                <Label htmlFor="lyrics-horizontal-position-1">Lyrics horizontal position</Label>
                <Hint>Align lyrics left, center, or right during playback</Hint>
                <SettingsSelect
                  id="lyrics-horizontal-position-1"
                  label="Lyrics horizontal position"
                  placeholder="Select horizontal position"
                  value={config?.lyrics_horizontal_position ?? DEFAULTS.lyrics_horizontal_position}
                  options={LYRICS_HORIZONTAL_POSITIONS}
                  triggerClassName={getFocusClassName(NAV.general.lyricsHorizontalPosition)}
                  onValueChange={(lyrics_horizontal_position) =>
                    mutate({
                      lyrics_horizontal_position:
                        lyrics_horizontal_position as AppConfig["lyrics_horizontal_position"],
                    })
                  }
                />
              </Field>
            </FieldGroup>
          </TabsContent>

          <TabsContent value="analysis" className="mt-4">
            <FieldGroup>
              <Field>
                <Label htmlFor="separator-1">Separator</Label>
                <Hint>Karaoke removes backing vocals for cleaner lyrics; Demucs is faster</Hint>
                <SettingsSelect
                  id="separator-1"
                  label="Separator"
                  placeholder="Select a separator"
                  value={config?.separator ?? DEFAULTS.separator}
                  options={SEPARATORS}
                  triggerClassName={getFocusClassName(NAV.analysis.separator)}
                  onValueChange={(separator) => mutate({ separator })}
                />
              </Field>

              <Field>
                <Label htmlFor="asr-engine-1">ASR Engine</Label>
                <Hint>
                  Whisper is multilingual and supports custom model sizes; Parakeet v3 is faster and
                  covers 25 European languages (falls back to Whisper otherwise)
                </Hint>
                <SettingsSelect
                  id="asr-engine-1"
                  label="ASR Engine"
                  placeholder="Select an engine"
                  value={asrEngine}
                  options={ASR_ENGINES}
                  triggerClassName={getFocusClassName(NAV.analysis.asrEngine)}
                  onValueChange={(asr_engine) => mutate({ asr_engine })}
                />
              </Field>

              <Field>
                <Label>Auto-analyze</Label>
                <Hint>Automatically queue every unanalyzed song after scans finish</Hint>
                <ButtonGroup>
                  <Button
                    variant={config?.auto_analyze === true ? "outline" : "default"}
                    onClick={() => mutate({ auto_analyze: false })}
                    className={getFocusClassName(NAV.analysis.autoAnalyze, 0)}
                  >
                    Off
                  </Button>
                  <Button
                    variant={config?.auto_analyze === true ? "default" : "outline"}
                    onClick={() => mutate({ auto_analyze: true })}
                    className={getFocusClassName(NAV.analysis.autoAnalyze, 1)}
                  >
                    On
                  </Button>
                </ButtonGroup>
              </Field>

              {!isParakeet && (
                <>
                  <Field>
                    <Label htmlFor="model-1">Model</Label>
                    <Hint>Smaller models are faster but produce worse results</Hint>
                    <SettingsSelect
                      id="model-1"
                      label="Model"
                      placeholder="Select a model"
                      value={config?.whisper_model ?? DEFAULTS.whisper_model}
                      options={modelOptions}
                      triggerClassName={getFocusClassName(NAV.analysis.whisperModel)}
                      onValueChange={(whisper_model) => mutate({ whisper_model })}
                    />
                  </Field>

                  <Field>
                    <Label>Beam Size</Label>
                    <Hint>Higher values improve accuracy at the cost of speed</Hint>
                    <NumberButtonGroup
                      name="beam_size"
                      value={beamSize}
                      segment={NAV.analysis.beamSize}
                      getFocusClassName={getFocusClassName}
                      onChange={(beam_size) => mutate({ beam_size })}
                    />
                  </Field>
                </>
              )}

              <Field>
                <Label>Batch Size</Label>
                <Hint>Higher values use more memory but process faster</Hint>
                <NumberButtonGroup
                  name="batch_size"
                  value={batchSize}
                  segment={batchSegment}
                  getFocusClassName={getFocusClassName}
                  onChange={(batch_size) => mutate({ batch_size })}
                />
              </Field>
            </FieldGroup>
          </TabsContent>
        </Tabs>

        <div className="flex flex-col-reverse gap-2 border-t pt-4 sm:flex-row sm:justify-end">
          <Button
            variant="ghost"
            onClick={resetDefaults}
            className={getFocusClassName(footerSegment, 0)}
          >
            Restore Defaults
          </Button>
          <Button variant="outline" onClick={close} className={getFocusClassName(footerSegment, 1)}>
            Close
          </Button>
        </div>
      </div>
    </div>
  );
};
