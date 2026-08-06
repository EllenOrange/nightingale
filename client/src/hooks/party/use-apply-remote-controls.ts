/**
 * Party layer: make the TV playback session obey the admin's live controls.
 *
 * The admin page mutates shared jukebox state (paused, guide_vocal, volume, and
 * a restart token) and it is rebroadcast as a `jukebox` event. This hook, which
 * must be mounted inside the playback providers, applies each control to the
 * running audio engine.
 *
 * It reacts only to a control that actually *changed* since the last frame, not
 * to every frame. That keeps an unrelated update (say a guide-vocal change) from
 * clobbering a control the admin did not touch (say resuming a locally paused
 * song). On the first frame it adopts the current shared audio state (so a TV
 * joining mid-party picks up the live guide/volume/pause), diffing against
 * neutral defaults; only the restart-seek is suppressed on that first frame so a
 * stale restart token cannot yank a just-started song back to zero.
 *
 * Inert on Tauri (no party server) and, in practice, only ever runs on the TV,
 * since guest/admin tabs never navigate into the playback route.
 */

import { useEffect, useRef } from "react";
import { isTauri, listen, type UnlistenFn } from "@/bridge/runtime";
import { usePlaybackTransportActions } from "@/contexts/playback";

interface JukeboxControls {
  paused: boolean;
  guide_vocal: number | null;
  volume: number | null;
  restart_token: number;
}

export const PartyRemoteControls = () => {
  const { handlePause, handleContinue, setGuideVolume, setMasterVolume, seek } =
    usePlaybackTransportActions();
  const lastRef = useRef<JukeboxControls | null>(null);

  useEffect(() => {
    if (isTauri) return;

    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    listen<JukeboxControls>("jukebox", ({ payload }) => {
      const prev = lastRef.current;
      const isFirst = prev === null;
      lastRef.current = payload;

      // Diff against neutral defaults on the first frame so the TV adopts the
      // current shared state (paused / guide / volume) when it joins.
      const prevPaused = prev?.paused ?? false;
      const prevGuide = prev?.guide_vocal ?? null;
      const prevVolume = prev?.volume ?? null;

      if (payload.paused !== prevPaused) {
        if (payload.paused) handlePause();
        else handleContinue();
      }
      if (payload.guide_vocal != null && payload.guide_vocal !== prevGuide) {
        setGuideVolume(payload.guide_vocal);
      }
      if (payload.volume != null && payload.volume !== prevVolume) {
        setMasterVolume(payload.volume);
      }
      // Never seek to zero on the first frame: a restart token left over from
      // earlier in the server's life would otherwise restart a fresh song.
      if (!isFirst && payload.restart_token !== prev.restart_token) {
        seek(0);
      }
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [handlePause, handleContinue, setGuideVolume, setMasterVolume, seek]);

  return null;
};
