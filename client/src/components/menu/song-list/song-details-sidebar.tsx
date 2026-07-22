import { convertFileSrc } from "@/bridge/media";
import { Button } from "@/components/ui/button";
import { Separator } from "@/components/ui/separator";
import { useMenuFocus } from "@/contexts/menu-focus-context";
import { useAnalysis } from "@/hooks/use-analysis";
import { useDialog } from "@/hooks/use-dialog";
import { DIALOG_FOCUSABLE_SELECTOR, useDialogNav } from "@/hooks/navigation/use-dialog-nav";
import { SONGS } from "@/queries/keys";
import type { QueuedStatus } from "@/types/QueuedStatus";
import type { Song } from "@/types/Song";
import { useQueryClient } from "@tanstack/react-query";
import {
  AlignLeftIcon,
  AudioLinesIcon,
  LanguagesIcon,
  type LucideIcon,
  MicIcon,
  MusicIcon,
  PencilLineIcon,
  PlayIcon,
  RefreshCwIcon,
  Trash2Icon,
  XIcon,
} from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router";
import { toast } from "sonner";
import { formatSeconds, getSongStatusInfo, LanguageBadge, StatusBadge } from "./song-card";
import { Shifts, type ShiftType } from "./shifts";

const NAV_GROUP_SELECTOR = "[data-song-details-nav-group]";

interface NavigationRow {
  size: number;
  horizontal: boolean;
}

function getDetailsFocusables(container: HTMLElement | null): HTMLElement[] {
  if (!container) return [];
  return Array.from(container.querySelectorAll<HTMLElement>(DIALOG_FOCUSABLE_SELECTOR)).filter(
    (element) => element.offsetWidth > 0 || element.offsetHeight > 0,
  );
}

function getNavigationRows(focusables: HTMLElement[]): NavigationRow[] {
  const rows: NavigationRow[] = [];
  let previousGroup: Element | null = null;

  focusables.forEach((element) => {
    const group = element.closest(NAV_GROUP_SELECTOR);
    if (group && group === previousGroup) rows[rows.length - 1].size += 1;
    else rows.push({ size: 1, horizontal: group !== null });
    previousGroup = group;
  });

  return rows.length > 0 ? rows : [{ size: 1, horizontal: false }];
}

interface SongDetailsSidebarProps {
  song: Song;
  queueStatus?: QueuedStatus;
  onClose: () => void;
}

interface ActionItemProps {
  icon: LucideIcon;
  title: string;
  description: string;
  onClick: () => void | Promise<void>;
  disabled?: boolean;
  destructive?: boolean;
}

const ActionItem = ({
  icon: Icon,
  title,
  description,
  onClick,
  disabled,
  destructive,
}: ActionItemProps) => (
  <Button
    type="button"
    variant={destructive ? "destructive" : "ghost"}
    size="lg"
    className="h-auto min-h-10 w-full items-start justify-start gap-2 px-2 py-1.5 text-left whitespace-normal"
    disabled={disabled}
    onClick={onClick}
  >
    <Icon className="mt-0.5 size-4" />
    <span className="min-w-0">
      <span className="block text-xs font-medium leading-tight">{title}</span>
      <span
        className={
          destructive
            ? "mt-0.5 block text-[0.625rem] leading-tight text-destructive/70"
            : "mt-0.5 block text-[0.625rem] leading-tight text-muted-foreground"
        }
      >
        {description}
      </span>
    </span>
  </Button>
);

