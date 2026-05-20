import { atom, useAtom } from "jotai";
import { EMPTY_LIBRARY_FILTER } from "@/lib/library-menu-filter";
import type { LibraryMenuFilters } from "@/types/LibraryMenuFilters";

export const libraryFilterAtom = atom<LibraryMenuFilters>(EMPTY_LIBRARY_FILTER);

export const useLibraryFilter = () => {
  const [filter, setLibraryFilter] = useAtom(libraryFilterAtom);

  return {
    artist: filter.artist,
    album: filter.album,
    query: filter.query,
    setLibraryFilter,
  };
};

export type { LibraryMenuFilters } from "@/types/LibraryMenuFilters";
export type { LibraryMenuSection } from "@/lib/library-menu-filter";
