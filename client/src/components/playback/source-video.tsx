import { joinMediaUrl } from "@/adapters/playback";
import type { TimeSubscriber } from "@/hooks/use-audio-player";
import { useSourceVideoSync } from "@/hooks/use-source-video-sync";
import { getMediaPort } from "@/tauri-bridge/playback";
import { useEffect, useRef, useState } from "react";
import { VIDEO_CLASS_NAME } from "./video-styles";

interface SourceVideoProps {
  filePath: string;
  tempoRatio: number;
  isPlaying: boolean;
  isActive: boolean;
  subscribe: (fn: TimeSubscriber) => () => void;
  getCurrentTime: () => number;
}

function useMediaUrl(filePath: string): string | null {
  const [src, setSrc] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setSrc(null);

    void getMediaPort().then((port) => {
      if (cancelled) return;
      setSrc(joinMediaUrl(`http://127.0.0.1:${port}`, filePath));
    });

    return () => {
      cancelled = true;
    };
  }, [filePath]);

  return src;
}

export const SourceVideo = ({
  filePath,
  tempoRatio,
  isPlaying,
  isActive,
  subscribe,
  getCurrentTime,
}: SourceVideoProps) => {
  const videoRef = useRef<HTMLVideoElement>(null);
  const src = useMediaUrl(filePath);
  const { ready } = useSourceVideoSync({
    videoRef,
    src,
    isPlaying,
    tempoRatio,
    subscribe,
    getCurrentTime,
  });

  if (!src) return null;

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
