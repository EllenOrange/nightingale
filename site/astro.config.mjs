import { defineConfig } from "astro/config";
import sitemap from "@astrojs/sitemap";

export default defineConfig({
  outDir: "dist",
  site: "https://nightingale.cafe",
  integrations: [
    sitemap({
      customPages: [
        "https://nightingale.cafe/docs/",
        "https://nightingale.cafe/docs/introduction.html",
        "https://nightingale.cafe/docs/getting-started.html",
        "https://nightingale.cafe/docs/controls.html",
        "https://nightingale.cafe/docs/how-it-works.html",
        "https://nightingale.cafe/docs/stems.html",
        "https://nightingale.cafe/docs/lyrics.html",
        "https://nightingale.cafe/docs/scoring.html",
        "https://nightingale.cafe/docs/backgrounds.html",
        "https://nightingale.cafe/docs/profiles.html",
        "https://nightingale.cafe/docs/configuration.html",
        "https://nightingale.cafe/docs/building.html",
        "https://nightingale.cafe/docs/troubleshooting.html",
      ],
    }),
  ],
});
