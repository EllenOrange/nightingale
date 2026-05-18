import { playbackAdapter } from "@/bridge/playback";
import { useLatestRef } from "@/hooks/use-latest-ref";
import { PixabayFlavorLibrary } from "@/lib/playback/pixabay-flavor-library";
import { getNextFlavor, type VideoFlavor } from "@/lib/playback/video-flavor";
import {
  fetchPixabayVideos,
  onPixabayVideoDownloaded,
  type PixabayVideoDownloaded,
} from "@/bridge/playback";
import { useCallback, useEffect, useRef, useState, type RefObject } from "react";

const SLOT_COUNT = 3;
const STALL_TIMEOUT_MS = 4000;
const MIN_QUEUE_BEFORE_REFRESH = 2;
const REFRESH_COOLDOWN_MS = 8000;
const NEAR_END_TOLERANCE_SEC = 0.05;

export interface PixabaySlot {
  ref: RefObject<HTMLVideoElement | null>;
  src: string;
  isActive: boolean;
}

export interface UsePixabaySlotsResult {
  slots: PixabaySlot[];
  onActiveEnded: () => void;
}

function nextSlotIndex(slot: number): number {
  return (slot + 1) % SLOT_COUNT;
}

function isAtEnd(video: HTMLVideoElement): boolean {
  return (
    Number.isFinite(video.duration) &&
    video.duration > 0 &&
    video.currentTime >= video.duration - NEAR_END_TOLERANCE_SEC
  );
}

function safePlay(video: HTMLVideoElement): void {
  void video.play().catch(() => {});
}

function emptySlotStrings(): string[] {
  return Array.from({ length: SLOT_COUNT }, () => "");
}

function emptySlotFlavors(): (VideoFlavor | null)[] {
  return Array.from({ length: SLOT_COUNT }, () => null);
}

