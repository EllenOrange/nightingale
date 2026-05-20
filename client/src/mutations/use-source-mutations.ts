import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

import {
  clearLibrarySource,
  jellyfinLogin,
  selectFolderPath,
  setLibrarySource,
  triggerScan,
} from "@/bridge/source";
import { ANALYSIS_QUEUE, CONFIG, JELLYFIN_HEALTH, MENU, SONGS, SONGS_META } from "@/queries/keys";
import type { AppConfig } from "@/types/AppConfig";
import type { JellyfinLoginResult } from "@/types/JellyfinLoginResult";

const useInvalidateLibrary = () => {
  const queryClient = useQueryClient();

  return () => {
    queryClient.invalidateQueries({ queryKey: CONFIG });
    queryClient.invalidateQueries({ queryKey: SONGS });
    queryClient.invalidateQueries({ queryKey: SONGS_META });
    queryClient.invalidateQueries({ queryKey: MENU });
    queryClient.invalidateQueries({ queryKey: ANALYSIS_QUEUE });
    queryClient.invalidateQueries({ queryKey: JELLYFIN_HEALTH });
  };
};

/** Pick a folder, persist it as the active source, and kick off a scan. */
export const useSelectFolderSource = () => {
  const queryClient = useQueryClient();
  const invalidateLibrary = useInvalidateLibrary();

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

  return useMutation({
    mutationFn: clearLibrarySource,
    onSuccess: (config) => {
      queryClient.setQueryData(CONFIG, config);
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
    onSuccess: ({ config }) => {
      queryClient.setQueryData(CONFIG, config);
      invalidateLibrary();
    },
  });
};
