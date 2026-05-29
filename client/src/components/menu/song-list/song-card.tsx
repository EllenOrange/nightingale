import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Item, ItemContent, ItemDescription, ItemMedia, ItemTitle } from "@/components/ui/item";
import { Stars } from "@/components/shared/stars";
import { useAnalysis } from "@/hooks/use-analysis";
import type { QueuedStatus } from "@/types/QueuedStatus";
import type { Song } from "@/types/Song";
import { convertFileSrc } from "@/bridge/media";
import {
  AlignLeftIcon,
  AudioLinesIcon,
  LanguagesIcon,
  LoaderCircleIcon,
  MenuIcon,
  MicIcon,
  MusicIcon,
  PencilLineIcon,
  RefreshCwIcon,
  Trash2Icon,
  VideoIcon,
} from "lucide-react";
import { memo, MouseEvent, useState } from "react";
import { useNavigate } from "react-router";
import { toast } from "sonner";
import { cn } from "@/lib/utils";
import { useDialog } from "@/hooks/use-dialog";
import { Shifts, ShiftType } from "./shifts";
import { useQueryClient } from "@tanstack/react-query";
import { SONGS } from "@/queries/keys";

function formatSeconds(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  const secs = Math.floor(seconds) % 60;

  return `${minutes}:${secs.toString().padStart(2, "0")}`;
}

function formatTranscriptSource(source: Song["transcript_source"]): string {
  switch (source) {
    case "Lyrics":
      return "Lyrics";
    case "Usdx":
      return "USDX";
    default:
      return "Generated";
  }
}

type StatusInfo = {
  label: string;
  variant: "default" | "secondary" | "destructive" | "outline";
  className?: string;
  isAnalyzing?: boolean;
  isReady?: boolean;
};

function getStatusInfo(isAnalyzed: boolean, queueStatus?: QueuedStatus): StatusInfo {
  if (queueStatus) {
    if (queueStatus === "Queued") {
      return { label: "Queued", variant: "secondary" };
    }

    if (typeof queueStatus === "object") {
      if ("Analyzing" in queueStatus) {
        return {
          label: `Analyzing ${queueStatus.Analyzing}%`,
          variant: "default",
          className: "animate-pulse",
          isAnalyzing: true,
        };
      }

      if ("Failed" in queueStatus) {
        return { label: "Failed", variant: "destructive" };
      }
    }
  }

  if (isAnalyzed) {
    return {
      label: "Analyzed",
      variant: "default",
      className: "bg-green-600 text-white",
      isReady: true,
    };
  }

  return { label: "Not Analyzed", variant: "outline" };
}

interface SongCardProps {
  song: Song;
  queueStatus?: QueuedStatus;
  bestScore?: number;
  index: number;
  isFocused: boolean;
}

