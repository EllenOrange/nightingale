/**
 * Guest page (/party): the phone UI for a house party. A guest sets their name,
 * searches the local library (or, for songs not in it, gets YouTube options
 * automatically), and manages the shared queue: add, reorder, remove, skip.
 * Mobile-first and standalone; it does not mount the desktop menu shell.
 *
 * Requirement 1 (multi-guest queue) lives here.
 */

import { useEffect, useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { loadSongs } from "@/bridge/songs";
import { partyQueueAdd, partyQueueRemove, partyQueueReorder, partySkip } from "@/bridge/party";
import { usePartyQueue } from "@/hooks/party/use-party-queue";
import { OnlineSearch } from "./online-search";
import type { Song } from "@/types/Song";
import type { QueueEntry, QueueStatus } from "@/types/party";

const NAME_KEY = "partyGuestName";

const emptyFilters = {
  artist: null,
  album: null,
  playlist: null,
  query: null,
  status: null,
  transcript_source: null,
  search: null,
};

const STATUS_LABEL: Record<QueueStatus, string> = {
  queued: "Queued",
  downloading: "Downloading",
  analyzing: "Preparing",
  ready: "Ready",
  playing: "Now playing",
  done: "Played",
  error: "Failed",
};

interface QueueRowProps {
  entry: QueueEntry;
  canMoveUp: boolean;
  canMoveDown: boolean;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onRemove: () => void;
  onSkip: () => void;
}

const QueueRow = ({
  entry,
  canMoveUp,
  canMoveDown,
  onMoveUp,
  onMoveDown,
  onRemove,
  onSkip,
}: QueueRowProps) => {
  const inFlight =
    entry.status === "downloading" || entry.status === "analyzing" || entry.status === "queued";
  const isPlaying = entry.status === "playing";

  return (
    <li className="flex items-center gap-2 rounded-lg border p-3">
      <div className="min-w-0 flex-1">
        <div className="truncate font-medium">{entry.title || entry.query || "Untitled"}</div>
        <div className="text-muted-foreground truncate text-xs">
          {entry.artist ? `${entry.artist} · ` : ""}
          {entry.requestedBy}
        </div>
        {entry.status === "error" && entry.error ? (
          <div className="text-destructive mt-1 truncate text-xs">{entry.error}</div>
        ) : null}
      </div>

      <span
        className={
          "shrink-0 rounded-full px-2 py-0.5 text-xs " +
          (isPlaying
            ? "bg-primary text-primary-foreground"
            : entry.status === "error"
              ? "bg-destructive/15 text-destructive"
              : "bg-muted text-muted-foreground")
        }
      >
        {inFlight ? (
          <span className="animate-pulse">{STATUS_LABEL[entry.status]}…</span>
        ) : (
          STATUS_LABEL[entry.status]
        )}
      </span>

      <div className="flex shrink-0 items-center gap-1">
        {isPlaying ? (
          <Button size="sm" variant="secondary" className="h-8 px-2" onClick={onSkip}>
            Skip
          </Button>
        ) : (
          <>
            <Button
              size="sm"
              variant="ghost"
              className="h-8 w-8 p-0"
              disabled={!canMoveUp}
              aria-label="Move up"
              onClick={onMoveUp}
            >
              ↑
            </Button>
            <Button
              size="sm"
              variant="ghost"
              className="h-8 w-8 p-0"
              disabled={!canMoveDown}
              aria-label="Move down"
              onClick={onMoveDown}
            >
              ↓
            </Button>
            <Button
              size="sm"
              variant="ghost"
              className="h-8 w-8 p-0"
              aria-label="Remove"
              onClick={onRemove}
            >
              ✕
            </Button>
          </>
        )}
      </div>
    </li>
  );
};

export const Party = () => {
  const [name, setName] = useState<string>(() => localStorage.getItem(NAME_KEY) ?? "");
  const [search, setSearch] = useState("");
  const [results, setResults] = useState<Song[]>([]);
  const [searching, setSearching] = useState(false);
  const [adding, setAdding] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const searchSeq = useRef(0);

  const queue = usePartyQueue();
  // The live part of the queue (drop entries that already played).
  const visibleQueue = useMemo(
    () => queue.entries.filter((e) => e.status !== "done"),
    [queue.entries],
  );

  const guestName = name.trim() || "Guest";

  // Persist the name as it is typed so it survives a reload and travels with
  // every add.
  useEffect(() => {
    localStorage.setItem(NAME_KEY, name);
  }, [name]);

  // Debounced local-library search.
  useEffect(() => {
    const term = search.trim();
    if (!term) {
      setResults([]);
      setSearching(false);
      return;
    }
    setSearching(true);
    const seq = ++searchSeq.current;
    const timer = setTimeout(() => {
      loadSongs({ search: term, filters: emptyFilters, skip: 0, take: 20 })
        .then((store) => {
          if (seq === searchSeq.current) setResults(store.processed);
        })
        .catch(() => {})
        .finally(() => {
          if (seq === searchSeq.current) setSearching(false);
        });
    }, 300);
    return () => clearTimeout(timer);
  }, [search]);

  const flash = (msg: string) => {
    setNotice(msg);
    setTimeout(() => setNotice((cur) => (cur === msg ? null : cur)), 2500);
  };

  const resetSearch = () => {
    setSearch("");
    setResults([]);
  };

  const addLibrarySong = async (song: Song) => {
    setAdding(song.file_hash);
    try {
      await partyQueueAdd({ fileHash: song.file_hash, requestedBy: guestName });
      flash(`Added "${song.title}"`);
      resetSearch();
    } catch (e) {
      flash(`Could not add: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setAdding(null);
    }
  };

  // Reorder against the full queue array so indices stay correct even with
  // played/errored entries interleaved.
  const move = (id: string, dir: -1 | 1) => {
    const visibleIds = visibleQueue.map((e) => e.id);
    const vi = visibleIds.indexOf(id);
    const neighborId = visibleIds[vi + dir];
    if (!neighborId) return;
    const targetFullIdx = queue.entries.findIndex((e) => e.id === neighborId);
    if (targetFullIdx >= 0) void partyQueueReorder(id, targetFullIdx);
  };

  const showOnline = Boolean(search.trim()) && !searching && results.length === 0;

  return (
    <div className="mx-auto flex min-h-dvh max-w-md flex-col gap-4 p-4 pb-[max(1rem,env(safe-area-inset-bottom))]">
      <header>
        <h1 className="text-xl font-semibold">Party queue</h1>
      </header>

      <label className="flex items-center gap-2 text-sm">
        <span className="text-muted-foreground shrink-0">Your name</span>
        <Input
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Add your name so songs are yours"
          maxLength={40}
          className="h-9"
        />
      </label>

      <div className="flex flex-col gap-2">
        <Input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search your library or any song"
          inputMode="search"
        />
        {search.trim() && (
          <div className="flex flex-col gap-2">
            {results.map((song) => (
              <button
                key={song.file_hash}
                className="hover:bg-accent flex items-center gap-3 rounded-lg border p-3 text-left disabled:opacity-50"
                disabled={adding === song.file_hash}
                onClick={() => addLibrarySong(song)}
              >
                <div className="min-w-0 flex-1">
                  <div className="truncate font-medium">{song.title}</div>
                  <div className="text-muted-foreground truncate text-xs">{song.artist}</div>
                </div>
                <span className="text-primary shrink-0 text-sm">Add</span>
              </button>
            ))}

            {/* No local match: offer YouTube options automatically. */}
            {showOnline && (
              <OnlineSearch
                query={search}
                requestedBy={guestName}
                onAdded={(label) => {
                  flash(`Requested "${label}"`);
                  resetSearch();
                }}
              />
            )}
          </div>
        )}
      </div>

      {notice && (
        <div className="bg-muted text-muted-foreground rounded-md px-3 py-2 text-sm">{notice}</div>
      )}

      <section className="flex min-h-0 flex-1 flex-col gap-2">
        <h2 className="text-muted-foreground text-sm font-medium">
          Up next ({visibleQueue.length})
        </h2>
        {visibleQueue.length === 0 ? (
          <p className="text-muted-foreground text-sm">Nothing queued yet. Add the first song!</p>
        ) : (
          <ul className="flex flex-col gap-2">
            {visibleQueue.map((entry, i) => (
              <QueueRow
                key={entry.id}
                entry={entry}
                canMoveUp={i > 0}
                canMoveDown={i < visibleQueue.length - 1}
                onMoveUp={() => move(entry.id, -1)}
                onMoveDown={() => move(entry.id, 1)}
                onRemove={() => void partyQueueRemove(entry.id)}
                onSkip={() => void partySkip()}
              />
            ))}
          </ul>
        )}
      </section>
    </div>
  );
};
