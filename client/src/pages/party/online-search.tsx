/**
 * Online song search: find a song that is NOT in the local library, in one
 * step. As soon as it is shown (the local search came up empty), it looks the
 * query up in LRCLIB to confirm lyrics exist and get the canonical artist/title,
 * then lists YouTube videos to choose from. Picking a video downloads it and
 * enqueues it carrying the canonical artist/title, so the analyzer's own LRCLIB
 * match is guaranteed.
 *
 * Kept to a single visible step (type -> pick a video) on purpose: the LRCLIB
 * lookup happens automatically and the top lyric match supplies the metadata.
 */

import { useEffect, useState } from "react";
import { partyQueueAdd, partySearchLrclib, partyYoutubeCandidates } from "@/bridge/party";
import type { LrclibSearchResult, YoutubeCandidate } from "@/types/party";

const DEBOUNCE_MS = 500;

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
  const [loading, setLoading] = useState(false);
  const [canonical, setCanonical] = useState<LrclibSearchResult | null>(null);
  const [videos, setVideos] = useState<YoutubeCandidate[]>([]);
  const [noLyrics, setNoLyrics] = useState(false);
  const [adding, setAdding] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const term = query.trim();
    setCanonical(null);
    setVideos([]);
    setNoLyrics(false);
    setError(null);
    if (!term) {
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);
    const timer = setTimeout(async () => {
      try {
        // Step 1 (automatic): confirm lyrics exist and get canonical metadata.
        const tracks = await partySearchLrclib(term);
        if (cancelled) return;
        if (tracks.length === 0) {
          setNoLyrics(true);
          setLoading(false);
          return;
        }
        const top = tracks[0];
        setCanonical(top);

        // Step 2 (automatic): list videos to choose from.
        const vids = await partyYoutubeCandidates(`${top.artist_name} ${top.track_name}`);
        if (cancelled) return;
        setVideos(vids);
      } catch (e) {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }, DEBOUNCE_MS);

    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [query]);

  const addVideo = async (video: YoutubeCandidate) => {
    if (!canonical) return;
    setAdding(video.videoId);
    try {
      await partyQueueAdd({
        query: video.url,
        title: canonical.track_name,
        artist: canonical.artist_name,
        requestedBy,
      });
      onAdded(`${canonical.artist_name} - ${canonical.track_name}`);
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

  if (loading) {
    return <div className="text-muted-foreground py-4 text-center text-sm">Searching online…</div>;
  }

  if (noLyrics) {
    return (
      <p className="text-muted-foreground text-sm">
        Not found in the lyrics database. Try including the artist, or check the spelling.
      </p>
    );
  }

  if (!canonical) {
    return null;
  }

  return (
    <div className="flex flex-col gap-2">
      <div className="text-muted-foreground text-xs">
        Lyrics:{" "}
        <span className="text-foreground font-medium">
          {canonical.artist_name} - {canonical.track_name}
        </span>
        {" · pick a video to add:"}
      </div>
      {videos.length === 0 ? (
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
};
