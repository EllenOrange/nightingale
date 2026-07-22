import { convertFileSrc } from "@/bridge/media";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";
import { ANALYSIS_STATUS_STYLES } from "@/lib/analysis-status-styles";
import { getLanguageName } from "@/lib/languages";
import type { QueuedStatus } from "@/types/QueuedStatus";
import type { Song } from "@/types/Song";
import { LoaderCircleIcon, MusicIcon, VideoIcon } from "lucide-react";
import { memo, type KeyboardEvent } from "react";

export function formatSeconds(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const secs = Math.floor(seconds) % 60;
  return `${minutes}:${secs.toString().padStart(2, "0")}`;
}

function formatTranscriptSource(source: Song["transcript_source"]): string {
  if (source === "Lyrics") return "Lyrics";
  if (source === "Usdx") return "USDX";
  return "Generated";
}

export type SongStatusInfo = {
  label: string;
  variant: "default" | "secondary" | "destructive" | "outline";
  className?: string;
  isAnalyzing?: boolean;
  isReady?: boolean;
};

export function getSongStatusInfo(isAnalyzed: boolean, queueStatus?: QueuedStatus): SongStatusInfo {
  if (queueStatus === "Queued") {
    return { label: "Queued", variant: "secondary", className: ANALYSIS_STATUS_STYLES.queued };
  }

  if (typeof queueStatus === "object") {
    if ("Analyzing" in queueStatus) {
      return {
        label: `Analyzing ${queueStatus.Analyzing}%`,
        variant: "default",
        className: `${ANALYSIS_STATUS_STYLES.analysing} animate-pulse`,
        isAnalyzing: true,
      };
    }
    if ("Failed" in queueStatus) return { label: "Failed", variant: "destructive" };
  }

  if (isAnalyzed) {
    return {
      label: "Analyzed",
      variant: "default",
      className: ANALYSIS_STATUS_STYLES.analysed,
      isReady: true,
    };
  }

  return { label: "Not analyzed", variant: "outline" };
}

function SongThumbnail({ song, className }: { song: Song; className?: string }) {
  return (
    <div
      className={cn(
        "relative shrink-0 overflow-hidden rounded-md bg-muted text-muted-foreground",
        className,
      )}
    >
      {song.album_art_path ? (
        <img
          src={convertFileSrc(song.album_art_path)}
          alt=""
          loading="lazy"
          decoding="async"
          className="size-full object-cover"
        />
      ) : (
        <MusicIcon className="absolute inset-0 m-auto size-5" aria-hidden="true" />
      )}
      {song.is_video ? (
        <VideoIcon className="absolute right-1 bottom-1 size-3 rounded-sm bg-background/85 p-0.5" />
      ) : null}
    </div>
  );
}

export function StatusBadge({ song, queueStatus }: { song: Song; queueStatus?: QueuedStatus }) {
  const status = getSongStatusInfo(song.is_analyzed, queueStatus);
  const source = status.isReady ? ` (${formatTranscriptSource(song.transcript_source)})` : "";

  return (
    <Badge variant={status.variant} className={cn("border-foreground/15", status.className)}>
      {status.isAnalyzing ? <LoaderCircleIcon className="animate-spin" /> : null}
      {status.label}
      {source}
    </Badge>
  );
}

export function LanguageBadge({ language }: { language?: string | null }) {
  if (!language) return null;

  const shortCode = language.slice(0, 2).toUpperCase();

  return (
    <span
      className="inline-grid size-5 shrink-0 place-items-center rounded-sm bg-foreground/8 p-0 text-center font-mono text-[0.5625rem] leading-none font-semibold tracking-tight text-muted-foreground ring-1 ring-foreground/10 ring-inset"
      title={getLanguageName(language)}
      aria-label={`Language: ${getLanguageName(language)}`}
    >
      {shortCode}
    </span>
  );
}

interface SongItemProps {
  song: Song;
  queueStatus?: QueuedStatus;
  index: number;
  isFocused: boolean;
  isSelected: boolean;
  onSelect: () => void;
}

export const SongTableRow = memo(
  ({ song, queueStatus, index, isFocused, isSelected, onSelect }: SongItemProps) => {
    const onKeyDown = (event: KeyboardEvent<HTMLTableRowElement>) => {
      if (event.key !== "Enter" && event.key !== " ") return;
      event.preventDefault();
      onSelect();
    };

    return (
      <tr
        tabIndex={0}
        data-song-index={index}
        aria-selected={isSelected}
        onClick={onSelect}
        onKeyDown={onKeyDown}
        className={cn(
          "cursor-pointer border-b border-border/70 outline-none [&>td]:bg-background [&>td]:transition-colors hover:[&>td]:bg-accent focus-visible:[&>td]:bg-primary/15",
          (isFocused || isSelected) && "[&>td]:bg-primary/15 hover:[&>td]:bg-primary/20",
        )}
      >
        <td className="song-table__thumbnail py-1.5 pr-2 pl-2">
          <SongThumbnail song={song} className="size-10" />
        </td>
        <td className="song-table__song px-2 py-2 align-middle font-medium">
          <div className="flex h-5 min-w-0 items-center gap-2">
            <span className="min-w-0 truncate leading-5">{song.title}</span>
            <LanguageBadge language={song.language} />
          </div>
        </td>
        <td className="song-table__band px-2 py-2 text-muted-foreground">
          <span className="block truncate">{song.artist || "—"}</span>
        </td>
        <td className="song-table__album px-2 py-2 text-muted-foreground">
          <span className="block truncate">{song.album || "—"}</span>
        </td>
        <td className="song-table__duration px-2 py-2 font-variant-numeric tabular-nums text-muted-foreground">
          {formatSeconds(song.duration_secs)}
        </td>
        <td className="song-table__status px-2 py-2 text-right">
          <StatusBadge song={song} queueStatus={queueStatus} />
        </td>
      </tr>
    );
  },
);

export const SongGridCard = memo(
  ({ song, queueStatus, index, isFocused, isSelected, onSelect }: SongItemProps) => (
    <button
      type="button"
      data-song-index={index}
      aria-pressed={isSelected}
      onClick={onSelect}
      className={cn(
        "group flex min-h-32 min-w-0 cursor-pointer items-start gap-3 rounded-lg border bg-card p-3 text-left outline-none transition-colors hover:border-ring hover:bg-muted/40 focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30",
        (isFocused || isSelected) && "border-ring bg-muted ring-2 ring-ring/30",
      )}
    >
      <SongThumbnail song={song} className="size-24" />
      <div className="flex min-w-0 flex-1 self-stretch flex-col py-0.5">
        <div className="line-clamp-2 text-sm leading-snug font-semibold">{song.title}</div>
        <p className="mt-1 truncate text-xs text-muted-foreground">{song.artist || "—"}</p>
        <p className="truncate text-xs text-muted-foreground">{song.album || "—"}</p>
        <div className="mt-auto flex flex-wrap items-center justify-between gap-2 pt-2">
          <span className="text-xs tabular-nums text-muted-foreground">
            {formatSeconds(song.duration_secs)}
          </span>
          <div className="flex items-center gap-1">
            <LanguageBadge language={song.language} />
            <StatusBadge song={song} queueStatus={queueStatus} />
          </div>
        </div>
      </div>
    </button>
  ),
);
