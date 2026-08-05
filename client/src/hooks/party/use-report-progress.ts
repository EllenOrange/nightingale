/**
 * Party layer: heartbeat the TV's playback position to the server (~every 3s).
 *
 * The server's auto-advance watchdog uses this to tell "playing" from "paused"
 * from "TV gone", so it never advances a paused or restarted song and only falls
 * back to a wall-clock timer when these heartbeats stop. Must be mounted inside
 * the playback providers. Inert on Tauri.
 */

import { useEffect } from "react";
import { invoke, isTauri } from "@/bridge/runtime";
import { usePlaybackTransportActions, usePlaybackTransportState } from "@/contexts/playback";

const INTERVAL_MS = 3000;

export const PartyProgressReporter = ({ fileHash }: { fileHash: string }) => {
  const { paused } = usePlaybackTransportState();
  const { getCurrentTime } = usePlaybackTransportActions();

  useEffect(() => {
    if (isTauri) return;

    const send = () => {
      // Include the hash so the server can drop a stale heartbeat from a
      // just-finished song during a transition.
      void invoke("party_progress", {
        fileHash,
        positionMs: Math.round(getCurrentTime() * 1000),
        paused,
      }).catch(() => {});
    };

    // Send immediately so a pause/resume reflects promptly (the effect re-runs
    // when `paused` flips), then keep a steady heartbeat.
    send();
    const id = setInterval(send, INTERVAL_MS);
    return () => clearInterval(id);
  }, [fileHash, getCurrentTime, paused]);

  return null;
};