export const SongCard = memo(
  ({ song, queueStatus, bestScore, index, isFocused }: SongCardProps) => {
    const [shifting, setShifting] = useState<Record<ShiftType, boolean>>({
      tempo: false,
      key: false,
    });

    const navigate = useNavigate();
    const { setMode } = useDialog();
    const queryClient = useQueryClient();
    const {
      enqueueOne,
      deleteSongCache,
      reanalyzeFull,
      reanalyzeTranscript,
      realign,
      reanalyzeForceTranscribe,
    } = useAnalysis();
    const { label, variant, className, isAnalyzing, isReady } = getStatusInfo(
      song.is_analyzed,
      queueStatus,
    );

    const isUsdx = song.transcript_source === "Usdx";
    const displaySource = isReady ? ` (${formatTranscriptSource(song.transcript_source)})` : "";

    const disabled = shifting.tempo || shifting.key;

    const setShiftStatus = (type: ShiftType, isShifting: boolean) => {
      setShifting((prev) => ({ ...prev, [type]: isShifting }));
    };

    const withMenuAction = (action: () => void | Promise<void>) => async (e: MouseEvent) => {
      e.stopPropagation();
      await action();
    };

    return (
      <Item
        variant="outline"
        role="listitem"
        data-song-index={index}
        className={cn(
          "flex flex-nowrap cursor-pointer items-start gap-2 bg-card transition-colors hover:ring-primary focus-visible:ring-0 focus-visible:border-border sm:items-center",
          isFocused && "ring-2 ring-primary bg-muted",
          disabled && "bd-muted",
        )}
        onClick={() => {
          if (disabled) {
            return;
          }

          if (isReady) {
            return navigate("/playback", { state: { song } });
          }

          enqueueOne(song.file_hash);
        }}
      >
        <ItemMedia variant="image" className="size-14 sm:size-16">
          {song.album_art_path ? (
            <img
              src={convertFileSrc(song.album_art_path)}
              alt={song.title}
              loading="lazy"
              decoding="async"
            />
          ) : (
            <MusicIcon />
          )}
        </ItemMedia>

        <ItemContent className="min-w-0">
          {song.is_video && (
            <Badge variant="outline">
              <VideoIcon /> Video
            </Badge>
          )}
          <ItemTitle className="flex min-w-0 flex-row flex-wrap items-center gap-2">
            <span className="line-clamp-1 min-w-0">{song.title}</span>
            {bestScore != null ? <Stars score={bestScore} size="sm" className="shrink-0" /> : null}
          </ItemTitle>
          <ItemDescription>
            {song.artist} &bull; {song.album} &bull; {formatSeconds(song.duration_secs)}
            {song.language ? ` • ${song.language.toUpperCase()}` : ""}
          </ItemDescription>
        </ItemContent>

        <ItemContent className="min-w-0 flex-none flex-col items-end gap-1">
          <div className="flex items-center gap-1">
            <Badge variant={variant} className={className}>
              {isAnalyzing && <LoaderCircleIcon className="size-3 animate-spin" />}
              {label}
              {displaySource}
            </Badge>
          </div>
          {!isUsdx && isReady && (
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button variant="outline" size="xs" disabled={disabled}>
                  <MenuIcon /> Actions
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent side="bottom" align="start" className="min-w-56">
                <DropdownMenuLabel>Analysis</DropdownMenuLabel>
                <DropdownMenuGroup>
                  <DropdownMenuItem
                    onClick={withMenuAction(async () => {
                      realign(song.file_hash);
                      toast.info(`Realigning "${song.title}"`);
                    })}
                  >
                    <AlignLeftIcon />
                    Realign
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onClick={withMenuAction(async () => {
                      reanalyzeTranscript(song.file_hash);
                      toast.info(`Refetching lyrics & aligning "${song.title}"`);
                    })}
                  >
                    <RefreshCwIcon />
                    Refetch lyrics & align
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onClick={withMenuAction(async () => {
                      reanalyzeForceTranscribe(song.file_hash);
                      toast.info(`Force transcribing "${song.title}"`);
                    })}
                  >
                    <MicIcon />
                    Force transcribe
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onClick={withMenuAction(async () => {
                      reanalyzeFull(song.file_hash);
                      toast.info(`Full reanalysis (w/ stems) for "${song.title}"`);
                    })}
                  >
                    <AudioLinesIcon />
                    Full reanalysis (w/ stems)
                  </DropdownMenuItem>
                </DropdownMenuGroup>
                <DropdownMenuSeparator />
                <DropdownMenuLabel>Manage</DropdownMenuLabel>
                <DropdownMenuGroup>
                  <DropdownMenuItem
                    onClick={withMenuAction(async () => {
                      setMode({ mode: "edit-lyrics", song });
                    })}
                  >
                    <PencilLineIcon />
                    Edit lyrics
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    onClick={withMenuAction(async () => {
                      setMode({ mode: "language", song });
                    })}
                  >
                    <LanguagesIcon />
                    Change language
                  </DropdownMenuItem>
                </DropdownMenuGroup>
                <DropdownMenuSeparator />
                <DropdownMenuGroup>
                  <DropdownMenuItem
                    onClick={withMenuAction(async () => {
                      await deleteSongCache(song.file_hash);
                      toast.info(`Cache deleted for "${song.title}"`);
                    })}
                  >
                    <Trash2Icon />
                    Delete cache
                  </DropdownMenuItem>
                </DropdownMenuGroup>
              </DropdownMenuContent>
            </DropdownMenu>
          )}
        </ItemContent>
        <Shifts
          song={song}
          status={shifting}
          onStart={(type: ShiftType) => {
            setShiftStatus(type, true);
          }}
          onSuccess={(message, type: ShiftType) => {
            toast.success(message);
            queryClient.invalidateQueries({ queryKey: SONGS });
            setShiftStatus(type, false);
          }}
          onError={(message, type: ShiftType) => {
            toast.error(message);
            setShiftStatus(type, false);
          }}
        />
      </Item>
    );
  },
);
