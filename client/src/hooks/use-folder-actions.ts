import { MENU, SONGS, SONGS_META } from "@/queries/keys";
import { useSongsMeta } from "@/queries/use-songs";
import { selectFolder, triggerScan } from "@/bridge/folder";
import { useQueryClient } from "@tanstack/react-query";

export const useFolderActions = () => {
  const queryClient = useQueryClient();
  const { data: meta } = useSongsMeta();

  const invalidateAll = () => {
    queryClient.invalidateQueries({ queryKey: SONGS });
    queryClient.invalidateQueries({ queryKey: MENU });
    queryClient.invalidateQueries({ queryKey: SONGS_META });
  };

  return {
    selectFolder: async () => {
      await selectFolder();
      invalidateAll();
    },
    rescanFolder: async () => {
      if (!meta?.folder) {
        return;
      }

      await triggerScan(meta.folder);
      invalidateAll();
    },
    rescanFolderDisabled: !meta?.folder || meta.count !== meta.processed_count,
  };
};
