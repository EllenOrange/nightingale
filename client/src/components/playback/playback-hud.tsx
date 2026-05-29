import {
  usePlaybackMicActions,
  usePlaybackMicState,
  usePlaybackThemeActions,
  usePlaybackThemeState,
  usePlaybackTranscriptActions,
  usePlaybackTranscriptState,
  usePlaybackTransportActions,
  usePlaybackTransportState,
} from "@/contexts/playback";
import { usePlaybackConfigPersist } from "@/hooks/playback/use-playback-config-persist";
import type { VideoFlavor } from "@/lib/playback/video-flavor";
import type { AppConfig } from "@/types/AppConfig";
import { forwardRef, memo, useCallback, useEffect, useRef, useState } from "react";
import { isPixabayTheme, themeName } from "./background";

function formatTime(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = Math.floor(seconds) % 60;
  return `${mins}:${secs.toString().padStart(2, "0")}`;
}

function formatGuideText(volume: number): string {
  const pct = Math.round(volume * 100);
  return pct === 0 ? "Guide: OFF" : `Guide: ${pct}% [G +/-]`;
}

function formatThemeText(themeIndex: number, videoFlavor: VideoFlavor): string {
  return `Theme: ${themeName(themeIndex, videoFlavor)} [T${isPixabayTheme(themeIndex) ? "/F" : ""}]`;
}

const SkipButton = forwardRef<HTMLButtonElement, { label: string; onClick: () => void }>(
  ({ label, onClick }, ref) => (
    <button
      ref={ref}
      onClick={onClick}
      className="pointer-events-auto flex gap-1 rounded-sm border-2 border-white/70 bg-black/10 px-2.5 py-1 text-sm text-white/90 transition-colors hover:bg-black/20"
      style={{ display: "none" }}
    >
      <span>{label}</span> <span>⏎</span>
    </button>
  ),
);

function HintText({ children, fontSize = "sm" }: { children: React.ReactNode; fontSize?: string }) {
  return <p className={`text-${fontSize} text-white/50`}>{children}</p>;
}

const FOOTER_NOTE_CLASS = `pointer-events-none absolute bottom-2 z-20 text-[0.6rem] text-white/30`;
const TOUCH_QUERIES = ["(pointer: coarse)", "(any-pointer: coarse)"];

function hasTouchInput(): boolean {
  return (
    typeof window !== "undefined" &&
    (navigator.maxTouchPoints > 0 ||
      TOUCH_QUERIES.some((query) => window.matchMedia(query).matches))
  );
}

function useHasTouchInput(): boolean {
  const [enabled, setEnabled] = useState(hasTouchInput);

  useEffect(() => {
    const media = TOUCH_QUERIES.map((query) => window.matchMedia(query));
    const sync = () => setEnabled(hasTouchInput());

    sync();
    media.forEach((item) => item.addEventListener("change", sync));
    return () => media.forEach((item) => item.removeEventListener("change", sync));
  }, []);

  return enabled;
}

function TouchButton({
  label,
  onClick,
  disabled = false,
}: {
  label: string;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className="pointer-events-auto rounded-sm border-2 border-white/70 bg-black/10 px-2.5 py-1 text-sm text-white/90 transition-colors hover:bg-black/20 active:bg-black/30 disabled:opacity-35"
    >
      {label}
    </button>
  );
}

function SettingsInfo({
  guideVolume,
  micUserEnabled,
  micName,
  micMonitorUserEnabled,
  themeIndex,
  videoFlavor,
  showShortcuts,
}: {
  guideVolume: number;
  micUserEnabled: boolean;
  micName: string;
  micMonitorUserEnabled: boolean;
  themeIndex: number;
  videoFlavor: VideoFlavor;
  showShortcuts: boolean;
}) {
  return (
    <div className="flex flex-col items-end">
      <HintText>
        {showShortcuts ? formatGuideText(guideVolume) : `Guide: ${Math.round(guideVolume * 100)}%`}
      </HintText>
      <HintText>
        Mic: {micUserEnabled ? micName : "OFF"}
        {showShortcuts ? " [M/N]" : ""}
      </HintText>
      <HintText>
        Monitor: {micMonitorUserEnabled ? "ON" : "OFF"}
        {showShortcuts ? " [R]" : ""}
      </HintText>
      <HintText>
        {showShortcuts
          ? formatThemeText(themeIndex, videoFlavor)
          : `Theme: ${themeName(themeIndex, videoFlavor)}`}
      </HintText>
      {showShortcuts && <HintText>[ESC] Back</HintText>}
    </div>
  );
}

