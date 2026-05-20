import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useSetAtom } from "jotai";
import { toast } from "sonner";

import {
  clearLibrarySource,
  jellyfinLogin,
  navidromeLogin,
  selectFolderPath,
  setLibrarySource,
  triggerScan,
} from "@/bridge/source";
import { libraryFilterAtom } from "@/hooks/use-library-filter";
import { searchAtom } from "@/hooks/use-search";
import { EMPTY_LIBRARY_FILTER } from "@/lib/library-menu-filter";
import {
  ANALYSIS_QUEUE,
  CONFIG,
  JELLYFIN_HEALTH,
  MENU,
  NAVIDROME_HEALTH,
  SONGS,
  SONGS_META,
} from "@/queries/keys";
import type { AppConfig } from "@/types/AppConfig";
import type { JellyfinHealth } from "@/types/JellyfinHealth";
import type { JellyfinLoginResult } from "@/types/JellyfinLoginResult";
import type { NavidromeHealth } from "@/types/NavidromeHealth";
import type { NavidromeLoginResult } from "@/types/NavidromeLoginResult";

const useInvalidateLibrary = () => {
  const queryClient = useQueryClient();

  return () => {
    queryClient.invalidateQueries({ queryKey: CONFIG });
    queryClient.invalidateQueries({ queryKey: SONGS });
    queryClient.invalidateQueries({ queryKey: SONGS_META });
    queryClient.invalidateQueries({ queryKey: MENU });
    queryClient.invalidateQueries({ queryKey: ANALYSIS_QUEUE });
    queryClient.invalidateQueries({ queryKey: JELLYFIN_HEALTH });
    queryClient.invalidateQueries({ queryKey: NAVIDROME_HEALTH });
  };
};

/** Drop any library filter / search left over from the previous source so the
 * new library renders as "all songs" instead of accidentally filtering by an
 * artist that no longer exists. */
const useResetLibraryNavigation = () => {
  const setLibraryFilter = useSetAtom(libraryFilterAtom);
  const setSearch = useSetAtom(searchAtom);

  return () => {
    setLibraryFilter(EMPTY_LIBRARY_FILTER);
    setSearch("");
  };
};

/** Pick a folder, persist it as the active source, and kick off a scan. */
export const useSelectFolderSource = () => {
  const queryClient = useQueryClient();
  const invalidateLibrary = useInvalidateLibrary();
  const resetNavigation = useResetLibraryNavigation();

  return useMutation({
    mutationFn: async (): Promise<AppConfig | null> => {
      const path = await selectFolderPath();
      if (!path) {
        return null;
      }
      return setLibrarySource({ kind: "folder", path });
    },
    onSuccess: (config) => {
      if (!config) {
        return;
      }
      queryClient.setQueryData(CONFIG, config);
      resetNavigation();
      invalidateLibrary();
    },
    onError: (error: Error) => {
      toast.error(`Failed to select folder: ${error.message}`);
    },
  });
};

/** Re-run the scan against the currently configured source. */
export const useRescan = () => {
  const invalidateLibrary = useInvalidateLibrary();

  return useMutation({
    mutationFn: triggerScan,
    onSuccess: () => invalidateLibrary(),
    onError: (error: Error) => {
      toast.error(`Rescan failed: ${error.message}`);
    },
  });
};

/** Disconnect from whatever source is currently configured. */
export const useDisconnectSource = () => {
  const queryClient = useQueryClient();
  const invalidateLibrary = useInvalidateLibrary();
  const resetNavigation = useResetLibraryNavigation();

  return useMutation({
    mutationFn: clearLibrarySource,
    onSuccess: (config) => {
      queryClient.setQueryData(CONFIG, config);
      resetNavigation();
      invalidateLibrary();
    },
    onError: (error: Error) => {
      toast.error(`Could not clear source: ${error.message}`);
    },
  });
};

/**
 * Authenticate against a Jellyfin server. Does NOT persist the source — the
 * caller can chain this into `useConnectJellyfin` for that, or use it for a
 * standalone "Test connection" flow.
 */
export const useJellyfinLogin = () =>
  useMutation<JellyfinLoginResult, Error, { baseUrl: string; username: string; password: string }>({
    mutationFn: jellyfinLogin,
  });

/**
 * Composite: authenticate, persist the credentials as the active library
 * source, and trigger a scan (which is already done backend-side by
 * `set_library_source`).
 */
export const useConnectJellyfin = () => {
  const queryClient = useQueryClient();
  const invalidateLibrary = useInvalidateLibrary();
  const resetNavigation = useResetLibraryNavigation();

  return useMutation<
    { config: AppConfig; login: JellyfinLoginResult },
    Error,
    { baseUrl: string; username: string; password: string }
  >({
    mutationFn: async (params) => {
      const login = await jellyfinLogin(params);
      const config = await setLibrarySource({
        kind: "jellyfin",
        base_url: login.server_url,
        user_id: login.user_id,
        username: login.username,
        access_token: login.access_token,
        device_id: login.device_id,
      });
      return { config, login };
    },
    onSuccess: ({ config, login }) => {
      queryClient.setQueryData(CONFIG, config);

      // Seed the health cache from the successful login so the sidebar pill
      // flips to green immediately instead of waiting for the next poll.
      queryClient.setQueryData<JellyfinHealth>(JELLYFIN_HEALTH, {
        reachable: true,
        server_name: login.server_name ?? undefined,
        server_id: undefined,
        version: undefined,
        error: undefined,
      });

      resetNavigation();
      invalidateLibrary();
    },
  });
};

/**
 * Authenticate against a Navidrome / Subsonic server. Mirrors
 * `useJellyfinLogin`: no persistence, just a smoke test the dialog can use
 * to display a friendly error before it asks to commit the source.
 */
export const useNavidromeLogin = () =>
  useMutation<NavidromeLoginResult, Error, { baseUrl: string; username: string; password: string }>(
    {
      mutationFn: navidromeLogin,
    },
  );

/**
 * Composite: authenticate, persist the credentials as the active library
 * source, and trigger a scan (which is done backend-side as part of
 * `set_library_source`).
 */
export const useConnectNavidrome = () => {
  const queryClient = useQueryClient();
  const invalidateLibrary = useInvalidateLibrary();
  const resetNavigation = useResetLibraryNavigation();

  return useMutation<
    { config: AppConfig; login: NavidromeLoginResult },
    Error,
    { baseUrl: string; username: string; password: string }
  >({
    mutationFn: async (params) => {
      const login = await navidromeLogin(params);
      const config = await setLibrarySource({
        kind: "navidrome",
        base_url: login.server_url,
        username: login.username,
        password: login.password,
      });
      return { config, login };
    },
    onSuccess: ({ config, login }) => {
      queryClient.setQueryData(CONFIG, config);

      queryClient.setQueryData<NavidromeHealth>(NAVIDROME_HEALTH, {
        reachable: true,
        server_name: login.server_name ?? undefined,
        version: login.server_version ?? undefined,
        error: undefined,
      });

      resetNavigation();
      invalidateLibrary();
    },
  });
};
