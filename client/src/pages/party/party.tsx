/**
 * Guest page (/party): the phone UI for a house party. A guest sets a name,
 * searches the local library or requests a song from YouTube, and watches the
 * shared queue update live. Mobile-first and standalone: it does not mount the
 * desktop menu shell.
 *
 * Requirement 1 (multi-guest queue) lives here.
 */

import { useEffect, useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { loadSongs } from "@/bridge/songs";
import { partyQueueAdd } from "@/bridge/party";
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

const NameGate = ({ onSet }: { onSet: (name: string) => void }) => {
  const [value, setValue] = useState("");
  return (
    <div className="mx-auto flex min-h-dvh max-w-md flex-col justify-center gap-4 p-6">
      <h1 className="text-2xl font-semibold">Join the party</h1>
      <p className="text-muted-foreground text-sm">What should we call you on the queue?</p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          const name = value.trim();
          if (name) onSet(name);
        }}
        className="flex gap-2"
      >
        <Input
          autoFocus
          value={value}
          onChange={(e) => setValue(e.target.value)}
          placeholder="Your name"
          maxLength={40}
        />
        <Button type="submit" disabled={!value.trim()}>
          Join
        </Button>
      </form>
    </div>
  );
};

const QueueRow = ({ entry }: { entry: QueueEntry }) => {
  const inFlight =
    entry.status === "downloading" || entry.status === "analyzing" || entry.status === "queued";
  return (
    <li className="flex items-center gap-3 rounded-lg border p-3">
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
          (entry.status === "playing"
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
    </li>
  );
};

export const Party = () => {
  const [name, setName] = useState<string | null>(() => localStorage.getItem(NAME_KEY));
  const [search, setSearch] = useState("");
  const [results, setResults] = useState<Song[]>([]);
  const [searching, setSearching] = useState(false);
  const [adding, setAdding] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [showOnline, setShowOnline] = useState(false);
  const searchSeq = useRef(0);

  const queue = usePartyQueue();
  // Show the live part of the queue (drop entries that already played).
  const visibleQueue = useMemo(
    () => queue.entries.filter((e) => e.status !== "done"),
    [queue.entries],
  );

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

  const persistName = (n: string) => {
    localStorage.setItem(NAME_KEY, n);
    setName(n);
  };

  const flash = (msg: string) => {
    setNotice(msg);
    setTimeout(() => setNotice((cur) => (cur === msg ? null : cur)), 2500);
  };

  const addLibrarySong = async (song: Song) => {
    setAdding(song.file_hash);
    try {
      await partyQueueAdd({ fileHash: song.file_hash, requestedBy: name ?? "Guest" });
      flash(`Added "${song.title}"`);
      resetSearch();
    } catch (e) {
      flash(`Could not add: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setAdding(null);
    }
  };

  const resetSearch = () => {
    setSearch("");
    setResults([]);
    setShowOnline(false);
  };

  if (!name) {
    return <NameGate onSet={persistName} />;
  }

  return (
    <div className="mx-auto flex min-h-dvh max-w-md flex-col gap-4 p-4 pb-[max(1rem,env(safe-area-inset-bottom))]">
      <header className="flex items-center justify-between">
        <h1 className="text-xl font-semibold">Party queue</h1>
        <button className="text-muted-foreground text-xs underline" onClick={() => persistName("")}>
          {name}
        </button>
      </header>

      <div className="flex flex-col gap-2">
        <Input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search your library or a song to request"
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
            {!searching && results.length === 0 && (
              <p className="text-muted-foreground px-1 text-sm">Not in your library.</p>
            )}

            {showOnline ? (
              <OnlineSearch
                query={search}
                requestedBy={name ?? "Guest"}
                onAdded={(label) => {
                  flash(`Requested "${label}"`);
                  resetSearch();
                }}
              />
            ) : (
              <Button variant="secondary" onClick={() => setShowOnline(true)}>
                Search online for “{search.trim()}”
              </Button>
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
            {visibleQueue.map((entry) => (
              <QueueRow key={entry.id} entry={entry} />
            ))}
          </ul>
        )}
      </section>
    </div>
  );
};
