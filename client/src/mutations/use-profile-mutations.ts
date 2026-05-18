import { PROFILES } from "@/queries/keys";
import { createProfile, deleteProfile, switchProfile } from "@/bridge/profile";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

export const useProfileMutations = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ name, type }: { name: string; type: "create" | "switch" | "delete" }) => {
      switch (type) {
        case "create":
          return createProfile(name);
        case "switch":
          return switchProfile(name);
        case "delete":
          return deleteProfile(name);
      }
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: PROFILES });
    },
    onError: (error: Error) => {
      toast.error(`Error updating profiles: ${error.message}`);
    },
  });
};