function TouchControls({ config, hasTouch }: { config: AppConfig | null; hasTouch: boolean }) {
  const [open, setOpen] = useState(false);
  const { guideVolume } = usePlaybackTransportState();
  const { setGuideVolume, handlePause } = usePlaybackTransportActions();
  const { micUserEnabled, micName, micMonitorUserEnabled } = usePlaybackMicState();
  const { handleToggleMic, handleCycleMic, handleToggleMicMonitor } = usePlaybackMicActions();
  const { themeIndex, videoFlavor } = usePlaybackThemeState();
  const { cycleTheme, cycleFlavor } = usePlaybackThemeActions();
  const persistConfig = usePlaybackConfigPersist(config);

  const setPersistedGuideVolume = useCallback(
    (volume: number) => {
      const next = Math.max(0, Math.min(1, volume));
      setGuideVolume(next);
      persistConfig({ guide_volume: next });
    },
    [persistConfig, setGuideVolume],
  );

  if (!hasTouch) {
    return null;
  }

  return (
    <div className="mt-2 flex w-[min(18rem,80vw)] flex-col items-end gap-2">
      <div className="sm:hidden">
        <SettingsInfo
          guideVolume={guideVolume}
          micUserEnabled={micUserEnabled}
          micName={micName}
          micMonitorUserEnabled={micMonitorUserEnabled}
          themeIndex={themeIndex}
          videoFlavor={videoFlavor}
          showShortcuts={false}
        />
      </div>

      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((prev) => !prev)}
        className="pointer-events-auto rounded-sm border-2 border-white/70 bg-black/10 px-2.5 py-1 text-sm text-white/90 transition-colors hover:bg-black/20 active:bg-black/30"
      >
        {open ? "Hide controls" : "Playback controls"}
      </button>

      {open && (
        <div className="grid w-full grid-cols-3 gap-2 text-center">
          <TouchButton label="Pause" onClick={handlePause} />
          <TouchButton
            label={guideVolume === 0 ? "Guide On" : "Guide Off"}
            onClick={() => setPersistedGuideVolume(guideVolume > 0 ? 0 : 0.3)}
          />
          <TouchButton label="Guide +" onClick={() => setPersistedGuideVolume(guideVolume + 0.1)} />
          <TouchButton label="Guide -" onClick={() => setPersistedGuideVolume(guideVolume - 0.1)} />
          <TouchButton label={micUserEnabled ? "Mic Off" : "Mic On"} onClick={handleToggleMic} />
          <TouchButton label="Mic Select" onClick={handleCycleMic} />
          <TouchButton
            label={micMonitorUserEnabled ? "Monitor Off" : "Monitor On"}
            onClick={handleToggleMicMonitor}
          />
          <TouchButton label="Theme" onClick={cycleTheme} />
          <TouchButton
            label="Flavor"
            onClick={cycleFlavor}
            disabled={!isPixabayTheme(themeIndex)}
          />
        </div>
      )}
    </div>
  );
}

function Disclaimer({ source }: { source: string }) {
  if (source === "usdx") {
    return null;
  }

  const text =
    source === "lyrics"
      ? "Timing is AI-generated and may not be perfectly accurate"
      : "Lyrics and timing are AI-generated and may not be perfectly accurate";

  return (
    <p className={`${FOOTER_NOTE_CLASS} left-1/2 -translate-x-1/2 whitespace-nowrap text-center`}>
      {text}
    </p>
  );
}

interface PlaybackHudProps {
  title: string;
  artist: string;
  config: AppConfig | null;
}