export function usePixabaySlots(flavor: VideoFlavor, isPlaying: boolean): UsePixabaySlotsResult {
  const videoRefs: RefObject<HTMLVideoElement | null>[] = [
    useRef<HTMLVideoElement | null>(null),
    useRef<HTMLVideoElement | null>(null),
    useRef<HTMLVideoElement | null>(null),
  ];

  const [activeIdx, setActiveIdx] = useState(0);
  const activeIdxRef = useRef(0);

  const [srcs, setSrcs] = useState<string[]>(emptySlotStrings);
  const srcsRef = useRef<string[]>(emptySlotStrings());
  const slotFlavorsRef = useRef<(VideoFlavor | null)[]>(emptySlotFlavors());

  const libraryRef = useRef<PixabayFlavorLibrary | null>(null);
  if (!libraryRef.current) libraryRef.current = new PixabayFlavorLibrary();
  const library = libraryRef.current;

  const flavorRef = useLatestRef(flavor);
  const isPlayingRef = useLatestRef(isPlaying);

  const adapterReadyRef = useRef(false);
  const inflightFetchesRef = useRef(new Set<VideoFlavor>());
  const lastRefreshAtRef = useRef(new Map<VideoFlavor, number>());
  const pendingDownloadsRef = useRef<PixabayVideoDownloaded[]>([]);

  const writeSrcs = useCallback((next: string[]) => {
    srcsRef.current = next;
    setSrcs(next);
  }, []);

  const activateSlot = useCallback(
    (slot: number) => {
      activeIdxRef.current = slot;
      setActiveIdx(slot);
      const video = videoRefs[slot].current;
      if (video && isPlayingRef.current) safePlay(video);
    },
    [isPlayingRef, videoRefs],
  );

  const ensureAdapter = useCallback(async () => {
    if (adapterReadyRef.current) return;
    await playbackAdapter.init();
    adapterReadyRef.current = true;
  }, []);

  const toUrl = useCallback((path: string): string => {
    return playbackAdapter.toMediaUrl(path);
  }, []);

  const setSlotSrc = useCallback(
    (slot: number, url: string, slotFlavor: VideoFlavor) => {
      const next = [...srcsRef.current];
      next[slot] = url;
      writeSrcs(next);
      slotFlavorsRef.current[slot] = slotFlavor;
      library.markUnready(url);

      const video = videoRefs[slot].current;
      if (video) {
        video.addEventListener(
          "canplay",
          () => {
            library.markReady(url);
          },
          { once: true },
        );
      }
    },
    [library, videoRefs, writeSrcs],
  );

  const activateWhenReady = useCallback(
    (slot: number, url: string, slotFlavor: VideoFlavor) => {
      const tryActivate = () => {
        if (
          srcsRef.current[slot] === url &&
          slotFlavorsRef.current[slot] === slotFlavor &&
          flavorRef.current === slotFlavor
        ) {
          activateSlot(slot);
        }
      };

      if (library.isReady(url)) {
        tryActivate();
        return;
      }

      const video = videoRefs[slot].current;
      if (!video) return;

      video.addEventListener("canplay", tryActivate, { once: true });
    },
    [activateSlot, flavorRef, library, videoRefs],
  );

  const restartIfEnded = useCallback(
    (slot: number) => {
      const video = videoRefs[slot].current;
      if (!video) return;

      if (video.ended || isAtEnd(video)) {
        video.currentTime = 0;
      }

      if (isPlayingRef.current) safePlay(video);
    },
    [isPlayingRef, videoRefs],
  );

  const refillSiblingSlots = useCallback(
    (slotFlavor: VideoFlavor) => {
      const active = activeIdxRef.current;
      for (let slot = 0; slot < SLOT_COUNT; slot++) {
        if (slot === active) continue;
        if (slotFlavorsRef.current[slot] !== slotFlavor || !srcsRef.current[slot]) {
          const url = library.pullUrl(slotFlavor, false) ?? library.pullUrl(slotFlavor, true);
          if (url) setSlotSrc(slot, url, slotFlavor);
        }
      }
    },
    [library, setSlotSrc],
  );

  const ensureFlavorPlayback = useCallback(
    (slotFlavor: VideoFlavor) => {
      const active = activeIdxRef.current;

      if (srcsRef.current[active] && slotFlavorsRef.current[active] === slotFlavor) {
        refillSiblingSlots(slotFlavor);
        return;
      }

      for (let slot = 0; slot < SLOT_COUNT; slot++) {
        if (
          slot !== active &&
          slotFlavorsRef.current[slot] === slotFlavor &&
          srcsRef.current[slot] &&
          library.isReady(srcsRef.current[slot])
        ) {
          activateSlot(slot);
          refillSiblingSlots(slotFlavor);
          return;
        }
      }

      for (let slot = 0; slot < SLOT_COUNT; slot++) {
        const candidate = srcsRef.current[slot];
        if (slot !== active && slotFlavorsRef.current[slot] === slotFlavor && candidate) {
          activateWhenReady(slot, candidate, slotFlavor);
          refillSiblingSlots(slotFlavor);
          return;
        }
      }

      const playSlot = nextSlotIndex(active);
      const preloadSlot = nextSlotIndex(playSlot);

      const playUrl = library.pullUrl(slotFlavor, true);
      if (!playUrl) return;
      setSlotSrc(playSlot, playUrl, slotFlavor);
      activateWhenReady(playSlot, playUrl, slotFlavor);

      const preloadUrl = library.pullUrl(slotFlavor, false) ?? library.pullUrl(slotFlavor, true);
      if (preloadUrl) setSlotSrc(preloadSlot, preloadUrl, slotFlavor);
    },
    [activateSlot, activateWhenReady, library, refillSiblingSlots, setSlotSrc],
  );

  const refreshFlavor = useCallback(
    async (slotFlavor: VideoFlavor) => {
      const now = Date.now();
      const lastRefresh = lastRefreshAtRef.current.get(slotFlavor) ?? 0;
      if (now - lastRefresh < REFRESH_COOLDOWN_MS) return;
      if (inflightFetchesRef.current.has(slotFlavor)) return;

      lastRefreshAtRef.current.set(slotFlavor, now);
      inflightFetchesRef.current.add(slotFlavor);

      try {
        await ensureAdapter();
        const paths = await fetchPixabayVideos(slotFlavor);
        const urls = paths.map((path) => toUrl(path));
        library.registerUrls(slotFlavor, urls);

        if (flavorRef.current === slotFlavor) ensureFlavorPlayback(slotFlavor);
      } finally {
        inflightFetchesRef.current.delete(slotFlavor);
      }
    },
    [ensureAdapter, ensureFlavorPlayback, flavorRef, library, toUrl],
  );

  const ingestDownload = useCallback(
    (event: PixabayVideoDownloaded) => {
      if (!adapterReadyRef.current) {
        pendingDownloadsRef.current.push(event);
        return;
      }

      const downloadedFlavor = event.flavor as VideoFlavor;
      library.registerUrls(downloadedFlavor, [toUrl(event.path)]);

      if (event.evictedPath) {
        library.removeUrl(downloadedFlavor, toUrl(event.evictedPath));
      }

      if (downloadedFlavor === flavorRef.current) {
        ensureFlavorPlayback(downloadedFlavor);
      }
    },
    [ensureFlavorPlayback, flavorRef, library, toUrl],
  );

  const flushPendingDownloads = useCallback(() => {
    if (!adapterReadyRef.current || pendingDownloadsRef.current.length === 0) return;

    const queued = [...pendingDownloadsRef.current];
    pendingDownloadsRef.current = [];
    queued.forEach((event) => ingestDownload(event));
  }, [ingestDownload]);

  const handleEnded = useCallback(() => {
    const currentFlavor = flavorRef.current;
    const active = activeIdxRef.current;
    const upcoming = nextSlotIndex(active);
    const upcomingSrc = srcsRef.current[upcoming];

    const refillActiveFromQueue = () => {
      const refill = library.pullUrl(currentFlavor, false) ?? library.pullUrl(currentFlavor, true);
      if (refill) setSlotSrc(active, refill, currentFlavor);
    };

    if (
      upcomingSrc &&
      slotFlavorsRef.current[upcoming] === currentFlavor &&
      library.isReady(upcomingSrc)
    ) {
      activateSlot(upcoming);
      refillActiveFromQueue();
    } else {
      let switched = false;
      for (let slot = 0; slot < SLOT_COUNT; slot++) {
        if (
          slot !== active &&
          slotFlavorsRef.current[slot] === currentFlavor &&
          srcsRef.current[slot] &&
          library.isReady(srcsRef.current[slot])
        ) {
          activateSlot(slot);
          refillActiveFromQueue();
          switched = true;
          break;
        }
      }

      if (!switched) {
        const fallback = library.pullUrl(currentFlavor, true);
        if (fallback) {
          setSlotSrc(upcoming, fallback, currentFlavor);
          activateWhenReady(upcoming, fallback, currentFlavor);
        }
        restartIfEnded(active);
      }
    }

    if (library.queueLength(currentFlavor) < MIN_QUEUE_BEFORE_REFRESH) {
      void refreshFlavor(currentFlavor);
    }
  }, [
    activateSlot,
    activateWhenReady,
    flavorRef,
    library,
    refreshFlavor,
    restartIfEnded,
    setSlotSrc,
  ]);

  useEffect(() => {
    ensureFlavorPlayback(flavor);
    void refreshFlavor(flavor);
    void refreshFlavor(getNextFlavor(flavor));
  }, [ensureFlavorPlayback, flavor, refreshFlavor]);

  useEffect(() => {
    const video = videoRefs[activeIdx].current;
    if (!video) return;

    if (isPlaying) safePlay(video);
    else video.pause();
  }, [activeIdx, isPlaying, videoRefs]);

  useEffect(() => {
    const video = videoRefs[activeIdx].current;
    if (!video) return;

    let lastTime = video.currentTime;
    const onTimeUpdate = () => {
      lastTime = video.currentTime;
    };
    video.addEventListener("timeupdate", onTimeUpdate);

    const stallTimer = window.setInterval(() => {
      if (!video.paused && video.currentTime === lastTime) {
        handleEnded();
      }
    }, STALL_TIMEOUT_MS);

    return () => {
      video.removeEventListener("timeupdate", onTimeUpdate);
      window.clearInterval(stallTimer);
    };
  }, [activeIdx, handleEnded, videoRefs]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    void ensureAdapter().then(() => {
      flushPendingDownloads();
      ensureFlavorPlayback(flavorRef.current);
    });

    void onPixabayVideoDownloaded((event) => {
      ingestDownload(event);
    }).then((fn) => {
      if (disposed) {
        void Promise.resolve(fn()).catch(() => {});
        return;
      }
      unlisten = fn;
    });

    return () => {
      disposed = true;
      if (!unlisten) return;
      void Promise.resolve(unlisten()).catch(() => {});
    };
  }, [ensureAdapter, ensureFlavorPlayback, flavorRef, flushPendingDownloads, ingestDownload]);

  const slots: PixabaySlot[] = videoRefs.map((ref, slot) => ({
    ref,
    src: srcs[slot],
    isActive: slot === activeIdx,
  }));

  return { slots, onActiveEnded: handleEnded };
}
