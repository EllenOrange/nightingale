import { useMenuFocus } from "@/contexts/menu-focus-context";
import { useNavInput } from "./use-nav-input";
import { useCallback, useEffect, useRef } from "react";

const NAV_LOCK_MS = 120;
const CONFIRM_COOLDOWN_MS = 140;
const BORDER_PADDING = 3;

interface UseMenuNavOptions {
  overlayOpen: boolean;
  onBack: () => void;
}

export function useMenuNav({ overlayOpen, onBack }: UseMenuNavOptions) {
  const { setFocus, activate, deactivate, actionsRef, scrollRef } = useMenuFocus();

  const onBackRef = useRef(onBack);
  onBackRef.current = onBack;
  const navLockedRef = useRef(false);
  const lastConfirmAtRef = useRef(0);
  const overlayOpenRef = useRef(overlayOpen);
  overlayOpenRef.current = overlayOpen;
  const navLockTimer = useRef<ReturnType<typeof setTimeout>>(undefined);

  // Mouse coexistence: deactivate focus on mouse move
  useEffect(() => {
    const onMouseMove = () => {
      if (navLockedRef.current) {
        return;
      }

      deactivate();
    };

    window.addEventListener("mousemove", onMouseMove);
    return () => window.removeEventListener("mousemove", onMouseMove);
  }, [deactivate]);

  // Tab key for panel switching (not routed through NavInput)
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Tab" || overlayOpenRef.current) return;
      event.preventDefault();

      activate();
      navLockedRef.current = true;
      clearTimeout(navLockTimer.current);

      navLockTimer.current = setTimeout(() => {
        navLockedRef.current = false;
      }, NAV_LOCK_MS);

      setFocus((prev) => ({
        ...prev,
        active: true,
        analyzeAllFocused: false,
        panel: prev.panel === "songList" ? "sidebar" : "songList",
      }));
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [activate, setFocus]);

  const scrollToSong = useCallback(
    (index: number) => {
      const container = scrollRef.current;
      if (!container) {
        return;
      }

      const cards = container.querySelectorAll<HTMLElement>("[data-song-index]");

      for (const card of cards) {
        const cardIndex = Number(card.dataset.songIndex);
        if (cardIndex !== index) {
          continue;
        }

        const cardRect = card.getBoundingClientRect();
        const containerRect = container.getBoundingClientRect();
        const cardTop = cardRect.top - containerRect.top + container.scrollTop;
        const cardBottom = cardTop + cardRect.height;

        if (cardTop < container.scrollTop) {
          container.scrollTop = cardTop - BORDER_PADDING;
        } else if (cardBottom > container.scrollTop + containerRect.height) {
          container.scrollTop = cardBottom - containerRect.height + BORDER_PADDING;
        }
        break;
      }
    },
    [scrollRef],
  );

  useNavInput(
    useCallback(
      (action) => {
        if (overlayOpenRef.current) return;

        const hasDirection = action.up || action.down || action.left || action.right;
        const hasAny = hasDirection || action.confirm || action.back;

        if (!hasAny) return;

        if (action.back) {
          const handled = actionsRef.current.onSidebarBack?.();
          if (!handled) {
            onBackRef.current();
          }
          return;
        }

        if (actionsRef.current.isSidebarBusy?.()) return;

        activate();

        // Set nav lock to prevent mouse hover from overriding
        navLockedRef.current = true;
        clearTimeout(navLockTimer.current);
        navLockTimer.current = setTimeout(() => {
          navLockedRef.current = false;
        }, NAV_LOCK_MS);

        if (action.confirm) {
          const now = performance.now();

          setFocus((prev) => {
            if (!prev.active) return prev;
            if (now - lastConfirmAtRef.current < CONFIRM_COOLDOWN_MS) return prev;
            lastConfirmAtRef.current = now;

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
          return;
        }

        // Horizontal: sidebar cluster rows cycle sub-index via L/R; otherwise L/R switches panels.
        if (action.left || action.right) {
          let handled = false;

          setFocus((prev) => {
            if (prev.panel === "sidebar") {
              const subCount = actionsRef.current.sidebarSubCountByIndex.get(prev.sidebarIndex);
              if (subCount && subCount > 1) {
                const delta = action.left ? -1 : 1;
                const nextSub = Math.max(0, Math.min(subCount - 1, prev.sidebarSubIndex + delta));
                if (nextSub === prev.sidebarSubIndex) {
                  handled = true;
                  return prev;
                }
                handled = true;
                return { ...prev, sidebarSubIndex: nextSub, active: true };
              }
            }

            return prev;
          });

          if (handled) return;

          if (action.left) {
            setFocus((prev) => ({
              ...prev,
              panel: "sidebar",
              analyzeAllFocused: false,
              active: true,
            }));
            return;
          }

          setFocus((prev) => ({ ...prev, panel: "songList", active: true }));
          return;
        }

        // Vertical navigation
        if (action.up || action.down) {
          setFocus((prev) => {
            const next = { ...prev, active: true };

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
      },
      [activate, setFocus, actionsRef, scrollToSong],
    ),
  );

  // Cleanup nav lock timer
  useEffect(() => {
    return () => {
      clearTimeout(navLockTimer.current);
    };
  }, []);
}
