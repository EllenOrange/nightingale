import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { isTauri } from "@/bridge/runtime";

document.addEventListener("contextmenu", (e) => e.preventDefault());

interface WebBootstrap {
  config: typeof window.__NIGHTINGALE_APP_CONFIG__;
  songsMeta: typeof window.__NIGHTINGALE_SONGS_META__;
  dataPathPinned?: boolean;
  libraryPinned?: boolean;
}

/**
 * Tauri injects `window.__NIGHTINGALE_*` ahead of script execution via the
 * webview `initialization_script`. The web target gets the same payload from
 * `/api/bootstrap` and seeds the globals before React mounts so downstream
 * code can read them synchronously.
 */
async function loadWebBootstrap(): Promise<void> {
  if (isTauri) return;

  try {
    const res = await fetch("/api/bootstrap");
    if (!res.ok) {
      console.warn("bootstrap fetch failed", res.status);
      return;
    }
    const data = (await res.json()) as WebBootstrap;
    if (data.config) window.__NIGHTINGALE_APP_CONFIG__ = data.config;
    if (data.songsMeta) window.__NIGHTINGALE_SONGS_META__ = data.songsMeta;
    window.__NIGHTINGALE_SERVER_FLAGS__ = {
      dataPathPinned: Boolean(data.dataPathPinned),
      libraryPinned: Boolean(data.libraryPinned),
    };
  } catch (e) {
    console.warn("bootstrap fetch failed", e);
  }
}

void loadWebBootstrap().then(() => {
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
});
