import { useQuery } from "@tanstack/react-query";
import { PROFILES } from "./keys";
import { loadProfiles } from "@/bridge/profile";
import { ProfileStore } from "@/types/ProfileStore";

export const useProfiles = () =>
  useQuery<ProfileStore>({
    queryKey: PROFILES,
    queryFn: loadProfiles,
  });
