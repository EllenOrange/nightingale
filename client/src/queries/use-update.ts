import { useQuery } from "@tanstack/react-query";
import { type Update } from "@tauri-apps/plugin-updater";
import { UPDATER } from "./keys";
import { checkForUpdate } from "@/tauri-bridge/updater";

export type UpdateStatus = "checking" | "available" | "up-to-date" | "error";

export interface UpdateState {
  status: UpdateStatus;
  update: Update | null;
  error: Error | null;
}

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

export const useUpdate = () => {
  const query = useQuery({
    queryKey: UPDATER,
    queryFn: checkForUpdate,
    staleTime: Infinity,
    cacheTime: Infinity,
    retry: false,
    refetchOnWindowFocus: false,
    refetchOnMount: false,
  });

  let status: UpdateStatus;
  if (query.isLoading || query.isFetching) {
    status = "checking";
  } else if (query.isError) {
    status = "error";
  } else if (query.data) {
    status = "available";
  } else {
    status = "up-to-date";
  }

  const error = query.error instanceof Error ? query.error : null;

  const state: UpdateState = {
    status,
    update: query.data ?? null,
    error,
  };

  return {
    ...state,
    isOffline: error ? isOfflineError(error) : false,
    refetch: query.refetch,
  };
};
