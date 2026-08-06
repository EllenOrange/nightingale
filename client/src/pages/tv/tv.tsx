/**
 * TV idle / lobby screen (/tv): what the TV shows between songs. Big join QR,
 * the current song, and what's up next. It is a *screen*, not a controller, so
 * the remote-playback hook takes it into /playback when a song starts; it comes
 * back here between sets.
 *
 * Display policy (PHASE1_PLAN.md Step 5): open the TV browser on /tv. A playing
 * song takes over the screen at /playback with the video + synced lyrics; when
 * the queue runs dry, playback returns to the menu and the host can reopen /tv.
 * Auto-returning here on an empty queue is a Phase 3 polish.
 */

import { useMemo } from "react";
import { usePartyQueue } from "@/hooks/party/use-party-queue";

export const Tv = () => {
  const queue = usePartyQueue();

  const nowPlaying = useMemo(
    () => queue.entries.find((e) => e.status === "playing") ?? null,
    [queue.entries],
  );
  const upNext = useMemo(
    () =>
      queue.entries.filter(
        (e) => e.status !== "playing" && e.status !== "done" && e.status !== "error",
      ),
    [queue.entries],
  );

  return (
    <div className="flex min-h-dvh flex-col gap-8 bg-black p-10 text-white">
      <div className="flex flex-1 items-center justify-between gap-10">
        <div className="flex min-w-0 flex-1 flex-col gap-6">
          <div>
            <div className="text-lg uppercase tracking-widest text-white/50">Now playing</div>
            {nowPlaying ? (
              <>
                <div className="truncate text-5xl font-bold">
                  {nowPlaying.title || nowPlaying.query}
                </div>
                <div className="mt-1 truncate text-2xl text-white/70">
                  {nowPlaying.artist}
                  {nowPlaying.requestedBy ? `  ·  ${nowPlaying.requestedBy}` : ""}
                </div>
              </>
            ) : (
              <div className="text-4xl font-semibold text-white/60">Nothing playing yet</div>
            )}
          </div>

          <div className="min-h-0 flex-1">
            <div className="text-lg uppercase tracking-widest text-white/50">Up next</div>
            {upNext.length === 0 ? (
              <div className="mt-2 text-2xl text-white/40">Scan to add the first song</div>
            ) : (
              <ol className="mt-2 flex flex-col gap-2">
                {upNext.slice(0, 6).map((e, i) => (
                  <li key={e.id} className="flex items-baseline gap-3 text-2xl">
                    <span className="w-8 shrink-0 text-white/40 tabular-nums">{i + 1}</span>
                    <span className="min-w-0 flex-1 truncate">
                      {e.title || e.query}
                      <span className="text-white/50"> — {e.requestedBy}</span>
                    </span>
                    {(e.status === "downloading" || e.status === "analyzing") && (
                      <span className="shrink-0 animate-pulse text-base text-white/40">
                        preparing…
                      </span>
                    )}
                  </li>
                ))}
              </ol>
            )}
          </div>
        </div>

        <div className="flex shrink-0 flex-col items-center gap-4">
          <div className="rounded-2xl bg-white p-4">
            {/* Server-rendered QR of http://<this-host>/party. */}
            <img src="/qr" alt="Scan to join the party" width={240} height={240} />
          </div>
          <div className="text-center text-2xl font-semibold">Scan to join</div>
          <div className="text-center text-lg text-white/50">then add your songs</div>
        </div>
      </div>
    </div>
  );
};
