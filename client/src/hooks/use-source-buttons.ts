import { useMemo, type ComponentType, type SVGProps } from "react";
import { FolderIcon, RefreshCwIcon } from "lucide-react";

import { JellyfinIcon } from "@/components/icons/jellyfin";
import type { BadgeTone } from "@/components/menu/sidebar/source-action-button";
import { useDialog } from "@/hooks/use-dialog";
import { useLibrarySourceActions } from "@/hooks/use-library-source-actions";

export interface SourceButton {
  key: string;
  icon: ComponentType<SVGProps<SVGSVGElement>>;
  label: string;
  tooltip: string;
  handler: () => void;
  disabled: boolean;
  badge?: BadgeTone;
}

/**
 * Derives the ordered list of Library-cluster buttons (Jellyfin, Folder,
 * Rescan) along with their dynamic state — most notably the Jellyfin
 * connection badge + tooltip driven by `useJellyfinHealth`. Centralising this
 * keeps `FolderActions` focused on layout + focus management.
 */
export const useSourceButtons = (): SourceButton[] => {
  const { setMode } = useDialog();
  const { selectFolder, rescan, rescanDisabled, isPending, hasSource, jellyfinSource, health } =
    useLibrarySourceActions();

  const jellyfin = useMemo<{ tooltip: string; badge?: BadgeTone }>(() => {
    if (!jellyfinSource) {
      return { tooltip: "Connect Jellyfin" };
    }
    const hostname = health?.server_name ?? jellyfinSource.base_url.replace(/^https?:\/\//, "");
    if (!health) {
      return { tooltip: `Checking ${hostname}…`, badge: "muted" };
    }
    if (health.reachable) {
      return { tooltip: `Connected to: ${hostname}`, badge: "ok" };
    }
    return {
      tooltip: health.error ? `Offline: ${hostname} — ${health.error}` : `Offline: ${hostname}`,
      badge: "warn",
    };
  }, [jellyfinSource, health]);

  return useMemo<SourceButton[]>(() => {
    const buttons: SourceButton[] = [
      {
        key: "jellyfin",
        icon: JellyfinIcon,
        label: "Connect Jellyfin",
        tooltip: jellyfin.tooltip,
        handler: () => setMode("jellyfin-connect"),
        disabled: isPending,
        badge: jellyfin.badge,
      },
      {
        key: "folder",
        icon: FolderIcon,
        label: "Select folder",
        tooltip: "Select folder",
        handler: selectFolder,
        disabled: isPending,
      },
    ];

    if (hasSource) {
      buttons.push({
        key: "rescan",
        icon: RefreshCwIcon,
        label: "Rescan library",
        tooltip: "Rescan library",
        handler: rescan,
        disabled: rescanDisabled,
      });
    }

    return buttons;
  }, [hasSource, isPending, jellyfin, rescan, rescanDisabled, selectFolder, setMode]);
};
