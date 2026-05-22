import { ClearCacheDialog } from "@/components/menu/dialogs/clear-cache";
import { EditLyricsDialog } from "@/components/menu/dialogs/edit-lyrics";
import { ExitDialog } from "@/components/menu/dialogs/exit";
import { InfoDialog } from "@/components/menu/dialogs/info";
import { JellyfinConnectDialog } from "@/components/menu/dialogs/remote-source/jellyfin-connect";
import { NavidromeConnectDialog } from "@/components/menu/dialogs/remote-source/navidrome-connect";
import { SelectLanguageDialog } from "@/components/menu/dialogs/language";
import { CreateProfileDialog } from "@/components/menu/dialogs/profile/create";
import { SelectProfileDialog } from "@/components/menu/dialogs/profile/select";
import { SettingsDialog } from "@/components/menu/dialogs/settings";
import { UpdateDialog } from "@/components/menu/dialogs/update";
import { Sidebar } from "@/components/menu/sidebar/sidebar";
import { EmptySongList } from "@/components/menu/song-list/empty-song-list";
import { SongList } from "@/components/menu/song-list/song-list";
import { SidebarInset } from "@/components/ui/sidebar";
import { EXIT_SUPPORTED } from "@/bridge/exit";
import { useMenuNav } from "@/hooks/navigation/use-menu-nav";
import { useDialog } from "@/hooks/use-dialog";
import { useShouldRunSetup } from "@/hooks/use-should-run-setup";
import { useSongsMeta } from "@/queries/use-songs";
import { ReactElement, useCallback } from "react";

export const Menu = () => {
  const { data: meta, isLoading: isLoadingMeta } = useSongsMeta();
  const { mode, setMode } = useDialog();
  const { shouldRunSetup } = useShouldRunSetup();

  const overlayOpen = mode !== null || shouldRunSetup;

  const onBack = useCallback(() => {
    setMode((prev) => {
      if (prev === null) {
        // Web mode has no app to exit; swallow the back input rather than
        // surfacing a dialog whose confirm action can't do anything useful.
        return EXIT_SUPPORTED ? "exit" : null;
      }

      if (prev === "exit") {
        return null;
      }

      return prev;
    });
  }, [setMode]);

  useMenuNav({ overlayOpen, onBack });

  let content: ReactElement | null = <EmptySongList />;

  if (meta?.folder) {
    content = <SongList />;
  }

  if (isLoadingMeta) {
    content = null;
  }

  return (
    <Sidebar>
      {EXIT_SUPPORTED && <ExitDialog />}
      <SettingsDialog />
      <CreateProfileDialog />
      <SelectProfileDialog />
      <InfoDialog />
      <UpdateDialog />
      <SelectLanguageDialog />
      <EditLyricsDialog />
      <ClearCacheDialog />
      <JellyfinConnectDialog />
      <NavidromeConnectDialog />
      <SidebarInset>{content}</SidebarInset>
    </Sidebar>
  );
};
