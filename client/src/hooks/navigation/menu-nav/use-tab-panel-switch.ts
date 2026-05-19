import { useEffect } from "react";
import { blurActiveTextInput } from "./dom";
import type { MenuNavHookOptions } from "./types";

export function useTabPanelSwitch({ menuFocus, refs, lock }: MenuNavHookOptions) {
  const { activate, setFocus } = menuFocus;

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Tab" || refs.overlayOpenRef.current) {
        return;
      }

      event.preventDefault();

      activate();
      lock.lockTemporarily();
      blurActiveTextInput();

      setFocus((prev) => ({
        ...prev,
        active: true,
        analyzeAllFocused: false,
        panel: prev.panel === "songList" ? "sidebar" : "songList",
        source: "nav",
      }));
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [activate, lock, refs, setFocus]);
}
