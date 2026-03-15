import { defineConfig } from "astro/config";
import sitemap from "@astrojs/sitemap";

export default defineConfig({
  outDir: "dist",
  site: "https://nightingale.pages.dev",
  integrations: [
    sitemap({
      customPages: [
        "https://nightingale.pages.dev/docs/",
        "https://nightingale.pages.dev/docs/introduction.html",
        "https://nightingale.pages.dev/docs/getting-started.html",
        "https://nightingale.pages.dev/docs/controls.html",
        "https://nightingale.pages.dev/docs/how-it-works.html",
        "https://nightingale.pages.dev/docs/stems.html",
        "https://nightingale.pages.dev/docs/lyrics.html",
        "https://nightingale.pages.dev/docs/scoring.html",
        "https://nightingale.pages.dev/docs/backgrounds.html",
        "https://nightingale.pages.dev/docs/profiles.html",
        "https://nightingale.pages.dev/docs/configuration.html",
        "https://nightingale.pages.dev/docs/building.html",
        "https://nightingale.pages.dev/docs/troubleshooting.html",
      ],
    }),
  ],
});
