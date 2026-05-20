import { LoaderCircleIcon } from "lucide-react";

import { Progress as ShadCnProgress } from "@/components/ui/progress";
import { useConfig } from "@/queries/use-config";
import { useSongsMeta } from "@/queries/use-songs";
import type { LibrarySource } from "@/types/LibrarySource";

function sourceLabel(source: LibrarySource | null | undefined): string {
  switch (source?.kind) {
    case "jellyfin":
      return "Jellyfin";
    case "navidrome":
      return "Navidrome";
    case "folder":
      return "library folder";
    default:
      return "library";
  }
}

export const Progress = () => {
  const { data: meta } = useSongsMeta();
  const { data: config } = useConfig();

  if (!meta) {
    return null;
  }

  const { count, processed_count, folder } = meta;

  // Pre-first-page window: scan kicked off (folder label set) but the source
  // hasn't told us how big the catalogue is yet, so the determinate bar
  // would be `max=0`. Show an indeterminate "Connecting…" hint so the user
  // knows something is happening during the initial round-trip (Jellyfin
  // `/Items`, Navidrome `getAlbumList2`).
  if (folder && count === 0 && processed_count === 0) {
    return (
      <div className="flex items-center gap-1 text-xs text-muted-foreground">
        <LoaderCircleIcon className="size-3 animate-spin" />
        Connecting to {sourceLabel(config?.library_source)}...
      </div>
    );
  }

  if (count === processed_count) {
    return null;
  }

  return <ShadCnProgress max={count} value={processed_count} />;
};
