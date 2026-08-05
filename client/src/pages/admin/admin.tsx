/**
 * Admin page (/admin): the host's control panel. Transport (pause/resume, skip,
 * restart), live audio (guide vocal, master volume), and queue management
 * (reorder, remove, retry, clear).
 *
 * No authentication by design: this is a home-LAN convenience surface, not a
 * security boundary (PHASE1_PLAN.md Step 4). It is a controller route, so the
 * remote-playback hook never navigates this tab into the video.
 */

import { Button } from "@/components/ui/button";
import {
  partyPause,
  partyQueueAdd,
  partyQueueClear,
  partyQueueRemove,
  partyQueueReorder,
  partyRestart,
  partyResume,
  partySetGuideVocal,
  partySetVolume,
  partySkip,
} from "@/bridge/party";
import { usePartyQueue } from "@/hooks/party/use-party-queue";
import { useJukebox } from "@/hooks/party/use-jukebox";
import type { QueueEntry } from "@/types/party";

const Section = ({ title, children }: { title: string; children: React.ReactNode }) => (
  <section className="flex flex-col gap-2 rounded-lg border p-3">
    <h2 className="text-muted-foreground text-sm font-medium">{title}</h2>
    {children}
  </section>
);

const Slider = ({
  label,
  value,
  onCommit,
}: {
  label: string;
  value: number;
  onCommit: (v: number) => void;
}) => (
  <label className="flex items-center gap-3 text-sm">
    <span className="w-24 shrink-0">{label}</span>
    <input
      type="range"
      min={0}
      max={100}
      value={Math.round(value * 100)}
      onChange={(e) => onCommit(Number(e.target.value) / 100)}
      className="flex-1"
    />
    <span className="text-muted-foreground w-10 text-right tabular-nums">
      {Math.round(value * 100)}%
    </span>
  </label>
);

const AdminQueueRow = ({
  entry,
  index,
  count,
}: {
  entry: QueueEntry;
  index: number;
  count: number;
}) => (
  <li className="flex items-center gap-2 rounded-md border p-2">
    <div className="min-w-0 flex-1">
      <div className="truncate text-sm font-medium">{entry.title || entry.query || "Untitled"}</div>
      <div className="text-muted-foreground truncate text-xs">
        {entry.status}
        {entry.requestedBy ? ` · ${entry.requestedBy}` : ""}
        {entry.error ? ` · ${entry.error}` : ""}
      </div>
    </div>
    <div className="flex shrink-0 gap-1">
      <Button
        size="sm"
        variant="ghost"
        disabled={index === 0}
        onClick={() => partyQueueReorder(entry.id, index - 1)}
      >
        ↑
      </Button>
      <Button
        size="sm"
        variant="ghost"
        disabled={index === count - 1}
        onClick={() => partyQueueReorder(entry.id, index + 1)}
      >
        ↓
      </Button>
      {entry.status === "error" && entry.query ? (
        <Button
          size="sm"
          variant="secondary"
          onClick={() => partyQueueAdd({ query: entry.query!, requestedBy: entry.requestedBy })}
        >
          Retry
        </Button>
      ) : null}
      <Button size="sm" variant="ghost" onClick={() => partyQueueRemove(entry.id)}>
        ✕
      </Button>
    </div>
  </li>
);

export const Admin = () => {
  const queue = usePartyQueue();
  const jukebox = useJukebox();

  const guide = jukebox?.guide_vocal ?? 0.3;
  const volume = jukebox?.volume ?? 1;
  const paused = jukebox?.paused ?? false;

  // Render the full array (including played entries) so reorder indices line up
  // exactly with the server's positions; played entries stay visible as history.
  const entries = queue.entries;

  return (
    <div className="mx-auto flex min-h-dvh max-w-xl flex-col gap-4 p-4">
      <h1 className="text-xl font-semibold">Admin</h1>

      <Section title="Transport">
        <div className="flex flex-wrap gap-2">
          {paused ? (
            <Button onClick={() => partyResume()}>Resume</Button>
          ) : (
            <Button onClick={() => partyPause()}>Pause</Button>
          )}
          <Button variant="secondary" onClick={() => partyRestart()}>
            Restart
          </Button>
          <Button variant="secondary" onClick={() => partySkip()}>
            Skip
          </Button>
        </div>
      </Section>

      <Section title="Audio">
        <Slider label="Guide vocal" value={guide} onCommit={(v) => partySetGuideVocal(v)} />
        <Slider label="Volume" value={volume} onCommit={(v) => partySetVolume(v)} />
      </Section>

      <Section title={`Queue (${entries.length})`}>
        {entries.length === 0 ? (
          <p className="text-muted-foreground text-sm">Queue is empty.</p>
        ) : (
          <ul className="flex flex-col gap-2">
            {entries.map((entry, i) => (
              <AdminQueueRow key={entry.id} entry={entry} index={i} count={entries.length} />
            ))}
          </ul>
        )}
        {entries.length > 0 && (
          <Button
            variant="ghost"
            className="text-destructive self-start"
            onClick={() => partyQueueClear()}
          >
            Clear all
          </Button>
        )}
      </Section>
    </div>
  );
};
