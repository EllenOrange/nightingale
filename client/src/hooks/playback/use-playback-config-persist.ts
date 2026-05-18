/**
 * Keeps a ref to the latest app config so partial updates can be persisted
 * without listing `config` in callback dependency arrays.
 */

import { saveConfig } from "@/bridge/config";
import { CONFIG } from "@/queries/keys";
import { useQueryClient } from "@tanstack/react-query";
import type { AppConfig } from "@/types/AppConfig";
import { useCallback, useRef } from "react";

export function usePlaybackConfigPersist(config: AppConfig | null) {
  const queryClient = useQueryClient();
  const configRef = useRef(config);
  configRef.current = config;

  const persistConfig = useCallback(
    (patch: Partial<AppConfig>) => {
      const current = configRef.current;
      if (!current) {
        return;
      }

      void saveConfig({ ...current, ...patch }).then((savedConfig) => {
        queryClient.setQueryData(CONFIG, savedConfig);
      });
    },
    [queryClient],
  );

  return persistConfig;
}