export const SongDetailsSidebar = ({ song, queueStatus, onClose }: SongDetailsSidebarProps) => {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { focus, setFocus, actionsRef } = useMenuFocus();
  const { mode, setMode } = useDialog();
  const detailsRef = useRef<HTMLElement>(null);
  const {
    enqueueOne,
    deleteSongCache,
    reanalyzeFull,
    reanalyzeTranscript,
    realign,
    reanalyzeForceTranscribe,
  } = useAnalysis();
  const [shifting, setShifting] = useState<Record<ShiftType, boolean>>({
    tempo: false,
    key: false,
  });
  const [navigationRows, setNavigationRows] = useState<NavigationRow[]>([
    { size: 1, horizontal: false },
  ]);
  const status = getSongStatusInfo(song.is_analyzed, queueStatus);
  const analysisBusy = queueStatus === "Queued" || status.isAnalyzing;
  const supportsShifts = song.is_analyzed && song.transcript_source !== "Usdx";
  const adjustmentSectionClass = supportsShifts ? "px-4 pt-4 pb-2" : "px-4 py-4";
  const supportsAnalysisActions = status.isReady && song.transcript_source !== "Usdx";

  const closeDetails = () => {
    setFocus((previous) => ({
      ...previous,
      active: true,
      panel: "songList",
      source: "nav",
    }));
    onClose();
  };

  const detailsPanelOpen = mode === null && focus.panel === "songDetails";
  const navigationStops = useMemo(() => navigationRows.map((row) => row.size), [navigationRows]);
  const focusableCount = useMemo(
    () => navigationRows.reduce((total, row) => total + row.size, 0),
    [navigationRows],
  );
  const { focusedIndex } = useDialogNav({
    open: detailsPanelOpen,
    itemCount: focusableCount,
    stops: navigationStops,
    containerRef: detailsRef,
    onBack: closeDetails,
    onAction: (segment, slot, action) => {
      setFocus((previous) => ({ ...previous, active: true, source: "nav" }));

      const horizontalRow = navigationRows[segment]?.horizontal ?? false;
      if (horizontalRow && action.right) return false;
      if (horizontalRow && action.left && slot > 0) return false;

      if (action.left) {
        setFocus((previous) => ({
          ...previous,
          active: true,
          panel: "songList",
          source: "nav",
        }));
        if (!window.matchMedia("(min-width: 1280px)").matches) onClose();
        return true;
      }

      return action.right;
    },
  });

  useEffect(() => {
    actionsRef.current.hasSongDetails = true;
    return () => {
      actionsRef.current.hasSongDetails = false;
      setFocus((previous) => {
        if (previous.panel !== "songDetails") return previous;
        return { ...previous, panel: "songList", active: true, source: "nav" };
      });
    };
  }, [actionsRef, setFocus]);

  useLayoutEffect(() => {
    const nextRows = getNavigationRows(getDetailsFocusables(detailsRef.current));
    setNavigationRows((currentRows) => {
      const unchanged =
        currentRows.length === nextRows.length &&
        currentRows.every(
          (row, index) =>
            row.size === nextRows[index].size && row.horizontal === nextRows[index].horizontal,
        );
      return unchanged ? currentRows : nextRows;
    });
  });

  useEffect(() => {
    const focusables = getDetailsFocusables(detailsRef.current);
    focusables.forEach((element) => delete element.dataset.songDetailsFocused);
    if (!focus.active || !detailsPanelOpen) return;

    const target = focusables[focusedIndex];
    if (!target) return;
    target.dataset.songDetailsFocused = "true";
    target.focus({ preventScroll: true });
    target.scrollIntoView({ block: "nearest" });
  }, [detailsPanelOpen, focus.active, focusedIndex, focusableCount]);

  const run = (message: string, action: () => void | Promise<void>) => async () => {
    await action();
    toast.info(message);
  };

  return (
    <aside
      ref={detailsRef}
      className="flex min-h-0 min-w-0 flex-1 flex-col border-l bg-background [&_[data-song-details-focused=true]]:z-10 [&_[data-song-details-focused=true]]:ring-2 [&_[data-song-details-focused=true]]:ring-primary xl:w-96 xl:flex-none"
      aria-label="Song details"
    >
      <header className="relative border-b px-4 pb-4 pt-3">
        <Button
          variant="ghost"
          size="icon-sm"
          className="absolute right-2 top-2"
          onClick={closeDetails}
          aria-label="Close song details"
        >
          <XIcon />
        </Button>

        <div className="flex items-center gap-3 pr-8">
          <div className="relative size-16 shrink-0 overflow-hidden rounded-lg bg-muted">
            {song.album_art_path ? (
              <img
                src={convertFileSrc(song.album_art_path)}
                alt=""
                className="size-full object-cover"
              />
            ) : (
              <MusicIcon className="absolute inset-0 m-auto size-6 text-muted-foreground" />
            )}
          </div>

          <div className="min-w-0 flex-1">
            <h2 className="line-clamp-2 text-sm font-semibold leading-snug text-balance">
              {song.title}
            </h2>
            <p className="mt-1 truncate text-xs text-muted-foreground">
              {song.artist || "Unknown band"}
            </p>
            <p className="mt-0.5 truncate text-xs text-muted-foreground">
              {song.album || "Unknown album"}
            </p>
          </div>
        </div>

        <div className="mt-3 flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
          <StatusBadge song={song} queueStatus={queueStatus} />
          {song.language ? (
            <>
              <span aria-hidden="true">·</span>
              <LanguageBadge language={song.language} />
            </>
          ) : null}
          <span aria-hidden="true">·</span>
          <span className="tabular-nums">{formatSeconds(song.duration_secs)}</span>
        </div>
      </header>

      <div className="no-scrollbar min-h-0 flex-1 overflow-y-auto">
        <section className={adjustmentSectionClass} aria-labelledby="song-adjustments-heading">
          <h3 id="song-adjustments-heading" className="mb-2 text-xs font-semibold">
            Key & tempo
          </h3>
          <Shifts
            song={song}
            status={shifting}
            onStart={(type) => setShifting((current) => ({ ...current, [type]: true }))}
            onSuccess={(message, type) => {
              toast.success(message);
              queryClient.invalidateQueries({ queryKey: SONGS });
              setShifting((current) => ({ ...current, [type]: false }));
            }}
            onError={(message, type) => {
              toast.error(message);
              setShifting((current) => ({ ...current, [type]: false }));
            }}
          />
          {!supportsShifts ? (
            <p className="max-w-72 text-xs leading-relaxed text-muted-foreground">
              Key and tempo controls become available after compatible analysis.
            </p>
          ) : null}
        </section>

        <Separator />

        <section className="px-2 py-4" aria-labelledby="song-actions-heading">
          <h3 id="song-actions-heading" className="mb-2 px-2 text-xs font-semibold">
            Actions
          </h3>
          <div className="flex flex-col gap-1">
            {!status.isReady ? (
              <ActionItem
                icon={AudioLinesIcon}
                title={analysisBusy ? "Analysis in progress" : "Analyze song"}
                description="Prepare lyrics, timing, key, tempo, and stems."
                disabled={Boolean(analysisBusy)}
                onClick={() => enqueueOne(song.file_hash)}
              />
            ) : null}

            {supportsAnalysisActions ? (
              <>
                <ActionItem
                  icon={AlignLeftIcon}
                  title="Realign"
                  description="Rebuild timing from the current lyrics."
                  onClick={run(`Realigning "${song.title}"`, () => realign(song.file_hash))}
                />
                <ActionItem
                  icon={RefreshCwIcon}
                  title="Refetch lyrics & align"
                  description="Fetch fresh lyrics, then rebuild timing."
                  onClick={run(`Refetching lyrics & aligning "${song.title}"`, () =>
                    reanalyzeTranscript(song.file_hash),
                  )}
                />
                <ActionItem
                  icon={MicIcon}
                  title="Force transcribe"
                  description="Ignore online lyrics and transcribe the vocals."
                  onClick={run(`Force transcribing "${song.title}"`, () =>
                    reanalyzeForceTranscribe(song.file_hash),
                  )}
                />
                <ActionItem
                  icon={AudioLinesIcon}
                  title="Full reanalysis"
                  description="Recreate stems, lyrics, timing, key, and tempo."
                  onClick={run(`Full reanalysis (w/ stems) for "${song.title}"`, () =>
                    reanalyzeFull(song.file_hash),
                  )}
                />

                <Separator className="my-1" />

                <ActionItem
                  icon={PencilLineIcon}
                  title="Edit lyrics"
                  description="Correct the words and rebuild their timing."
                  onClick={() => setMode({ mode: "edit-lyrics", song })}
                />
                <ActionItem
                  icon={LanguagesIcon}
                  title="Change language"
                  description="Set the language and choose how to reprocess."
                  onClick={() => setMode({ mode: "language", song })}
                />

                <Separator className="my-1" />

                <ActionItem
                  icon={Trash2Icon}
                  title="Delete cache"
                  description="Remove every generated file for this song."
                  destructive
                  onClick={run(`Cache deleted for "${song.title}"`, () =>
                    deleteSongCache(song.file_hash),
                  )}
                />
              </>
            ) : null}
          </div>
        </section>
      </div>

      <footer className="border-t p-3 pb-[max(0.75rem,env(safe-area-inset-bottom))]">
        <Button
          size="lg"
          className="h-8 w-full"
          disabled={!status.isReady || shifting.key || shifting.tempo}
          onClick={() => navigate("/playback", { state: { song } })}
        >
          <PlayIcon /> Play
        </Button>
      </footer>
    </aside>
  );
};
