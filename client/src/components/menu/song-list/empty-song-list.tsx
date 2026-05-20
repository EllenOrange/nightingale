import { FolderIcon, MusicIcon } from "lucide-react";

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
    <Empty>
      <EmptyHeader>
        <EmptyMedia variant="icon">
          <MusicIcon />
        </EmptyMedia>
        <EmptyTitle>No library yet</EmptyTitle>
        <EmptyDescription>
          Pick a folder on this machine or connect a Jellyfin server to start enjoying your karaoke!
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent className="flex-row justify-center gap-2">
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