function PlaybackHudImpl({ title, artist, config }: PlaybackHudProps) {
  const { duration, guideVolume } = usePlaybackTransportState();
  const { subscribe, getCurrentTime } = usePlaybackTransportActions();
  const { themeIndex, videoFlavor } = usePlaybackThemeState();
  const { firstSegmentStart, lastSegmentEnd, introSkipLeadSec, transcriptSource } =
    usePlaybackTranscriptState();
  const { handleSkipIntro, handleSkipOutro } = usePlaybackTranscriptActions();
  const { pitchScore, micUserEnabled, micName, micMonitorUserEnabled } = usePlaybackMicState();

  const lastSecondRef = useRef(-1);
  const timerRef = useRef<HTMLParagraphElement>(null);
  const skipIntroRef = useRef<HTMLButtonElement>(null);
  const skipOutroRef = useRef<HTMLButtonElement>(null);

  const showPixabayCredit = isPixabayTheme(themeIndex);
  const hasTouch = useHasTouchInput();

  // Updates the timer text and skip-button visibility via direct DOM mutation
  // (rAF subscriber), only triggering a text update when the displayed second changes.
  useEffect(() => {
    if (timerRef.current) {
      timerRef.current.textContent = `${formatTime(getCurrentTime())} / ${formatTime(duration)}`;
    }

    return subscribe((time) => {
      const sec = Math.floor(time);
      if (sec !== lastSecondRef.current) {
        lastSecondRef.current = sec;
        if (timerRef.current) {
          timerRef.current.textContent = `${formatTime(time)} / ${formatTime(duration)}`;
        }
      }

      if (skipIntroRef.current) {
        skipIntroRef.current.style.display =
          time < firstSegmentStart - introSkipLeadSec ? "" : "none";
      }
      if (skipOutroRef.current) {
        skipOutroRef.current.style.display = time > lastSegmentEnd + 1 ? "" : "none";
      }
    });
  }, [subscribe, getCurrentTime, duration, firstSegmentStart, introSkipLeadSec, lastSegmentEnd]);

  return (
    <>
      <div className="pointer-events-auto absolute inset-x-0 top-[4.25rem] z-20 flex items-start justify-between gap-3 px-3 md:top-3 md:px-4">
        <div className="min-w-0 max-w-[58%] overflow-hidden sm:max-w-[34%] lg:max-w-[40%]">
          <h1 className="line-clamp-2 [overflow-wrap:anywhere] text-base leading-tight text-white md:text-[1.375rem]">
            {title}
          </h1>
          <p className="line-clamp-1 [overflow-wrap:anywhere] text-sm text-white/70 md:text-base">
            {artist}
          </p>
          <p ref={timerRef} className="text-sm text-white/70 md:text-base">
            0:00 / {formatTime(duration)}
          </p>
          <div className="mt-2 flex gap-2">
            <SkipButton ref={skipIntroRef} label="Skip Intro" onClick={handleSkipIntro} />
            <SkipButton ref={skipOutroRef} label="Skip Outro" onClick={handleSkipOutro} />
          </div>
        </div>

        <div className="flex min-w-0 flex-col items-end">
          <div className={`text-base md:text-lg ${pitchScore ? "text-white" : "text-white/50"}`}>
            Score: {pitchScore ?? "--"}
          </div>
          <div className="hidden sm:block">
            <SettingsInfo
              guideVolume={guideVolume}
              micUserEnabled={micUserEnabled}
              micName={micName}
              micMonitorUserEnabled={micMonitorUserEnabled}
              themeIndex={themeIndex}
              videoFlavor={videoFlavor}
              showShortcuts={!hasTouch}
            />
          </div>
          <TouchControls config={config} hasTouch={hasTouch} />
        </div>
      </div>

      {showPixabayCredit && <p className={`${FOOTER_NOTE_CLASS} right-4`}>Videos by Pixabay</p>}

      <Disclaimer source={transcriptSource} />
    </>
  );
}

export const PlaybackHud = memo(PlaybackHudImpl);
