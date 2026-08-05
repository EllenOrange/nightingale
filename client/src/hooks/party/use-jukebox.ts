/**
 * Live jukebox state for controller pages (the admin). Tracks the latest
 * `jukebox` broadcast so the admin's sliders and transport buttons reflect the
 * shared state every client sees.
 */

import { useEffect, useState } from "react";
import { listen } from "@/bridge/runtime";

export interface JukeboxSnapshot {
  requested_song_hash: string | null;
  play_token: number;
  restart_token: number;
  paused: boolean;
  guide_vocal: number | null;
  key_offset: number | null;
  volume: number | null;
}

export const useJukebox = (): JukeboxSnapshot | null => {
  const [state, setState] = useState<JukeboxSnapshot | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    // The server sends a snapshot immediately on connect, so no separate fetch
    // is needed to seed the initial value.
    listen<JukeboxSnapshot>("jukebox", ({ payload }) => {
      if (!cancelled) setState(payload);
    }).then((fn) => {
      if (cancelled) fn();
      else unlisten = fn;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return state;
};
