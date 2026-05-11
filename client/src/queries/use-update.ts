import { useQuery } from "@tanstack/react-query";
import { type Update } from "@tauri-apps/plugin-updater";
import { UPDATER } from "./keys";
import { checkForUpdate } from "@/tauri-bridge/updater";
import { UPDATES_SUPPORTED } from "@/tauri-bridge/platform";

export type UpdateState =
  | { status: "unsupported" }
  | { status: "checking" }
  | { status: "error"; error: Error; isOffline: boolean }
  | { status: "up-to-date" }
  | { status: "available"; update: Update };

export type UpdateStatus = UpdateState["status"];

const isOfflineError = (error: Error): boolean => {
  if (typeof navigator !== "undefined" && navigator.onLine === false) {
    return true;
  }

  const msg = error.message.toLowerCase();
  return (
    msg.includes("network") ||
    msg.includes("failed to fetch") ||
    msg.includes("dns") ||
    msg.includes("getaddrinfo") ||
    msg.includes("connect") ||
    msg.includes("timed out")
  );
};

const buildState = (query: {
  isLoading: boolean;
  isFetching: boolean;
  isError: boolean;
  data: Update | null | undefined;
  error: unknown;
}): UpdateState => {
  if (!UPDATES_SUPPORTED) {
    return { status: "unsupported" };
  }

  if (query.isLoading || query.isFetching) {
    return { status: "checking" };
  }

  if (query.isError) {
    const error = query.error instanceof Error ? query.error : new Error("Unknown error");

    return { status: "error", error, isOffline: isOfflineError(error) };
  }

  if (query.data) {
    return { status: "available", update: query.data };
  }

  return { status: "up-to-date" };
};

export const useUpdate = () => {
  const query = useQuery({
    queryKey: UPDATER,
    queryFn: checkForUpdate,
    staleTime: Infinity,
    cacheTime: Infinity,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnMount: false,
    enabled: UPDATES_SUPPORTED,
  });

  return { ...buildState(query), refetch: query.refetch };
};
