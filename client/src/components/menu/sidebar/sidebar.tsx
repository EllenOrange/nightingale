import {
  Sidebar as ShadCnSidebar,
  SidebarFooter,
  SidebarProvider,
  SidebarSeparator,
} from "@/components/ui/sidebar";
import { Stats } from "./stats";
import { Header } from "./header";
import { MainNavigation } from "./main-navigation";
import { Actions } from "./actions";
import { useMenuFocus } from "@/contexts/menu-focus-context";
import { useCallback, useEffect, useMemo, useRef, useState, type PropsWithChildren } from "react";

const FOLDER_SLOT_INDEX = 0;
const MAIN_NAV_BASE_INDEX = FOLDER_SLOT_INDEX + 1;

export const Sidebar = ({ children }: PropsWithChildren<{}>) => {
  const { focus, actionsRef, setFocus } = useMenuFocus();
  const [mainNavigationCallbacks, setMainNavigationCallbacks] = useState<(() => void)[]>([]);
  const [folderCallback, setFolderCallback] = useState<((subIndex: number) => void) | null>(null);
  const [cacheCallback, setCacheCallback] = useState<((subIndex: number) => void) | null>(null);
  const [actionsCallback, setActionsCallback] = useState<(() => void) | null>(null);

  const focusRef = useRef(focus);
  focusRef.current = focus;

  const folderClusterCallback = useCallback(() => {
    folderCallback?.(focusRef.current.sidebarSubIndex);
  }, [folderCallback]);

  const cacheClusterCallback = useCallback(() => {
    cacheCallback?.(focusRef.current.sidebarSubIndex);
  }, [cacheCallback]);

  const cacheSlotIndex = MAIN_NAV_BASE_INDEX + mainNavigationCallbacks.length;
  const actionsSlotIndex = cacheSlotIndex + 1;

  const sidebarCallbacks = useMemo(
    () => [
      folderClusterCallback,
      ...mainNavigationCallbacks,
      cacheClusterCallback,
      ...(actionsCallback ? [actionsCallback] : []),
    ],
    [folderClusterCallback, mainNavigationCallbacks, cacheClusterCallback, actionsCallback],
  );

  const registerMainNavigationCallbacks = useCallback((callbacks: (() => void)[]) => {
    setMainNavigationCallbacks(callbacks);
  }, []);

  const registerFolderCallback = useCallback((callback: ((subIndex: number) => void) | null) => {
    setFolderCallback(() => callback);
  }, []);

  const registerCacheCallback = useCallback((callback: ((subIndex: number) => void) | null) => {
    setCacheCallback(() => callback);
  }, []);

  const registerActionsCallback = useCallback((callback: (() => void) | null) => {
    setActionsCallback(() => callback);
  }, []);

  useEffect(() => {
    actionsRef.current.sidebarCount = sidebarCallbacks.length;

    actionsRef.current.onConfirmSidebar = (index: number) => {
      sidebarCallbacks[index]?.();
    };

    setFocus((prev) => {
      const maxIndex = Math.max(0, sidebarCallbacks.length - 1);
      const nextSidebarIndex = Math.min(prev.sidebarIndex, maxIndex);
      if (nextSidebarIndex === prev.sidebarIndex) {
        return prev;
      }
      return { ...prev, sidebarIndex: nextSidebarIndex };
    });

    return () => {
      actionsRef.current.onConfirmSidebar = null;
      actionsRef.current.sidebarCount = 0;
    };
  }, [actionsRef, sidebarCallbacks, setFocus]);

  return (
    <SidebarProvider>
      <ShadCnSidebar>
        <Header />

        <MainNavigation
          baseIndex={MAIN_NAV_BASE_INDEX}
          registerCallbacks={registerMainNavigationCallbacks}
          folderFocusedSidebarIndex={FOLDER_SLOT_INDEX}
          registerFolderCallback={registerFolderCallback}
        />

        <SidebarFooter>
          <Stats
            cacheFocusedSidebarIndex={cacheSlotIndex}
            registerCacheCallback={registerCacheCallback}
          />
          <SidebarSeparator />
          <Actions
            focusedSidebarIndex={actionsSlotIndex}
            registerCallback={registerActionsCallback}
          />
        </SidebarFooter>
      </ShadCnSidebar>
      {children}
    </SidebarProvider>
  );
};
