import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type RefObject,
  type ReactNode,
} from "react";

export type FocusPanel = "songList" | "sidebar";

export interface MenuFocus {
  active: boolean;
  panel: FocusPanel;
  songIndex: number;
  sidebarIndex: number;
  sidebarSubIndex: number;
  analyzeAllFocused: boolean;
}

export interface MenuFocusActions {
  onConfirmSong: ((index: number) => void) | null;
  onConfirmSidebar: ((index: number) => void) | null;
  onConfirmAnalyzeAll: (() => void) | null;
  onSidebarBack: (() => boolean) | null;
  isSidebarBusy: (() => boolean) | null;
  songCount: number;
  sidebarCount: number;
  sidebarSubCountByIndex: Map<number, number>;
}

export interface MenuFocusContextValue {
  focus: MenuFocus;
  setFocus: (updater: (prev: MenuFocus) => MenuFocus) => void;
  activate: () => void;
  deactivate: () => void;
  actionsRef: RefObject<MenuFocusActions>;
  scrollRef: RefObject<HTMLElement | null>;
}

const MenuFocusContext = createContext<MenuFocusContextValue | null>(null);

const INITIAL_FOCUS: MenuFocus = {
  active: false,
  panel: "songList",
  songIndex: 0,
  sidebarIndex: 0,
  sidebarSubIndex: 0,
  analyzeAllFocused: false,
};

const INITIAL_ACTIONS: MenuFocusActions = {
  onConfirmSong: null,
  onConfirmSidebar: null,
  onConfirmAnalyzeAll: null,
  onSidebarBack: null,
  isSidebarBusy: null,
  songCount: 0,
  sidebarCount: 0,
  sidebarSubCountByIndex: new Map(),
};

export function MenuFocusProvider({ children }: { children: ReactNode }) {
  const [focus, setFocusState] = useState<MenuFocus>(INITIAL_FOCUS);
  const actionsRef = useRef<MenuFocusActions>({ ...INITIAL_ACTIONS });
  const scrollRef = useRef<HTMLElement | null>(null);

  const setFocus = useCallback((updater: (prev: MenuFocus) => MenuFocus) => {
    setFocusState(updater);
  }, []);

  const activate = useCallback(() => {
    setFocusState((prev) => (prev.active ? prev : { ...prev, active: true }));
  }, []);

  const deactivate = useCallback(() => {
    setFocusState((prev) => (prev.active ? { ...prev, active: false } : prev));
  }, []);

  const value = useMemo(
    () => ({ focus, setFocus, activate, deactivate, actionsRef, scrollRef }),
    [focus, setFocus, activate, deactivate],
  );

  return <MenuFocusContext.Provider value={value}>{children}</MenuFocusContext.Provider>;
}

export function useMenuFocus(): MenuFocusContextValue {
  const ctx = useContext(MenuFocusContext);
  if (!ctx) {
    throw new Error("useMenuFocus must be used within MenuFocusProvider");
  }
  return ctx;
}
