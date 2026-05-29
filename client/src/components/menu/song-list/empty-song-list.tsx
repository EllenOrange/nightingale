import { Disc3Icon, FolderIcon, MusicIcon } from "lucide-react";

import { JellyfinIcon } from "@/components/icons/jellyfin";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { useDialog } from "@/hooks/use-dialog";
import { useLibrarySourceActions } from "@/hooks/use-library-source-actions";

export const EmptySongList = () => {
  const { selectFolder, isPending } = useLibrarySourceActions();
  const { setMode } = useDialog();

  return (
    <Empty className="px-4 pt-16 md:pt-6">
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <MusicIcon />
        </EmptyMedia>
        <EmptyTitle>No library yet</EmptyTitle>
        <EmptyDescription>
          Pick a folder on this machine or connect a Jellyfin or Navidrome server to start enjoying
          your karaoke!
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent className="max-w-md flex-col justify-center gap-2 sm:flex-row sm:flex-wrap">
        <Button variant="outline" onClick={() => setMode("navidrome-connect")} disabled={isPending}>
          <Disc3Icon /> Connect Navidrome
        </Button>
        <Button variant="outline" onClick={() => setMode("jellyfin-connect")} disabled={isPending}>
          <JellyfinIcon /> Connect Jellyfin
        </Button>
        <Button variant="outline" onClick={() => selectFolder()} disabled={isPending}>
          <FolderIcon /> Select folder
        </Button>
      </EmptyContent>
    </Empty>
  );
};
