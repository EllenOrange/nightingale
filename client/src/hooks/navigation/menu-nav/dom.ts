export const SIDEBAR_SUB_SELECTOR = "[data-sidebar-sub-index]";
export const SIDEBAR_NAV_SELECTOR = "[data-sidebar-nav-index]";
export const ANALYZE_ALL_SELECTOR = "[data-analyze-all-focus]";
export const SONG_SELECTOR = "[data-song-index]";

export interface SidebarSubTarget {
  sidebarIndex: number;
  sidebarSubIndex: number;
}

function finiteDatasetNumber(value: string | undefined): number | null {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : null;
}

export function blurActiveTextInput() {
  const active = document.activeElement;
  if (
    active instanceof HTMLElement &&
    (active.tagName === "INPUT" || active.tagName === "TEXTAREA" || active.isContentEditable)
  ) {
    active.blur();
  }
}

export function getHoveredSidebarSubTarget(target: Element | null): SidebarSubTarget | null {
  const subEl = target?.closest<HTMLElement>(SIDEBAR_SUB_SELECTOR);
  if (!subEl) {
    return null;
  }

  const parentEl = subEl.closest<HTMLElement>(SIDEBAR_NAV_SELECTOR);
  const sidebarIndex = finiteDatasetNumber(parentEl?.dataset.sidebarNavIndex);
  const sidebarSubIndex = finiteDatasetNumber(subEl.dataset.sidebarSubIndex);

  if (sidebarIndex === null || sidebarSubIndex === null) {
    return null;
  }

  return { sidebarIndex, sidebarSubIndex };
}

export function getHoveredSidebarIndex(target: Element | null): number | null {
  const sidebarEl = target?.closest<HTMLElement>(SIDEBAR_NAV_SELECTOR);
  return finiteDatasetNumber(sidebarEl?.dataset.sidebarNavIndex);
}

export function isAnalyzeAllTarget(target: Element | null): boolean {
  return target?.closest(ANALYZE_ALL_SELECTOR) != null;
}

export function getHoveredSongIndex(target: Element | null): number | null {
  const songEl = target?.closest<HTMLElement>(SONG_SELECTOR);
  return finiteDatasetNumber(songEl?.dataset.songIndex);
}
