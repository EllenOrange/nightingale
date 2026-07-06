/// <reference types="vite/client" />

import type { ServerFlags } from "./bridge/server-flags";
import type { AppConfig } from "./types/AppConfig";
import type { SongsMeta } from "./types/SongsMeta";

declare global {
  interface Window {
    __NIGHTINGALE_APP_CONFIG__?: AppConfig;
    __NIGHTINGALE_SONGS_META__?: SongsMeta;
    __NIGHTINGALE_SERVER_FLAGS__?: ServerFlags;
  }
}

export {};
