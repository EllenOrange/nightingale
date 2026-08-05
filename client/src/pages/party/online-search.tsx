/**
 * Online song search: find a song that is NOT in the local library.
 *
 * Two steps, gated on lyrics so guests can only pick songs that will karaoke
 * well: (1) find the track in LRCLIB (confirms lyrics exist, gives the
 * canonical artist/title), then (2) choose which YouTube video to download.
 * The chosen video is enqueued carrying the canonical artist/title so the
 * analyzer's own LRCLIB match is guaranteed.
 */

import { useEffect, useState } from "react";
import { partyQueueAdd, partySearchLrclib, partyYoutubeCandidates } from "@/bridge/party";
import type { LrclibSearchResult, YoutubeCandidate } from "@/types/party";

const fmtDuration = (secs: number | null | undefined): string => {
  if (secs == null || !Number.isFinite(secs)) return "";
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${String(s).padStart(2, "0")}`;
};

interface OnlineSearchProps {
  query: string;
  requestedBy: string;
  onAdded: (label: string) => void;
}

export const OnlineSearch = ({ query, requestedBy, onAdded }: OnlineSearchProps) => {
  const [tracks, setTracks] = useState<LrclibSearchResult[] | null>(null);
  const [selected, setSelected] = useState<LrclibSearchResult | null>(null);
  const [videos, setVideos] = useState<YoutubeCandidate[] | null>(null);
  const [adding, setAdding] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Step 1: LRCLIB search whenever the query changes.
  useEffect(() => {
    const term = query.trim();
    setSelected(null);
    setVideos(null);
    setError(null);
    if (!term) {
      setTracks(null);
      return;
    }
    let cancelled = false;
    setTracks(null);
    partySearchLrclib(term)
      .then((r) => {
        if (!cancelled) setTracks(r);
      })
      .catch((e) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [query]);

  // Step 2: YouTube candidates for the chosen track.
  useEffect(() => {
    if (!selected) {
      setVideos(null);
      return;
    }
    let cancelled = false;
    setVideos(null);
    setError(null);
    partyYoutubeCandidates(`${selected.artist_name} ${selected.track_name}`)
      .then((r) => {
        if (!cancelled) setVideos(r);
      })
      .catch((e) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      cancelled = true;
    };
  }, [selected]);

  const addVideo = async (video: YoutubeCandidate) => {
    if (!selected) return;
    setAdding(video.videoId);
    try {
      await partyQueueAdd({
        query: video.url,
        title: selected.track_name,
        artist: selected.artist_name,
        requestedBy,
      });
      onAdded(`${selected.artist_name} - ${selected.track_name}`);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setAdding(null);
    }
  };

  if (error) {
    return (
      <div className="text-destructive rounded-lg border p-3 text-sm">
        {error}
        <button className="ml-2 underline" onClick={() => setError(null)}>
          dismiss
        </button>
      </div>
    );
  }

  // ── Step 2 UI: pick a video ──────────────────────────────────────────────
  if (selected) {
    return (
      <div className="flex flex-col gap-2">
        <button
          className="text-muted-foreground self-start text-xs underline"
          onClick={() => setSelected(null)}
        >
          ← back to matches
        </button>
        <div className="text-sm font-medium">
          {selected.artist_name} - {selected.track_name}
        </div>
        <div className="text-muted-foreground text-xs">Choose a video to download:</div>
        {videos === null ? (
          <div className="text-muted-foreground py-4 text-center text-sm">Searching YouTube…</div>
        ) : videos.length === 0 ? (
          <p className="text-muted-foreground text-sm">No videos found.</p>
        ) : (
          videos.map((v) => (
            <button
              key={v.videoId}
              className="hover:bg-accent flex items-center gap-3 rounded-lg border p-2 text-left disabled:opacity-50"
              disabled={adding !== null}
              onClick={() => addVideo(v)}
            >
              <img
                src={v.thumbnail}
                alt=""
                width={80}
                height={45}
                className="h-[45px] w-[80px] shrink-0 rounded object-cover"
                onError={(e) => {
                  e.currentTarget.style.visibility = "hidden";
                }}
              />
              <div className="min-w-0 flex-1">
                <div className="truncate text-sm font-medium">{v.title}</div>
                <div className="text-muted-foreground truncate text-xs">
                  {v.channel}
                  {v.durationSecs ? ` · ${fmtDuration(v.durationSecs)}` : ""}
                </div>
              </div>
              <span className="text-primary shrink-0 text-sm">
                {adding === v.videoId ? "Adding…" : "Add"}
              </span>
            </button>
          ))
        )}
      </div>
    );
  }

  // ── Step 1 UI: pick the song from LRCLIB ─────────────────────────────────
  return (
    <div className="flex flex-col gap-2">
      {tracks === null ? (
        <div className="text-muted-foreground py-4 text-center text-sm">
          Searching lyrics database…
        </div>
      ) : tracks.length === 0 ? (
        <p className="text-muted-foreground text-sm">
          No lyrics found for that. Try a different spelling, or include the artist.
        </p>
      ) : (
        tracks.slice(0, 8).map((t, i) => (
          <button
            key={`${t.artist_name}-${t.track_name}-${i}`}
            className="hover:bg-accent flex items-center gap-3 rounded-lg border p-3 text-left"
            onClick={() => setSelected(t)}
          >
            <div className="min-w-0 flex-1">
              <div className="truncate font-medium">{t.track_name}</div>
              <div className="text-muted-foreground truncate text-xs">
                {t.artist_name}
                {t.album_name ? ` · ${t.album_name}` : ""}
                {t.duration_secs ? ` · ${fmtDuration(t.duration_secs)}` : ""}
              </div>
            </div>
            {t.has_synced && (
              <span className="bg-muted text-muted-foreground shrink-0 rounded-full px-2 py-0.5 text-xs">
                synced
              </span>
            )}
          </button>
        ))
      )}
    </div>
  );
};
