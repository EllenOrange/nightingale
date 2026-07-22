import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import { useMenuFocus } from "@/contexts/menu-focus-context";
import { useLibraryFilter } from "@/hooks/use-library-filter";
import { usePersistentScroll } from "@/hooks/use-persistent-scroll";
import { useSearch } from "@/hooks/use-search";
import { useConfigMutation } from "@/mutations/use-config-mutation";
import { useConfig } from "@/queries/use-config";
import { useAnalysisQueue, useSongs } from "@/queries/use-songs";
import { useEffect, useMemo, useRef, useState } from "react";
import { Filters, type SongListView } from "./filters";
import { Progress } from "./progress";
import { SongDetailsSidebar } from "./song-details-sidebar";
import type { SongItemProps } from "./types";
import { SongGrid } from "./views/song-grid";
import { SongTable } from "./views/song-table";

export const SongList = () => {
  const { data: queue } = useAnalysisQueue();
  const { data: config } = useConfig();
  const { mutate: saveConfig, isPending: isSavingView } = useConfigMutation();
  const { focus, actionsRef, setFocus } = useMenuFocus();
  const { setScrollContainer, resetScroll } = usePersistentScroll("songList");
  const { search } = useSearch();
  const { artist, album, query } = useLibraryFilter();
  const { data, fetchNextPage, hasNextPage, isFetchingNextPage } = useSongs();
  const [selectedSongHash, setSelectedSongHash] = useState<string | null>(null);

  const view: SongListView = config?.song_list_view === "grid" ? "grid" : "table";
  const songs = useMemo(() => data?.pages.flatMap((page) => page.processed) ?? [], [data]);
  const selectedSong = songs.find((song) => song.file_hash === selectedSongHash) ?? null;
  const isFirstFilterRun = useRef(true);
  const songsRef = useRef(songs);
  const sentinelRef = useRef<HTMLDivElement>(null);
  songsRef.current = songs;

  useEffect(() => {
    if (isFirstFilterRun.current) {
      isFirstFilterRun.current = false;
      return;
    }
    setSelectedSongHash(null);
    resetScroll();
    setFocus((previous) => ({ ...previous, songIndex: 0 }));
  }, [search, artist, album, query, resetScroll, setFocus]);

  useEffect(() => {
    actionsRef.current.songCount = songs.length;
  }, [songs.length, actionsRef]);

  useEffect(() => {
    actionsRef.current.onConfirmSong = (index: number) => {
      const song = songsRef.current[index];
      if (!song) return;

      setSelectedSongHash(song.file_hash);
    };
    return () => {
      actionsRef.current.onConfirmSong = null;
    };
  }, [actionsRef]);

  useEffect(() => {
    const element = sentinelRef.current;
    if (!element) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting && hasNextPage && !isFetchingNextPage) fetchNextPage();
      },
      { rootMargin: "200px" },
    );

    observer.observe(element);
    return () => observer.disconnect();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage, view]);

  const isSongListActive = focus.active && focus.panel === "songList";
  const selectSong = (fileHash: string) => setSelectedSongHash(fileHash);

  const getItemProps = (song: (typeof songs)[number], index: number): SongItemProps => ({
    song,
    queueStatus: queue?.entries[song.file_hash],
    index,
    isSelected: selectedSongHash === song.file_hash,
    isFocused: isSongListActive && !focus.analyzeAllFocused && focus.songIndex === index,
    onSelect: () => selectSong(song.file_hash),
  });

  return (
    <div className="flex min-h-0 w-full flex-1 overflow-hidden">
      <main
        className={cn(
          "min-w-0 flex-1 flex-col gap-3 p-3 sm:p-4",
          selectedSong ? "hidden xl:flex" : "flex",
        )}
      >
        <Filters
          view={view}
          isSavingView={isSavingView}
          onViewChange={(nextView) => saveConfig({ song_list_view: nextView })}
        />
        <Separator />
        <Progress />
        <div
          ref={setScrollContainer}
          data-song-layout={view}
          className="song-table-shell min-h-0 max-w-full flex-1 overflow-x-hidden overflow-y-auto pb-[max(0.75rem,env(safe-area-inset-bottom))]"
        >
          {view === "table" ? (
            <SongTable songs={songs} getItemProps={getItemProps} />
          ) : (
            <SongGrid songs={songs} getItemProps={getItemProps} />
          )}
          <div ref={sentinelRef} className="h-1" aria-hidden="true" />
        </div>
      </main>

      {selectedSong ? (
        <SongDetailsSidebar
          key={selectedSong.file_hash}
          song={selectedSong}
          queueStatus={queue?.entries[selectedSong.file_hash]}
          onClose={() => setSelectedSongHash(null)}
        />
      ) : null}
    </div>
  );
};
