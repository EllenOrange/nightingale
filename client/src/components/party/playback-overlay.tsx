/**
 * Party overlay for the TV playback screen. Shows the live "Up next" queue in
 * the upper right and a join QR in the lower right, replacing the singer-focused
 * keyboard hints that normally live there. Pressing `?` toggles those hints back
 * in, in place of the queue (press again to return to the queue).
 *
 * Mounted only in web (party) mode, inside the playback providers so it can read
 * the transport/mic/theme state the hints need.
 */

import { useEffect, useMemo, useState } from "react";
import { SettingsInfo } from "@/components/playback/playback-hud";
import {
  usePlaybackMicState,
  usePlaybackThemeState,
  usePlaybackTransportState,
} from "@/contexts/playback";
import { usePartyQueue } from "@/hooks/party/use-party-queue";

const UpNextPanel = () => {
  const queue = usePartyQueue();
  const upNext = useMemo(
    () => queue.entries.filter((e) => e.status !== "done" && e.status !== "playing").slice(0, 6),
    [queue.entries],
  );

  return (
    <div className="flex flex-col items-end gap-1 text-right">
      <div className="text-xs uppercase tracking-widest text-white/50">Up next</div>
      {upNext.length === 0 ? (
        <div className="text-sm text-white/40">Scan to add a song →</div>
      ) : (
        upNext.map((e, i) => (
          <div key={e.id} className="max-w-full truncate text-sm text-white/80">
            <span className="text-white/40">{i + 1}. </span>
            {e.title || e.query || "Untitled"}
            {e.requestedBy ? <span className="text-white/40"> · {e.requestedBy}</span> : null}
          </div>
        ))
      )}
    </div>
  );
};

export const PartyPlaybackOverlay = () => {
  const [showHints, setShowHints] = useState(false);
  const { guideVolume, guideAvailable } = usePlaybackTransportState();
  const { micUserEnabled, micName, micMonitorUserEnabled } = usePlaybackMicState();
  const { themeIndex, videoFlavor } = usePlaybackThemeState();

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "?") {
        e.preventDefault();
        setShowHints((v) => !v);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <>
      {/* Upper right: the queue, or the keyboard hints when toggled with `?`. */}
      <div className="pointer-events-none absolute right-3 top-16 z-20 w-56 max-w-[45vw] md:right-4 md:top-[3.75rem]">
        {showHints ? (
          <SettingsInfo
            guideVolume={guideVolume}
            guideAvailable={guideAvailable}
            micUserEnabled={micUserEnabled}
            micName={micName}
            micMonitorUserEnabled={micMonitorUserEnabled}
            themeIndex={themeIndex}
            videoFlavor={videoFlavor}
            showShortcuts
          />
        ) : (
          <UpNextPanel />
        )}
      </div>

      {/* Lower right: the join QR. */}
      <div className="pointer-events-none absolute bottom-3 right-3 z-20 flex flex-col items-end md:right-4">
        <div className="rounded-md bg-white p-1.5">
          <img src="/qr" alt="Scan to join the party" width={96} height={96} />
        </div>
        <div className="mt-1 text-right text-[0.6rem] leading-tight text-white/60">
          Scan to join · press ? for controls
        </div>
      </div>
    </>
  );
};
