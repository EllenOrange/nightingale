import { useConfig } from "@/queries/use-config";
import { useJellyfinHealth } from "@/queries/use-jellyfin-health";
import {
  useDisconnectSource,
  useRescan,
  useSelectFolderSource,
} from "@/mutations/use-source-mutations";
import { getSource } from "@/lib/library-source";

/**
 * Coordinated source actions used by the sidebar + empty-state. Wraps the
 * `bridge/source` mutations and exposes precomputed booleans for "what's the
 * active source" / "is rescan enabled".
 */
export const useLibrarySourceActions = () => {
  const { data: config } = useConfig();
  const { data: health } = useJellyfinHealth();

  const folderMutation = useSelectFolderSource();
  const rescanMutation = useRescan();
  const disconnectMutation = useDisconnectSource();

  const hasSource = !!config?.library_source;
  const jellyfinSource = getSource(config, "jellyfin");
  const isFolderSource = config?.library_source?.kind === "folder";
  const isJellyfinSource = jellyfinSource !== null;

  const isPending =
    folderMutation.isPending || rescanMutation.isPending || disconnectMutation.isPending;

  // Rescan is safe to fire whenever there's an active source — start_scan()
  // bumps the cancellation generation, so kicking off a new one while one is
  // already running just supersedes the old. The only hard-blocks are: no
  // source at all, or a Jellyfin source whose server we know is offline.
  const rescanDisabled =
    !hasSource || rescanMutation.isPending || (isJellyfinSource && health?.reachable === false);

  return {
    config,
    health,
    hasSource,
    jellyfinSource,
    isFolderSource,
    isJellyfinSource,
    selectFolder: () => folderMutation.mutate(),
    rescan: () => rescanMutation.mutate(),
    disconnectSource: () => disconnectMutation.mutate(),
    isPending,
    rescanDisabled,
  };
};
