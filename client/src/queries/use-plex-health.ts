import { useQuery } from "@tanstack/react-query";

import { plexPing } from "@/bridge/source";
import { getSource } from "@/lib/library-source";
import { useConfig } from "@/queries/use-config";

import { PLEX_HEALTH } from "./keys";

const ONLINE_REFRESH_MS = 30_000;
const OFFLINE_REFRESH_MS = 10_000;

export const usePlexHealth = () => {
  const { data: config } = useConfig();
  const enabled = getSource(config, "plex") !== null;

  return useQuery({
    queryKey: PLEX_HEALTH,
    queryFn: plexPing,
    enabled,
    refetchInterval: (data) => (data?.reachable === false ? OFFLINE_REFRESH_MS : ONLINE_REFRESH_MS),
    refetchOnWindowFocus: true,
    staleTime: 5_000,
  });
};
