import { CONFIG } from "@/queries/keys";
import { useConfig } from "@/queries/use-config";
import { loadConfig, saveConfig } from "@/bridge/config";
import { AppConfig } from "@/types/AppConfig";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";

export const useConfigMutation = () => {
  const queryClient = useQueryClient();
  const { data: config } = useConfig();

  return useMutation({
    mutationFn: async (partialConfig: Partial<AppConfig>) => {
      const current = config ?? queryClient.getQueryData<AppConfig>(CONFIG) ?? (await loadConfig());
      return saveConfig({ ...current, ...partialConfig });
    },
    onSuccess: (savedConfig) => {
      queryClient.setQueryData(CONFIG, savedConfig);
      queryClient.invalidateQueries({ queryKey: CONFIG });
    },
    onError: (error: Error) => {
      toast.error(`Error updating the local config: ${error.message}`);
    },
  });
};
