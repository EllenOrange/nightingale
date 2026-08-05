/**
 * Party layer: makes the TV browser tab obey a remote play signal.
 *
 * The server never plays audio (the browser does), so a `party_play` command
 * only publishes a play *intent* into the shared jukebox state, rebroadcast to
 * every client over `/ws` as a `jukebox` event. This hook watches that stream
 * and, when the `play_token` advances, resolves the requested hash to a full
 * `Song` and navigates to /playback exactly as the song-details sidebar does.
 *
 * Mount this once, high in the tree (inside the Router), so it is active while
 * the tab sits on the menu. See PHASE1_PLAN.md Step 1.
 *
 * Role split: only the "screen" (the TV) should obey a play signal. The guest
 * and admin pages are controllers, not screens, so a tab currently on `/party`
 * or `/admin` never navigates to playback (otherwise a guest's own phone would
 * get yanked into the video when their song starts).
 */

/** Routes that act as remote controllers and must never become the TV screen. */
const CONTROLLER_ROUTES = ["/party", "/admin"];

const isControllerTab = () => CONTROLLER_ROUTES.some((r) => window.location.pathname.startsWith(r));

import { useEffect, useRef } from "react";
import { useNavigate } from "react-router";
import { invoke, isTauri, listen, type UnlistenFn } from "@/bridge/runtime";
import type { Song } from "@/types/Song";

interface JukeboxState {
  requested_song_hash: string | null;
  play_token: number;
}

export const useRemotePlayback = () => {
  const navigate = useNavigate();
  // Track the last play_token we acted on so we only navigate on a genuine
  // change, and so the snapshot the server sends on connect (token 0, or a
  // token from a play that already happened) does not re-trigger a navigate.
  const lastTokenRef = useRef<number | null>(null);

  useEffect(() => {
    // The jukebox is a web-server concept. In the Tauri desktop build there is
    // no server broadcasting it, so this hook is inert; skip it entirely to
    // avoid changing desktop behaviour.
    if (isTauri) return;

    let unlisten: UnlistenFn | undefined;
    let cancelled = false;

    const handle = async (state: JukeboxState) => {
      const token = state.play_token;
      const hash = state.requested_song_hash;

      // Initialise the baseline from the first snapshot without acting on it.
      if (lastTokenRef.current === null) {
        lastTokenRef.current = token;
        return;
      }

      if (token === lastTokenRef.current || !hash) return;
      lastTokenRef.current = token;

      // Controllers (guest/admin phones) observe play signals but never follow
      // them into the video; only the TV screen does.
      if (isControllerTab()) return;

      try {
        const song = await invoke<Song | null>("party_song_by_hash", { fileHash: hash });
        if (!cancelled && song) {
          navigate("/playback", { state: { song } });
        }
      } catch {
        // A failed resolve should not break the listener; the next play_token
        // change will try again.
      }
    };

    listen<JukeboxState>("jukebox", ({ payload }) => {
      void handle(payload);
    }).then((fn) => {
      if (cancelled) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [navigate]);
};

export const RemotePlayback = () => {
  useRemotePlayback();
  return null;
};
