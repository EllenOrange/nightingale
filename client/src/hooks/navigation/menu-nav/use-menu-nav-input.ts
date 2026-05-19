import { useCallback } from "react";
import type { NavAction } from "@/contexts/nav-input-context";
import { useNavInput } from "../use-nav-input";
import { blurActiveTextInput } from "./dom";
import type { MenuNavHookOptions } from "./types";

const CONFIRM_COOLDOWN_MS = 140;

interface UseMenuNavInputOptions extends MenuNavHookOptions {
  scrollToSong: (index: number) => void;
}

function hasInput(action: NavAction): boolean {
  return action.up || action.down || action.left || action.right || action.confirm || action.back;
}

export function useMenuNavInput({ menuFocus, refs, lock, scrollToSong }: UseMenuNavInputOptions) {
  const { activate, actionsRef } = menuFocus;

  useNavInput(
    useCallback(
      (action) => {
        if (refs.overlayOpenRef.current || !hasInput(action)) {
          return;
        }

        if (action.back) {
          const handled = actionsRef.current.onSidebarBack?.();
          if (!handled) {
            refs.onBackRef.current();
          }
          return;
        }

        if (actionsRef.current.isSidebarBusy?.()) {
          return;
        }

        blurActiveTextInput();
        activate();
        lock.lockTemporarily();

        if (action.confirm) {
          handleConfirmAction(menuFocus, refs);
          return;
        }

        if (action.left || action.right) {
          if (handleHorizontalAction(action, menuFocus)) {
            return;
          }
        }

        if (action.up || action.down) {
          handleVerticalAction(action, menuFocus, scrollToSong);
        }
      },
      [actionsRef, activate, lock, menuFocus, refs, scrollToSong],
    ),
  );
}

function handleConfirmAction(
  { actionsRef, setFocus }: Pick<MenuNavHookOptions["menuFocus"], "actionsRef" | "setFocus">,
  refs: MenuNavHookOptions["refs"],
) {
  const now = performance.now();

  setFocus((prev) => {
    if (!prev.active) return prev;
    if (now - refs.lastConfirmAtRef.current < CONFIRM_COOLDOWN_MS) return prev;
    refs.lastConfirmAtRef.current = now;

    if (prev.panel === "songList") {
      if (prev.analyzeAllFocused) {
        actionsRef.current.onConfirmAnalyzeAll?.();
      } else {
        actionsRef.current.onConfirmSong?.(prev.songIndex);
      }
    } else if (prev.panel === "sidebar") {
      actionsRef.current.onConfirmSidebar?.(prev.sidebarIndex);
    }

    return prev;
  });
}

function handleHorizontalAction(
  action: NavAction,
  { actionsRef, setFocus }: Pick<MenuNavHookOptions["menuFocus"], "actionsRef" | "setFocus">,
): boolean {
  let handled = false;

  setFocus((prev) => {
    if (prev.panel === "sidebar") {
      const subCount = actionsRef.current.sidebarSubCountByIndex.get(prev.sidebarIndex);
      if (subCount && subCount > 1) {
        const delta = action.left ? -1 : 1;
        const nextSub = Math.max(0, Math.min(subCount - 1, prev.sidebarSubIndex + delta));
        if (nextSub === prev.sidebarSubIndex) {
          handled = action.left;
          return prev;
        }

        handled = true;
        return { ...prev, sidebarSubIndex: nextSub, active: true, source: "nav" };
      }
    }

    return prev;
  });

  if (handled) {
    return true;
  }

  if (action.left) {
    setFocus((prev) => ({
      ...prev,
      panel: "sidebar",
      analyzeAllFocused: false,
      active: true,
      source: "nav",
    }));
    return true;
  }

  setFocus((prev) => ({ ...prev, panel: "songList", active: true, source: "nav" }));
  return true;
}

function handleVerticalAction(
  action: NavAction,
  { actionsRef, setFocus }: Pick<MenuNavHookOptions["menuFocus"], "actionsRef" | "setFocus">,
  scrollToSong: (index: number) => void,
) {
  setFocus((prev) => {
    const next = { ...prev, active: true, source: "nav" as const };

    if (prev.panel === "songList") {
      const songCount = actionsRef.current.songCount;

      if (prev.analyzeAllFocused) {
        if (action.down) {
          next.analyzeAllFocused = false;
          next.songIndex = 0;
          scrollToSong(0);
        }
      } else if (action.up) {
        if (prev.songIndex <= 0) {
          next.analyzeAllFocused = true;
        } else {
          next.songIndex = prev.songIndex - 1;
          scrollToSong(prev.songIndex - 1);
        }
      } else if (action.down) {
        if (prev.songIndex < songCount - 1) {
          next.songIndex = prev.songIndex + 1;
          scrollToSong(prev.songIndex + 1);
        }
      }
    } else if (prev.panel === "sidebar") {
      const sidebarCount = actionsRef.current.sidebarCount;
      if (sidebarCount <= 0) {
        next.sidebarIndex = 0;
        next.sidebarSubIndex = 0;
        return next;
      }

      if (action.up) {
        next.sidebarIndex = Math.max(0, prev.sidebarIndex - 1);
        next.sidebarSubIndex = 0;
      } else if (action.down) {
        next.sidebarIndex = Math.min(sidebarCount - 1, prev.sidebarIndex + 1);
        next.sidebarSubIndex = 0;
      }
    }

    return next;
  });
}
