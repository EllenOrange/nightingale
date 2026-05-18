import { playbackAdapter } from "@/bridge/playback";
import {
  usePlaybackThemeState,
  usePlaybackTransportActions,
  usePlaybackTransportState,
} from "@/contexts/playback";
import { useSourceVideoSync } from "@/hooks/use-source-video-sync";
import { useEffect, useRef, useState } from "react";
import { VIDEO_CLASS_NAME } from "@/lib/playback/video-styles";

interface SourceVideoProps {
  isActive: boolean;
}

function useMediaUrl(filePath: string): string | null {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setSrc(null);

    void playbackAdapter.init().then(() => {
      if (cancelled) return;
      setSrc(playbackAdapter.toMediaUrl(filePath));
    });

    return () => {
      cancelled = true;
    };
  }, [filePath]);

  return src;
}

export const SourceVideo = ({ isActive }: SourceVideoProps) => {
  const { sourceVideoPath, sourceVideoTempoRatio } = usePlaybackThemeState();
  const { isReady, isPlaying } = usePlaybackTransportState();
  const { subscribe, getCurrentTime } = usePlaybackTransportActions();

  const videoRef = useRef<HTMLVideoElement>(null);
  const src = useMediaUrl(sourceVideoPath ?? "");

  const playWhenActive = isReady && isPlaying && isActive;

  const { ready } = useSourceVideoSync({
    videoRef,
    src: sourceVideoPath ? src : null,
    isPlaying: playWhenActive,
    tempoRatio: sourceVideoTempoRatio,
    subscribe,
    getCurrentTime,
  });

  if (!sourceVideoPath || !src) return null;

  return (
    <video
      ref={videoRef}
      className={VIDEO_CLASS_NAME}
      style={{ visibility: ready && isActive ? "visible" : "hidden" }}
      src={src}
      muted
      playsInline
    />
  );
};
