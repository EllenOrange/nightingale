# Changelog

All notable changes to Nightingale are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The release pipeline (`.github/workflows/release.yml`) extracts the section
matching the pushed tag (e.g. `v0.6.0` -> `## [0.6.0]`) and uses it as the
GitHub Release body. If a section is missing the release is still created
with a fallback body, but ideally every tagged version has its own entry
below.

## [0.6.0-rc1] - 2026-05-10

### Highlights

- CJK lyric support — Japanese, Chinese, and Korean songs now go through a per-character forced-alignment path with romanized readings (Hepburn / pinyin / Revised Romanization) shown above each token. Japanese uses a hiragana-vocab wav2vec2 model fed through fugashi, which sidesteps the dense kanji vocabulary and matches natural speech far better than the default checkpoint.
- Parakeet v3 ASR (experimental) — alternative to Whisper for ~25 European languages. NeMo on CUDA, ONNX Runtime on CPU and Apple Silicon. Switchable from Settings → Analysis. Falls back to Whisper automatically if Parakeet returns no usable words.
- UltraStar Deluxe songs (experimental) — drop USDX bundles (.txt or .usdx plus sibling audio/vocals/instrumental/video) into your library and play them with their built-in pitch and lyric data. No analyzer pass needed; stem separation is skipped entirely when #VOCALS and #INSTRUMENTAL are provided. See [docs/usdx](https://nightingale.cafe/docs/usdx.html).
- Audio-reactive shader backgrounds — the 5-shader lineup is now 10 (Plasma, Waves, Nebula, Starfield, Sonar, Voronoi, Vortex, Metaballs, Spectrum, Oscilloscope) and they all react to your microphone input in real time when the mic is enabled.
- Persistent analyzer server — the Python analyzer is now a long-lived process talking to the app over a token-authenticated loopback TCP socket using NDJSON.
- In-app updater (macOS and Windows) — Nightingale now checks for new releases and can download and install updates from inside the app. A new "Check for updates" entry lives in the sidebar actions menu, with progress reporting and a one-click relaunch when the install finishes. Linux still ships the menu entry but opens the GitHub Releases page since the updater plugin isn't compiled in for Linux builds.

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
