import { EditLyricsDialog } from "@/components/menu/dialogs/edit-lyrics";
import { ExitDialog } from "@/components/menu/dialogs/exit";
import { InfoDialog } from "@/components/menu/dialogs/info";
import { SelectLanguageDialog } from "@/components/menu/dialogs/language";
import { CreateProfileDialog } from "@/components/menu/dialogs/profile/create";
import { SelectProfileDialog } from "@/components/menu/dialogs/profile/select";
import { SettingsDialog } from "@/components/menu/dialogs/settings";
import { UpdateDialog } from "@/components/menu/dialogs/update";
import { Sidebar } from "@/components/menu/sidebar/sidebar";
import { EmptySongList } from "@/components/menu/song-list/empty-song-list";
import { SongList } from "@/components/menu/song-list/song-list";
import { SidebarInset } from "@/components/ui/sidebar";
import { MenuFocusProvider } from "@/contexts/menu-focus-context";
import { useMenuNav } from "@/hooks/navigation/use-menu-nav";
import { useDialog } from "@/hooks/use-dialog";
import { useShouldRunSetup } from "@/hooks/use-should-run-setup";
import { useSongsMeta } from "@/queries/use-songs";
import { ReactElement, useCallback } from "react";

const MenuInner = () => {
  const { data: meta, isLoading: isLoadingMeta } = useSongsMeta();
  const { mode, setMode } = useDialog();
  const { shouldRunSetup } = useShouldRunSetup();

  const overlayOpen = mode !== null || shouldRunSetup;

  const onBack = useCallback(() => {
    setMode((prev) => {
      if (prev === null) {
        return "exit";
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
      <ExitDialog />
      <SettingsDialog />
      <CreateProfileDialog />
      <SelectProfileDialog />
      <InfoDialog />
      <UpdateDialog />
      <SelectLanguageDialog />
      <EditLyricsDialog />
      <SidebarInset>{content}</SidebarInset>
    </Sidebar>
  );
};

export const Menu = () => (
  <MenuFocusProvider>
    <MenuInner />
  </MenuFocusProvider>
);
