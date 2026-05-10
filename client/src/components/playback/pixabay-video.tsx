import { usePixabaySlots } from "@/hooks/use-pixabay-slots";
import type { VideoFlavor } from "@/lib/playback/video-flavor";
import { VIDEO_CLASS_NAME } from "@/lib/playback/video-styles";

interface PixabayVideoProps {
  flavor: VideoFlavor;
  isPlaying: boolean;
}

export const PixabayVideo = ({ flavor, isPlaying }: PixabayVideoProps) => {
  const { slots, onActiveEnded } = usePixabaySlots(flavor, isPlaying);

  return (
    <>
      {slots.map((slot, i) => (
        <video
          key={i}
          ref={slot.ref}
          className={VIDEO_CLASS_NAME}
          style={{ visibility: slot.isActive ? "visible" : "hidden" }}
          src={slot.src || undefined}
          preload="auto"
          muted
          playsInline
          onEnded={slot.isActive ? onActiveEnded : undefined}
          onError={slot.isActive ? onActiveEnded : undefined}
        />
      ))}
    </>
  );
};
