# Changelog

All notable changes to Nightingale are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The release pipeline (`.github/workflows/release.yml`) extracts the section
matching the pushed tag (e.g. `v0.6.0` -> `## [0.6.0]`) and uses it as the
GitHub Release body. If a section is missing the release is still created
with a fallback body, but ideally every tagged version has its own entry
below.

## [0.7.1] - 2026-05-27

### Fixes

- Folder libraries now detect `.opus` audio files and serve them with an Ogg audio MIME type for browser playback.
- Changing a song's language can now realign existing lyrics without forcing a fresh transcription, and the selected language is preserved for the alignment pass.
- Edited lyrics keep the song's previous language hint when re-running alignment, avoiding accidental language resets.
- The self-hosted update/install command no longer pipes the installer through `sudo`, matching the script's own privilege handling.

### Improvements

- Polished the language-selection dialog with explicit force-transcribe vs realign choices and controlled selection state.
- Tightened the library sidebar scroll/layout behavior around the main navigation list.

## [0.7.0] - 2026-05-20

### Highlights

- Self-hosted web mode (v1) — Nightingale now ships a second binary, `server`, that runs the same app over HTTP on a Linux box on the LAN. The React bundle is embedded into the binary via `rust-embed`, browsers on phones/laptops/tablets/TVs all open the app at `http://<hostname>.local`. A one-shot `scripts/install.sh` drops a systemd unit, a Caddy front-door (HTTP on `:80`, opt-in HTTPS via Caddy's local CA on `:443` for mic capture), and an avahi advertisement onto the host so it's reachable without DNS. See [docs/self-hosted](https://nightingale.cafe/docs/self-hosted.html).
- Jellyfin media provider — connect the library to a Jellyfin server from the sidebar. Items are scanned via paginated `GET /Items` with `SortName` for stable enumeration; bytes are downloaded lazily on first analysis into `cache/sources/<file_hash>.<container>` and rekeyed to a true Blake3 hash, so the rest of the karaoke pipeline (stems, transcription, shifts) behaves identically to a folder library.
- Navidrome / Subsonic media provider — same shape as Jellyfin, but talking the [Subsonic API](http://www.subsonic.org/pages/api.jsp). Audio-only (Navidrome doesn't serve video). Auth uses per-call `MD5(password + salt)` tokens; the password is encrypted at rest in `config.json`.
- Lyrics editor with LRCLIB browser — every song now has an "Edit lyrics" entry that opens an editor seeded with the current transcript. When LRCLIB returns multiple candidate matches, a second tab lets you carousel through them and apply one with a single click. Saving re-runs alignment with your edits, so timing stays accurate.
- Sidebar restructure — Library actions (folder picker, Jellyfin/Navidrome connect, rescan), cache actions (clear all / videos / models), and the theme toggle moved out of the avatar dropdown into dedicated clusters. The Library row exposes its source buttons inline with live status badges (green/grey/amber) and tooltips showing the reachable hostname or the connection error. The avatar dropdown is now Profile / Settings / Update / About / Exit / Re-run Setup.

### Improvements

- Persistent scroll — sidebar and song-list scroll positions are preserved when navigating away and back, via a new `usePersistentScroll` hook keyed by panel id.
- Higher-contrast sidebar surfaces — various sidebar/menu surfaces had their contrast bumped after the cluster restructure, mostly for the badges and the focused/hovered ring states.
- `app-core` crate extraction — all cross-runtime logic (config, scanner, library DB, vendor bootstrap, sources, secrets, media server) was lifted out of `client/src-tauri` into a new `app-core` crate consumed by both the Tauri desktop client and the new self-hosted `server`. Drops a chunk of duplicated code and removes the few client/server divergences that had crept in.
- `library_db` modularization — the single 1k-line `library_db.rs` was split into `connection`, `migrations`, `queries`, `songs`, `analysis_queue`, `remote`, and `rebase`. Remote-source helpers live in `library_db::remote` so Jellyfin/Navidrome share the prune/upsert plumbing.
- Single-focus refactor — `use-menu-nav.ts` was split into `menu-nav/{use-menu-nav-input, use-menu-nav-refs, use-mouse-menu-focus, use-nav-lock, use-scroll-to-song, use-tab-panel-switch}`. Resolves a pile of edge cases where focus could land in two panels at once or get stuck after dialog dismissal.
- `mic_mirroring` → `mic_monitoring` — the setting and its config keys are renamed (`mic_monitoring`, `mic_monitor_gain`, `mic_active`). Older configs with `mic_mirroring` / `mic_mirror_gain` are read transparently via serde aliases and rewritten under the new names on next save. Existing UI hotkeys (`R` to toggle, etc.) are unchanged.

### Documentation

- New [Self-Hosted Web Mode](https://nightingale.cafe/docs/self-hosted.html) page with the full install / HTTPS / firewall / co-existing-with-your-own-Caddy story.
- New [Library Sources](https://nightingale.cafe/docs/library-sources.html) page covering the Folder / Jellyfin / Navidrome options and the at-rest credential envelope.
- Updated [Lyrics & Transcription](https://nightingale.cafe/docs/lyrics.html) page with a section on the in-app lyrics editor and the LRCLIB candidate browser.
- Updated [Configuration](https://nightingale.cafe/docs/configuration.html) page with the new `library_source` key and the `mic_monitor_gain` rename.

## [0.6.0] - 2026-05-10

### Highlights

- CJK lyric support — Japanese, Chinese, and Korean songs now go through a per-character forced-alignment path with romanized readings (Hepburn / pinyin / Revised Romanization) shown above each token. Japanese uses a hiragana-vocab wav2vec2 model fed through fugashi, which sidesteps the dense kanji vocabulary and matches natural speech far better than the default checkpoint.
- Parakeet v3 ASR (experimental) — alternative to Whisper for ~25 European languages. NeMo on CUDA, ONNX Runtime on CPU and Apple Silicon. Switchable from Settings → Analysis. Falls back to Whisper automatically if Parakeet returns no usable words.
- UltraStar Deluxe songs (experimental) — drop USDX bundles (.txt or .usdx plus sibling audio/vocals/instrumental/video) into your library and play them with their built-in pitch and lyric data. No analyzer pass needed; stem separation is skipped entirely when #VOCALS and #INSTRUMENTAL are provided. See [docs/usdx](https://nightingale.cafe/docs/usdx.html).
- Audio-reactive shader backgrounds — the 5-shader lineup is now 10 (Plasma, Waves, Nebula, Starfield, Sonar, Voronoi, Vortex, Metaballs, Spectrum, Oscilloscope) and they all react to your microphone input in real time when the mic is enabled.
- Persistent analyzer server — the Python analyzer is now a long-lived process talking to the app over a token-authenticated loopback TCP socket using NDJSON.
- In-app updater (macOS and Windows) — Nightingale now checks for new releases and can download and install updates from inside the app. A new **Update** entry lives in the sidebar actions menu, with progress reporting and a one-click relaunch when the install finishes. Linux still ships the menu entry but opens the GitHub Releases page since the updater plugin isn't compiled in for Linux builds.

### Improvements

- Mic monitor gain slider — the live mic-mirror volume is now a 0–200% slider in Settings, replacing the previous fixed level.
- Cleaner client architecture — playback state lives in dedicated React contexts (transport, transcript, mic, theme) instead of being prop-drilled through playback-inner. The shader visualizer was also extracted from the old monolithic video-background component.
- Pitch and reactive analysis moved to the client — both run in TypeScript over raw PCM samples streamed from Tauri, dropping a chunk of native code and making them easier to tune.
- Single GPU model at a time — Whisper, Parakeet, the alignment model, and the stem separator are now loaded one at a time and freed between stages, lowering peak VRAM and reducing OOMs on smaller GPUs.
- Alignment robustness — several edge cases in the WhisperX alignment path were fixed, including better handling of silence-bounded segments and tokens that fall outside the model vocab.

### Fixes

- Various typo and copy fixes in playback UI strings.

### Documentation

- New [UltraStar Deluxe](https://nightingale.cafe/docs/usdx.html) docs page covering detection, supported tags, the BPM/GAP timing model, and limitations.
- Expanded [Lyrics & Transcription](https://nightingale.cafe/docs/lyrics.html) docs with ASR engine selection and a CJK languages section.
- Updated [Backgrounds](https://nightingale.cafe/docs/backgrounds.html), [Configuration](https://nightingale.cafe/docs/configuration.html), and [How It Works](https://nightingale.cafe/docs/how-it-works.html) pages.

## [0.5.0] - 2026-04-06

Initial public release tracked in this changelog. See the
[v0.5.0 release notes](https://github.com/rzru/nightingale/releases/tag/v0.5.0)
on GitHub for the full artifact list.
