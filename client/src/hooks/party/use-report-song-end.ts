/**
 * Party layer: report song end to the server so the queue can auto-advance.
 *
 * The server never sees playback position (the browser owns it), so the TV tab
 * has to tell the server when the current song finishes. This watches the
 * playback transport's `isFinished` and fires `party_song_ended` once per song.
 * The server matches on the hash before advancing, so a duplicate or stale
 * report is harmless.
 *
 * Must be mounted inside the playback providers (it reads transport state).
 * Inert on the Tauri desktop build, which has no party server.
 */

import { useEffect, useRef } from "react";
import { invoke, isTauri } from "@/bridge/runtime";
import { usePlaybackTransportState } from "@/contexts/playback";

export const PartySongEndReporter = ({ fileHash }: { fileHash: string }) => {
  const { isFinished } = usePlaybackTransportState();
  const reportedRef = useRef(false);

  useEffect(() => {
    if (isTauri) return;
    if (!isFinished || reportedRef.current) return;

    reportedRef.current = true;
    void invoke("party_song_ended", { fileHash }).catch(() => {
      // Best effort: if the report fails, the admin can skip manually. Don't
      // surface an error to the singer mid-party.
    });
  }, [isFinished, fileHash]);

  return null;
};
