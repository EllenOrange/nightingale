import { useConfig } from "@/queries/use-config";
import { useJellyfinHealth } from "@/queries/use-jellyfin-health";
import { useNavidromeHealth } from "@/queries/use-navidrome-health";
import {
  useDisconnectSource,
  useRescan,
  useSelectFolderSource,
} from "@/mutations/use-source-mutations";
import { getSource } from "@/lib/library-source";
import { getServerFlags } from "@/bridge/server-flags";

/**
 * Coordinated source actions used by the sidebar + empty-state. Wraps the
 * `bridge/source` mutations and exposes precomputed booleans for "what's the
 * active source" / "is rescan enabled".
 */
export const useLibrarySourceActions = () => {
  const { data: config } = useConfig();
  const { data: jellyfinHealth } = useJellyfinHealth();
  const { data: navidromeHealth } = useNavidromeHealth();

  const folderMutation = useSelectFolderSource();
  const rescanMutation = useRescan();
  const disconnectMutation = useDisconnectSource();

  // When the operator pins the library folder (NIGHTINGALE_LIBRARY_PATH), the
  // source is configured server-side and picking one in-app is disabled.
  const { libraryPinned } = getServerFlags();

  const hasSource = !!config?.library_source;
  const jellyfinSource = getSource(config, "jellyfin");
  const navidromeSource = getSource(config, "navidrome");
  const isFolderSource = config?.library_source?.kind === "folder";
  const isJellyfinSource = jellyfinSource !== null;
  const isNavidromeSource = navidromeSource !== null;

  const isPending =
    folderMutation.isPending || rescanMutation.isPending || disconnectMutation.isPending;

  // Rescan is safe to fire whenever there's an active source — start_scan()
  // bumps the cancellation generation, so kicking off a new one while one is
  // already running just supersedes the old. The only hard-blocks are: no
  // source at all, or a remote source whose server we know is offline.
  const rescanDisabled =
    !hasSource ||
    rescanMutation.isPending ||
    (isJellyfinSource && jellyfinHealth?.reachable === false) ||
    (isNavidromeSource && navidromeHealth?.reachable === false);

  return {
    config,
    health: jellyfinHealth,
    jellyfinHealth,
    navidromeHealth,
    hasSource,
    libraryPinned,
    jellyfinSource,
    navidromeSource,
    isFolderSource,
    isJellyfinSource,
    isNavidromeSource,
    selectFolder: () => folderMutation.mutate(),
    rescan: () => rescanMutation.mutate(),
    disconnectSource: () => disconnectMutation.mutate(),
    isPending,
    rescanDisabled,
  };
};
