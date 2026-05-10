import type { TimeSubscriber } from "@/hooks/use-audio-player";
import type { MicReactiveRef } from "@/hooks/use-mic-reactive";
import { loadingFragment, shaders } from "./shaders";
import { ShaderVisualizer } from "./shader-visualizer";
import { PixabayVideo } from "./pixabay-video";
import { SourceVideo } from "./source-video";
import { FLAVORS, type VideoFlavor } from "@/lib/playback/video-flavor";

export type ThemeMode = "shader" | "pixabay" | "source";

export interface BackgroundProps {
  themeIndex: number;
  videoFlavor: VideoFlavor;
  sourceVideoPath?: string;
  sourceVideoTempoRatio?: number;
  isReady: boolean;
  isPlaying: boolean;
  subscribe: (fn: TimeSubscriber) => () => void;
  getCurrentTime: () => number;
  reactiveRef?: MicReactiveRef;
}

const SHADER_COUNT = shaders.length;
const PIXABAY_INDEX = SHADER_COUNT;
export const SOURCE_VIDEO_INDEX = SHADER_COUNT + 1;

export function themeMode(index: number): ThemeMode {
  if (index === PIXABAY_INDEX) {
    return "pixabay";
  }

  if (index === SOURCE_VIDEO_INDEX) {
    return "source";
  }

  return "shader";
}

export function themeName(index: number, videoFlavor: VideoFlavor): string {
  const mode = themeMode(index);

  if (mode === "source") {
    return "Source Video";
  }

  if (mode === "pixabay") {
    const name = videoFlavor.charAt(0).toUpperCase() + videoFlavor.slice(1);

    return `Video — ${name}`;
  }

  return shaders[index % SHADER_COUNT].name;
}

export function themeCount(hasSourceVideo: boolean): number {
  return SHADER_COUNT + 1 + (hasSourceVideo ? 1 : 0);
}

export function nextThemeIndex(current: number, hasSourceVideo: boolean): number {
  return (current + 1) % themeCount(hasSourceVideo);
}

export function nextFlavorIndex(current: number): number {
  return (current + 1) % FLAVORS.length;
}

export function isPixabayTheme(index: number): boolean {
  return index === PIXABAY_INDEX;
}

function backgroundContent(
  mode: ThemeMode,
  props: {
    themeIndex: number;
    videoFlavor: VideoFlavor;
    sourceVideoPath?: string;
    sourceVideoTempoRatio?: number;
    isPlaying: boolean;
    subscribe: BackgroundProps["subscribe"];
    getCurrentTime: BackgroundProps["getCurrentTime"];
    reactiveRef?: MicReactiveRef;
  },
) {
  const { themeIndex, videoFlavor, isPlaying, reactiveRef } = props;

  switch (mode) {
    case "shader":
      return (
        <ShaderVisualizer
          shaderIndex={themeIndex % SHADER_COUNT}
          isPlaying={isPlaying}
          reactiveRef={reactiveRef}
        />
      );
    case "pixabay":
      return <PixabayVideo flavor={videoFlavor} isPlaying={isPlaying} />;
    case "source":
      return null;
  }
}

export const Background = ({
  themeIndex,
  videoFlavor,
  sourceVideoPath,
  sourceVideoTempoRatio,
  isReady,
  isPlaying,
  subscribe,
  getCurrentTime,
  reactiveRef,
}: BackgroundProps) => {
  const mode = themeMode(themeIndex);
  const showSourceVideo = mode === "source";

  if (!isReady) {
    return (
      <div className="fixed inset-0">
        <ShaderVisualizer shaderIndex={0} isPlaying={true} customFragment={loadingFragment} />
      </div>
    );
  }

  return (
    <div className="fixed inset-0">
      {sourceVideoPath && (
        <SourceVideo
          filePath={sourceVideoPath}
          tempoRatio={sourceVideoTempoRatio ?? 1}
          isPlaying={isReady && isPlaying && showSourceVideo}
          isActive={showSourceVideo}
          subscribe={subscribe}
          getCurrentTime={getCurrentTime}
        />
      )}
      {backgroundContent(mode, {
        themeIndex,
        videoFlavor,
        sourceVideoPath,
        sourceVideoTempoRatio,
        isPlaying: isReady && isPlaying,
        subscribe,
        getCurrentTime,
        reactiveRef,
      })}
    </div>
  );
};
