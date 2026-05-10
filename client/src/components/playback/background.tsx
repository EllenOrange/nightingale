import {
  usePlaybackMicActions,
  usePlaybackThemeState,
  usePlaybackTransportState,
} from "@/contexts/playback";
import { FLAVORS, type VideoFlavor } from "@/lib/playback/video-flavor";
import { memo } from "react";
import { PixabayVideo } from "./pixabay-video";
import { ShaderVisualizer } from "./shader-visualizer";
import { loadingFragment, shaders } from "./shaders";
import { SourceVideo } from "./source-video";

export type ThemeMode = "shader" | "pixabay" | "source";

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

function ShaderBranch({ themeIndex, isPlaying }: { themeIndex: number; isPlaying: boolean }) {
  const { reactiveRef } = usePlaybackMicActions();
  return (
    <ShaderVisualizer
      shaderIndex={themeIndex % SHADER_COUNT}
      isPlaying={isPlaying}
      reactiveRef={reactiveRef}
    />
  );
}

function BackgroundImpl() {
  const { isReady, isPlaying } = usePlaybackTransportState();
  const { themeIndex, videoFlavor, sourceVideoPath } = usePlaybackThemeState();

  if (!isReady) {
    return (
      <div className="fixed inset-0">
        <ShaderVisualizer shaderIndex={0} isPlaying={true} customFragment={loadingFragment} />
      </div>
    );
  }

  const mode = themeMode(themeIndex);
  const showSourceVideo = mode === "source";
  const playing = isReady && isPlaying;

  return (
    <div className="fixed inset-0">
      {sourceVideoPath && <SourceVideo isActive={showSourceVideo} />}
      {mode === "shader" && <ShaderBranch themeIndex={themeIndex} isPlaying={playing} />}
      {mode === "pixabay" && <PixabayVideo flavor={videoFlavor} isPlaying={playing} />}
    </div>
  );
}

export const Background = memo(BackgroundImpl);
