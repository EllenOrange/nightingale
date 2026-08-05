# Nightingale Plus: Home Karaoke System

Working notes and plan for this fork. Source of truth for intent is the owner's design brief ("Design Brief: Home Karaoke System (Nightingale + Party Server)"). This file records how that brief maps onto what is actually in the code, plus current status.

## 0. Conventions (read first)

- **No em dashes.** Use commas, colons, semicolons, parentheses, or a spaced hyphen ( - ). This applies to code comments, docs, commit messages, and UI copy.
- Keep the Nightingale patch minimal and isolated so upstream merges stay clean.
- Prefer reusing Nightingale's existing code paths over reimplementing them.
- Study `vicwomg/pikaraoke` and `xuancong84/OpenHomeKaraoke` for proven patterns before inventing new ones.

## 1. Goal

A self-hosted, LAN-only karaoke system for house parties. Guests use their phones to search for and queue songs; songs play on a TV through Nightingale, which handles vocal separation, synced lyrics over the original video, guide vocals, scoring, and key/tempo.

Nightingale natively covers requirements 2, 3, 5, 7, plus scoring and key shifting. The new work is requirement 1 (multi-guest phone queue) and 4 (automatic YouTube download), plus playback control glue.

## 2. Repository and remotes

- This repo is a fork of [`rzru/nightingale`](https://github.com/rzru/nightingale).
- `origin` -> `https://github.com/EllenOrange/nightingale.git` (owner's fork, account EllenOrange)
- `upstream` -> `https://github.com/rzru/nightingale.git`
- Currently on `master`, cloned at commit `382f0b5`. **Open item:** the brief calls for pinning to a release tag (it named v0.3.2). Reconcile the pin before writing the patch.

## 3. Environment status (Windows 11, RTX 4070 laptop)

| Tool | Status |
| --- | --- |
| Git + GitHub CLI (`gh`, authed as EllenOrange) | Installed |
| MSVC C++ build tools (`cl.exe` 14.44) | Installed |
| Rust 1.97.1, target `x86_64-pc-windows-msvc` | Installed |
| WebView2 runtime 150.x | Present |
| Node.js | 24.19.0 |
| npm / pnpm | 11.17.0 / 11.20.0 |
| First successful build | **Done.** Full workspace builds clean |

`corepack enable` fails with EPERM writing shims into `C:\Program Files\nodejs`, so pnpm was installed with `npm -g` and lives at `%APPDATA%\npm\pnpm.cmd`.

**Measured build times (first run, this machine):**

| Step | Time |
| --- | --- |
| `pnpm install` | 10.2 s |
| `pnpm build` (tsc + vite, 2319 modules) | 1.31 s |
| `cargo build --workspace` (clean, debug) | 2 m 35 s |
| `cargo build -p server` after touching one `.rs` | 35.5 s |

Artifacts in `target/debug/`: `Nightingale.exe` (33.9 MB), `server.exe` (22 MB), `xtask.exe`. `server.exe --help` runs and accepts `--bind` (default `0.0.0.0:8080`), `--data`, `--library`.

**First-launch setup is complete and CUDA works on Windows.** Data folder is `.nightingale/` inside this repo (git-excluded), 5.88 GB after bootstrap, `vendor/.ready` present. Verified with the bootstrapped venv:

```
torch 2.10.0+cu126
cuda_available True
device NVIDIA GeForce RTX 4070 Laptop GPU
capability (8, 9)          # Ada Lovelace, correct for a 4070
```

GPU is an RTX 4070 Laptop, 8188 MiB VRAM, driver 546.30. The driver natively reports CUDA 12.3 while PyTorch wants 12.6; it works via CUDA minor version compatibility, proven by `is_available` returning True rather than by inference. If odd CUDA errors show up under heavy Demucs or WhisperX load, a driver update is the first thing to try. The 8 GB VRAM confirms the brief's `analysis_concurrency = 1` constraint.

The Python analyzer is extracted to `vendor/analyzer/`: `pipeline.py`, `stems.py`, `transcribe.py`, `align.py`, `ctc_align.py`, `qwen_align.py`, `parakeet.py`, `key_detect.py`, `gpu.py`, `cjk.py`, `hallucination.py`, `language.py`, `whisper_compat.py`, `audio.py`, `analyze.py`, and `server.py` (the persistent analyzer process the README describes, spoken to over a token-authenticated loopback TCP socket with newline-delimited JSON).

Build order matters: run `pnpm build` before `cargo build`, because `rust-embed` pulls `client/dist/` in at compile time and `tauri-build` expects `frontendDist`.

### 3.1 Two Windows gotchas that cost real time

**`cargo desktop dev` is broken on Windows.** `xtask/src/main.rs` does `Command::new("pnpm")`, and Rust's `std::process::Command` on Windows resolves only `.exe`, never `.cmd` or `.bat`. Since pnpm ships as `pnpm.cmd`, the alias dies with `Failed to run pnpm: program not found`. Workaround: skip the alias and run `pnpm tauri dev` (or `pnpm tauri build`) from `client/` directly. A one-line fix (use `pnpm.cmd` on `cfg!(windows)`, or spawn via `cmd /c`) would make the alias work and is plausibly upstreamable.

**Setup can wedge itself with "Failed to clear vendor directory: Access is denied. (os error 5)".** Setup begins by calling `clear_vendor_dir()` (`app-core/src/vendor.rs:50`), a plain `remove_dir_all` on `vendor/`. But `uv.exe` lives *inside* `vendor/`, and if the app exits while `uv` is mid-install, `uv.exe` survives as an orphan (parent gone) still holding its own image. Windows cannot delete a running executable, so every subsequent setup attempt fails. This is an upstream bug: setup deletes the directory containing its own still-running tool, and nothing reaps the orphan.

Fix: `Get-Process uv` and `Stop-Process` it, then `Remove-Item -Recurse -Force` the `vendor/` folder, then rerun setup. Verify the orphan with `Get-CimInstance Win32_Process -Filter "ProcessId = <pid>"` and check `ParentProcessId` no longer resolves. If we take Path B, reaping child processes on exit is a small, worthwhile fix.

**Do not launch `target/debug/Nightingale.exe` directly.** `tauri-build` emits `cfg(dev)` for debug profiles, so a debug binary loads the frontend from `devUrl` (`http://localhost:1420`) rather than the bundled `dist/`. With no Vite server running, the frontend never loads. Combined with `tauri.conf.json` setting `"create": false, "visible": false` (the real window is created programmatically in Rust), the process starts, writes its data files, and reports as healthy while showing **no window at all**. The only visible artifact is a 13x13 pixel helper window at the top-left corner. Use `pnpm tauri dev` for a debug run, or build release for a standalone binary.

**Install gotcha, recorded so it is not rediscovered:** an interrupted `winget` run leaves an orphaned process holding the Windows Installer lock, and every later install fails with **error 1618, "Another installation is already in progress."** Fix: find the stale `winget` process by `StartTime` and `Stop-Process` it, then retry. Do not kill the long-lived `msiexec` with no start time; that is the Windows Installer service. Some winget installs also request elevation, so a UAC prompt may need approving on the desktop.

Build commands (from the README and `.cargo/config.toml`): `cargo desktop dev` and `cargo desktop build`, which alias to the `xtask` crate.

## 4. IMPORTANT: the brief's architectural premise does not match this codebase

The brief states that Nightingale is "a single-crate native Rust app built on the **Bevy** game engine" with "no router, no IPC, and no scripting surface," and therefore proposes forking it to add a filesystem IPC control channel (`commands/*.json`, `state.json`, `events.log`).

That description does not match current `master`. Verified findings:

1. **It is not Bevy.** There is zero `bevy` dependency anywhere in the workspace. It is a **Tauri 2 app (Rust backend + React/TypeScript frontend)**. So `bevy_kira_audio` playback-instance polling and "a Bevy plugin named `party_control`" (brief Section 6) do not apply as written.
2. **It is not a single crate.** It is a Cargo workspace: `app-core`, `client/src-tauri`, `client/src-server`, `xtask`.
3. **It already is a web server.** `client/src-server` is an **axum** server (the README's "self-hosted web mode"), built to be reached from phones and TVs on the LAN at `<hostname>.local`. It exposes:
   - `GET /api/bootstrap`
   - `POST /api/cmd/:name` (a generic command dispatch surface, roughly 490 lines of handlers)
   - `GET /api/asset`, `GET /media/:hash/:kind`
   - `GET|WS /ws`
4. **It already has a remote-control command vocabulary.** Commands in `client/src-server/src/commands.rs` that map directly onto brief needs include `enqueue_one` / `enqueue_all` / `load_analysis_queue` (analysis), `shift_key` (brief's `set_key`), `shift_tempo`, `trigger_scan` / `set_library_source` (getting new files into the library index), `load_songs`, `get_audio_paths`, `ensure_mp3_stems`, `add_score`, `is_ready`.
5. **It already has a real-time event bus.** `client/src-server/src/events.rs` broadcasts Tauri-style `{type, payload}` envelopes to every WebSocket client. This is a live push stream, which is strictly better than tailing an append-only `events.log`.
6. **It already has multi-client shared playback state.** `client/src-server/src/jukebox.rs` holds a single shared `JukeboxState` (current song, paused, position, pitch, rms, mic owner, controller, theme, score) rebroadcast to all browsers over WS.

**Consequence:** the filesystem IPC contract in brief Section 5 was designed to work around a limitation that does not exist. Building it now would mean adding a slower, lossier, poll-based channel next to a push-based HTTP + WebSocket API that already carries most of the same information.

### 4.1 The decisive finding: the server does not play audio, the browser does

This reframes the whole integration and is the single most important fact in this document.

- `client/src/bridge/playback.ts`: the server's only media role is `GET /api/asset?path=...`, a file proxy. Audio is mixed and played **in the browser** (`hooks/use-audio-player.ts`, `contexts/playback/playback-transport-context.tsx`).
- `commands.rs` contains **no** `play`, `pause`, `stop`, `skip`, or `seek` command. Verified by grep.
- `pages/playback/playback.tsx` starts a song from **React Router location state**: it needs a `Song` object passed via `navigate("/playback", { state: { song } })`, and remounts per `song.file_hash`.

So the "TV" is just a browser tab that plays audio itself. Guide vocal level, key, pause, and volume are live state inside that browser session, not on the server.

Therefore **no approach is zero-fork**. Nothing server-side can start playback, so any hands-free party needs a frontend change letting the TV tab obey a remote signal. That is also exactly what the dormant `jukebox.rs` looks designed for and was never wired to.

### 4.2 What is missing for a party

1. **No YouTube ingestion at all.** Zero `youtube` or `yt-dlp` hits across `.rs`, `.ts`, `.tsx`, `.py`. Requirement 4 is entirely absent.
2. **No multi-guest request queue.** The only queue is `AnalysisQueue`, the ML processing queue. No `requested_by`, no per-guest ownership, no reordering.
3. **The jukebox is dormant.** `jukebox.rs` plus its WS protocol (`claim_mic`, `release_mic`, controller, shared position and score) exist server-side, but `client/src` has **zero** references to it, case-insensitive.
4. **No auth of any kind** on the server. **DECIDED: keep it that way.** The owner dropped the brief's password-protected admin (Section 7.6) because this runs on a home LAN only. Anyone on the network can control playback, which is the right trust model for a house party. Hard consequence: **never expose this to the internet.** If that ever changes, auth becomes a prerequisite rather than an enhancement, and `/admin` must stop being treated as a mere convenience view.

Roughly 60 to 70 percent of the plumbing exists (library, analysis, playback, lyrics, mobile-friendly UI, command API, event bus). The gap is precisely the PiKaraoke layer.

## 5. Decisions needed from the owner

1. **Integration channel (highest impact).** Given Section 4, **drop the filesystem IPC layer**. See Section 5.1 for the two remaining candidate architectures and the recommendation.
2. **Host OS.** The brief assumed Linux. Reality is **Windows 11**. ~~Needs a check that CUDA works.~~ **CUDA is confirmed working on Windows (see Section 3), so this is no longer a blocker.** What remains Windows-specific: mDNS (Bonjour or a static LAN IP instead of avahi), path handling, the launcher, and the fact that `scripts/install.sh` (systemd + Caddy + avahi) is Linux-only, so the deployment story in brief Section 10 needs rewriting for Windows.
3. **Party Server language.** Brief recommends Python 3.11+ with FastAPI (yt-dlp is a Python library; OpenHomeKaraoke is a Python reference). Node/TS is viable and matches the owner's usual stack. Not yet confirmed.
4. **Does a fork patch remain necessary at all?** Possibly only for gaps such as playback-end detection or a play-arbitrary-path entry point. Decide after Phase 0 recon.
5. **Upstream pin.** `master` at `382f0b5`, or a release tag.

### 5.1 Path A versus Path B

**Path A, separate Party Server.** A Python/FastAPI or Node app beside Nightingale owning the queue, YouTube search, and yt-dlp. It calls `POST /api/cmd/:name` for what genuinely exists server-side (`trigger_scan` to index a download, `enqueue_one` to analyze, `load_songs`, `load_analysis_queue`) and listens on `/ws`. Plus a fork shim so the TV tab obeys remote play and control.

**Path B, build the party layer into the fork.** A `/party` guest page and admin controls in the existing React app, extra axum routes, and yt-dlp as a subprocess.

| | Path A | Path B |
| --- | --- | --- |
| Fork patch size | Small but real (relay shim) | Large |
| Iteration speed | Slight edge on queue logic | Near parity thanks to HMR |
| yt-dlp | Native Python library | Subprocess, or a sidecar |
| Live controls | Need a relay protocol | Native, no protocol |
| Guest experience | Two UIs, two URLs | One UI, one QR code |
| Upstream sync | Cheap | Real, ongoing cost |
| Processes to run | Two | One |

**Build speed, measured against the config rather than assumed.** An early draft of this plan leaned on "Rust rebuilds are slow" to favour Path A. That argument mostly does not survive scrutiny:

- Most party work is frontend, and the frontend has full HMR. `tauri.conf.json` sets `beforeDevCommand: "pnpm dev"` against Vite on port 1420 with `@vitejs/plugin-react` (Fast Refresh), and Vite is configured to ignore `src-tauri`. React edits are sub-second and preserve state.
- Rust incremental compilation is on. The root `Cargo.toml` defines no custom profiles, so the dev default (`incremental = true`) applies.
- Path B's Rust surface is small: a few axum routes plus a yt-dlp subprocess. `app-core` (12k lines) and the playback engine are untouched.

What stays true, now with measured numbers from Section 3: a frontend rebuild is **1.31 s** while an incremental Rust rebuild of one crate is **35.5 s**, roughly a 27x difference, with relink dominating the Rust side. There is no Tauri equivalent of Dioxus-style hot patching, and the app restarts on each Rust edit, losing in-app state.

So the shape of the original argument was right, but the scope was wrong. Slow rebuilds apply only to the Rust slice of the work, which for Path B is a few axum routes and a yt-dlp subprocess, not the queue and UI work where iteration actually concentrates.

Two unused speedups worth trying if the 35 s cycle grates: `lld-link` via `.cargo/config.toml` (note that file is tracked upstream, so a local override avoids fork drift), and `sccache`. Still to verify empirically: whether `rust-embed` (used without `debug-embed`) serves `dist/` from disk in debug builds, which would mean frontend changes need no Rust rebuild at all; and the web server needs a Vite proxy added for dev, since none is configured.

**DECIDED: Path B.** The party layer is built into the fork. Rationale: the relay protocol cut against A harder than build times cut against B, because every live control lives in the browser playback session, so A would have to invent a protocol to reach state that B simply touches. Phase 0 also surfaced three Windows bugs in upstream code, making A's premise of a barely-touched fork unrealistic here regardless.

Accepted costs: upstream merges get harder, and Rust edits cost a ~35 s rebuild-and-restart cycle. Mitigation: keep party code in clearly separated modules and pages so the diff stays legible, and lean on Vite HMR for the frontend majority of the work.

## 6. Plan

### Phase 0 results: end-to-end pipeline proven over the existing API

A full YouTube-to-karaoke run was completed **with zero code changes to Nightingale**, driven entirely through `server.exe`'s HTTP API. This is the go/no-go test from the plan below, and it passed.

Sequence: `yt-dlp` download into `C:\Users\Eleanor Todd\Music\Karaoke\` -> start `server.exe --library <folder>` -> `POST /api/cmd/trigger_scan` -> `POST /api/cmd/load_songs` (to get `file_hash`) -> `POST /api/cmd/enqueue_one {fileHash}` -> poll `load_analysis_queue` -> `get_audio_paths`.

**Measured on a 5:08 (308s) 1080p music video:**

| Metric | Value |
| --- | --- |
| Analysis wall time | **265 s (4 m 25 s)** |
| Peak VRAM | 3264 MiB of 8188 |
| Peak GPU SM utilisation | 100 % (mem bandwidth 76-80 %) |
| Stems produced | instrumental 7.4 MB, vocals 5.5 MB (mp3) |
| Transcript | 32 segments, **177 word-level timings**, language `en` |
| Key detected | `Am` |

CUDA is genuinely doing the work, confirmed by `nvidia-smi dmon` showing SM at 100 %, not merely by `torch.cuda.is_available()`. Framebuffer drops between phases (3264 -> 1378 MiB) show `gpu.py`'s `gpu_model` eviction working as designed.

Cache layout per song, keyed by blake3 file hash, with key and tempo encoded in the stem filenames (this is how playback variants are cached):

```
cache/<hash>_instrumental_<key>_<tempo>.mp3
cache/<hash>_vocals_<key>_<tempo>.mp3
cache/<hash>_transcript.json     # {language, segments[{text,start,end,words[{word,start,end,score}]}], source, key, tempo}
cache/<hash>_cover.jpg
```

**Gotchas found while driving the API:**

- **Command arguments are camelCase.** `file_hash` is rejected; `fileHash` works. Tauri's serde convention carries into the web API. Whoever writes the Party Server client needs this.
- **Downloaded files carry no metadata.** Artist and album both indexed as "Unknown". Pass `--embed-metadata` to yt-dlp, or supply title/artist separately, or the party library becomes a wall of "Unknown Artist".
- **Do not run the desktop app and `server.exe` at once.** They share `songs.db`; stop one before starting the other. The library source persists to `config.json`, so a folder pinned via `server --library` is visible to the desktop app afterwards.
- **`NIGHTINGALE_LIBRARY_PATH` is server-only.** It is referenced solely in `src-server`, so the desktop app ignores it.
- **Per-process VRAM is unavailable on Windows.** Consumer GPUs in WDDM mode report `N/A` for `--query-compute-apps=used_gpu_memory`. Monitor device-level stats (`nvidia-smi dmon`, `--query-gpu`) instead.

**Implication for the timeline:** roughly 4 to 5 minutes from request to singable for a fresh song, at the upper end of the README's 2-5 minute estimate. Transcription and alignment dominate the back half (58 % to 80 % took 143 s, versus the first 58 % in 102 s). Pre-seeding a library before a party matters.

### Lyric quality: two fixes found, both material

The default pipeline produced poor karaoke on the first real song. Two separate problems, each with a measured fix.

**Problem 1: WhisperX transcription misses most of the song.** On Bad Romance it covered only **32%** of the track (100s of 308s), with 8 gaps over 3s, the largest 28.7s, and 177 words. Not the hallucination filter (that only strips subtitle artifacts like translator credits); the vocal-activity detection simply failed to find vocals across two thirds of the song.

**Fix: LRCLIB lyrics plus forced alignment.** Fetch the lyric text from LRCLIB and `POST /api/cmd/save_lyrics {fileHash, lines[]}`, which calls `save_lyrics_and_realign`. Coverage went 32% -> 80%, words 177 -> 525, gaps 8 -> 1.

Do **not** apply LRCLIB's timed LRC directly: its entries are the ~295s album cut while the music video is 308.4s, so the timings are offset. Supplying plain lines and letting Nightingale align against the actual audio gets correct words *and* correct timing.

**Critical dependency:** Nightingale's own `search_lrclib_lyrics` returned an empty array, because it matches on artist/title tags and the yt-dlp download indexed as "Unknown Artist". The party server MUST write metadata at download time or the LRCLIB path silently fails and every song falls back to slow, lossy transcription.

**Problem 2: the default separator hurts both audio and alignment.** `stems.py` documents that the UVR karaoke model "isolates lead vocals, leaving backing vocals in the instrumental stem". On a heavily layered track that means audible vocals in the instrumental, and, because those parts are missing from the vocals stem, alignment has little to lock onto in dense passages and collapses (segments with physically impossible word density, e.g. 5 words in 0.5s).

**Fix: `separator = "demucs"`.** Config values: `separator` is `"karaoke"` (default) or `"demucs"`; `align_backend` is `"whisperx"` (default), `"ctc"`, or `"qwen"`.

Measured on the same song, same LRCLIB lyric text throughout:

| Config | weak segs | collapsed segs | coverage | median score | zero-score words |
| --- | --- | --- | --- | --- | --- |
| karaoke + whisperx (default) | 17 | 5 | 80% | 0.43 | 22 |
| karaoke + ctc | 66 | 3 | 78% | 0.08 | 35 |
| **demucs + whisperx (winner)** | **6** | **1** | **81%** | **0.53** | **11** |
| demucs + ctc | 64 | 1 | 78% | 0.18 | 17 |

**Methodological caveat:** confidence scores are not comparable across aligners, so the score-derived columns (weak segments, median, zero-score) cannot fairly rank whisperx against ctc. The aligner-agnostic metrics are **collapsed segments** and **coverage**, and on those Demucs is the clear win (5 -> 1 collapsed) while ctc gives no coverage benefit (78% vs 81%).

**Recommended defaults: `separator = "demucs"`, `align_backend = "whisperx"`, LRCLIB-first with Whisper as fallback.**

Timing caution: the Demucs full reanalyze took 44s versus 265s for the original run, but that comparison is confounded, since the first run included cold model downloads and warmup. Do not quote it as a speed win without a clean re-test.

### Phase 0: build and recon (current)

1. Finish installing Node.js and pnpm.
2. Get a clean `cargo desktop build` (and confirm the `server` binary builds, since that is the LAN web mode).
3. Run the app once, complete first-launch setup (it downloads ffmpeg, uv, Python, PyTorch, and ML models automatically), and confirm CUDA is detected on the 4070.
4. Answer the brief's Section 6.2 checklist against the **real** (Tauri/axum) architecture, and write `INTEGRATION_NOTES.md` covering:
   - song/library model, IDs, and cache layout
   - how to play a specific song, and whether an arbitrary file path can be played or must first be indexed (`trigger_scan` / `set_library_source` are the likely path)
   - playback-end detection, which drives auto-advance
   - guide-vocal level control
   - key control (`shift_key`)
   - analysis trigger (`enqueue_one`) and how completion is signalled
   - screen/state gating for accepting a play command
5. Spike: drive a song end to end from `curl` or a WS client against the running server, with no code changes. This is the go/no-go test for Section 5 decision 1.

### Phase 1 Step 1 results: remote-controlled playback proven

The Step 1 spike (PHASE1_PLAN.md) is **done and validated**. A WS message now drives the TV browser tab into playback with no interaction.

- **Server:** `JukeboxState` gained `requested_song_hash` and a monotonic `play_token` (`jukebox.rs`). New party command surface at `POST /api/cmd/party_play {fileHash}` and `party_song_by_hash {fileHash}`, isolated in `client/src-server/src/party/`. `party_*` commands are routed before the upstream dispatcher in `handle_cmd`, since they need the full `AppState` (jukebox), not just the event bus. `app_core` gained a `song_by_hash` helper and a `Song` re-export (the only upstream-file touch, two lines in `lib.rs`).
- **Frontend:** `client/src/hooks/party/use-remote-playback.ts` listens for the `jukebox` event (note: the real event name is `jukebox`, **not** `jukebox.state` as the plan assumed), and on a `play_token` change resolves the hash to a `Song` and does the same `navigate("/playback", { state: { song } })` the sidebar uses. Mounted once inside `<BrowserRouter>` in `App.tsx`, inert on Tauri.
- **Tests:** T1.1 (`cargo test -p server`, token increment + camelCase rejection) and T1.2 (`scripts/party-test/t1-remote-play.mjs`, drives a live server, asserts advancing token) both pass. T1.2 is written to be re-runnable against a long-lived server (baselines off the connect snapshot, since jukebox state is in-memory for the process lifetime).

**Autoplay finding (the risk the plan flagged): not a problem here.** From a cold browser tab sitting on the menu, the remote play navigates *and the stem audio starts on its own*, confirmed by ear on this machine (Windows, WebView-class browser, `http://127.0.0.1:8080`). The source video element autoplays muted (audio is the Web Audio stem mix, not the video track). So a party can start hands-free without a priming gesture on the TV. Revisit only if a stricter browser blocks it.

### Phase 1 Step 2 results: YouTube ingest proven end to end

Step 2 (PHASE1_PLAN.md) is **done and validated**. One call now takes a URL or search string to an analyzed, lyric-aligned song.

- **Server:** `client/src-server/src/party/ingest.rs`. `POST /api/cmd/party_ingest {query}` spawns the pipeline on a background thread and reports progress as `party.ingest` events (`downloading` -> `scanning` -> `analyzing` -> `ready`, or `error`). The pipeline: resolve yt-dlp -> download with `--embed-metadata` -> `start_scan` and poll for the file's hash by path -> `enqueue_one` and poll the analysis queue to completion -> report the final `transcript_source`.
- **No separate LRCLIB step is needed.** The analyzer already calls `fetch_lrclib_lyrics` automatically (`analyzer.rs:675`) when the song has real artist/title tags, so `--embed-metadata` alone yields `transcript_source = Lyrics` (LRCLIB text run through forced alignment). Confirmed by the server log: "Using pre-fetched lyrics ... Lyrics loaded: 9 lines". The manual `save_lyrics` path the plan described is only a fallback for when metadata is missing, which `--embed-metadata` prevents.
- **`transcript_source` enum values:** `Lyrics` = LRCLIB text + forced alignment (the good path), `Generated` = Whisper fallback, `Lrc` = provided timed LRC without alignment, `Usdx`. The party layer wants `Lyrics`.
- **yt-dlp is not on PATH and has no shim on this machine.** It lives at `%LOCALAPPDATA%\Microsoft\WinGet\Packages\yt-dlp.yt-dlp_Microsoft.Winget.Source_8wekyb3d8bbwe\yt-dlp.exe`. `resolve_ytdlp()` checks `NIGHTINGALE_YTDLP` (set this on the server for reliability), then PATH, then that WinGet path. yt-dlp warns "No supported JavaScript runtime" (deno) but still works for standard videos; install deno if extraction starts failing.
- **`after_move:filepath`** prints the final on-disk path on both a fresh download and an idempotent re-run, so the pipeline locates the file to scan without a directory diff. Warnings go to stderr; the printed path is the last stdout line.
- **Tests:** T2.1 (`cargo test -p server`, arg builder: always `--embed-metadata`, template under library, URL not wrapped, search wrapped once). T2.2 + T2.3 (`scripts/party-test/t2-ingest.mjs`, real end-to-end against a live server, using a 26s fixture, The Beatles - Her Majesty, `watch?v=Mh1hKt5kQ_4`). The script self-cleans the fixture first so it exercises a real analysis every run, asserts `analyzed`/artist/`source=Lyrics`, then asserts the re-ingest is idempotent (no second `analyzing` stage, same hash). Both pass.

### Phase 1 Step 3 results: party queue, guest page, and auto-advance

Step 3 (PHASE1_PLAN.md) is **done and validated**. Multiple guests can queue songs from phones, everyone sees the same live queue, and playback advances automatically.

- **Queue model (`party/queue.rs`, pure and unit-tested):** `PartyQueue` of `QueueEntry {id, query, file_hash, title, artist, requested_by, status, added_at, error}`. Status machine `queued -> downloading -> analyzing -> ready -> playing -> done`, plus `queued -> ready` (library song already analyzed), `queued -> analyzing` (library song needing analysis, no download), `error` from any live state, and `error -> queued` retry. `set_status` rejects illegal transitions. Persisted to `<data>/party_queue.json` via temp-file + rename (atomic).
- **Service (`party/queue_service.rs`):** `PartyQueueStore` (in `AppState`) plus routes `party_queue_list/add/remove/reorder`, `party_song_ended`, `party_skip`. Every mutation persists and broadcasts a `party.queue` event. A single async ingest worker drains `queued` entries one at a time (serial, respects analysis-concurrency 1), running the blocking download/analysis via `spawn_blocking` and reusing the Step 2 building blocks. `maybe_start_next` promotes the first `ready` entry to `playing` and issues the Step 1 play whenever nothing is playing; it fires on add, on an entry becoming ready, and on `party_song_ended`. On load, in-flight statuses are sanitized (crash recovery).
- **Song-end detection:** the TV's playback tab reports `party_song_ended {fileHash}` when the transport's `isFinished` flips (`hooks/party/use-report-song-end.ts`, mounted inside the playback providers). The server guards on the hash so a stale end cannot double-advance.
- **Guest page (`pages/party/party.tsx`, route `/party`):** mobile-first, name stored in `localStorage`, debounced local-library search with per-result Add, a "Request from YouTube" fallback that enqueues the raw query, and the live queue via `hooks/party/use-party-queue.ts`. Reuses the `loadSongs` data layer, not the desktop menu shell.
- **Role split (important):** `use-remote-playback` now ignores play signals when the tab is on a controller route (`/party`, `/admin`), so only the TV screen follows songs into the video; guest phones stay on their page. Verified with two browser tabs: adding a song from the phone navigated the TV to `/playback` while the phone stayed on `/party` and showed the song as "Now playing".
- **Tests:** T3.1 (state machine, table-driven) and T3.2 (persistence round-trip, atomic) in `cargo test -p server` (15 unit tests total pass). T3.3 (`scripts/party-test/t3-multi-guest.mjs`, two WS clients see the same queue, attribution preserved, removal propagates) and the auto-advance acceptance (`scripts/party-test/t3b-autoadvance.mjs`, A auto-plays, B waits, on song-end A->done and B auto-plays with a fresh jukebox token) both pass. Note: queue entries are camelCase on the wire (`fileHash`, `requestedBy`, `addedAt`).

### Phase 1 Step 4 results: admin controls

Step 4 (PHASE1_PLAN.md) is **done and validated** (except live key change, deliberately descoped, see below).

- **Server:** `JukeboxState` gained `restart_token`, `guide_vocal: Option<f32>`, `key_offset: Option<i32>`, `volume: Option<f32>` (Option so an untouched control never forces a value like muting to 0). `party/controls.rs` adds `party_control_pause/resume/restart`, `party_set_guide_vocal/volume/key` (values clamped), and `party_control_skip` aliases the queue's `party_skip`. `party_queue_clear` added. Each mutates the jukebox and broadcasts.
- **Frontend applier (`hooks/party/use-apply-remote-controls.ts`, mounted in the playback tree):** listens to `jukebox` and applies pause/resume (transport `handlePause`/`handleContinue`), guide vocal (`setGuideVolume`), master volume (`setMasterVolume`), and restart (`seek(0)`). It applies only controls that *changed* between frames, diffing against neutral defaults on the first frame so a TV joining mid-party adopts the current guide/volume/pause. Critical detail learned the hard way: the applier subscribes to the already-open socket and so misses the WS connect snapshot; if it skipped its first received frame as baseline, the first control after a song started would be lost. It now adopts state on the first frame (only the restart-seek is suppressed there).
- **Master volume did not exist** in the audio engine (instrumental wired straight to `ctx.destination`). Added a master `GainNode` in `hooks/use-audio-player.ts` between the whole mix (instrumental + the guide-vocal chain) and the speakers, plus `setMasterVolume`, threaded through the transport context. Verified non-destructive: audio still plays through it.
- **Admin page (`pages/admin/admin.tsx`, route `/admin`):** transport buttons, guide + volume sliders (seeded from live jukebox state via `hooks/party/use-jukebox.ts`), and queue management (reorder up/down against true array indices, remove, retry failed, clear). A controller route, so the TV-follow guard keeps it from becoming a screen. No auth by design (home LAN).
- **Live key change is descoped.** The playback session has no live pitch node; key change is an offline stem re-render triggered from the menu (`shift_key`, ~minutes). A live remote key control would need that re-render + a stem re-fetch, so it is deferred. The `party_set_key` command and `key_offset` field exist (and broadcast) but are not applied on the TV yet; the admin page does not expose a key control.
- **Tests:** T4.1 (`scripts/party-test/t4-admin.mjs`): two WS clients, every control (pause/resume/guide/volume/key/restart) changes the broadcast state and the second client observes it; out-of-range values are clamped. Passes, and T1/T3/T3b still pass. Manual browser check confirmed remote pause raises the TV's pause overlay, resume dismisses it, and audio plays through the new master gain node with synced lyrics.

**Resolved open question (was in Section 5.1):** rust-embed is used without `debug-embed`, so the **debug** server serves `client/dist/` from disk at runtime. A frontend-only change needs just `pnpm build` (no `cargo build`); a already-open browser tab must still hard-reload to fetch the new bundle (react-router navigation alone keeps the stale JS). The server binary only needs rebuilding for Rust changes.

### Phase 1: MVP

Party Server with: YouTube search and yt-dlp download into the library folder, trigger analysis, mark ready, auto-advancing playback, a phone guest UI (search, add, see queue), and a minimal admin (skip, clear). Acceptance: a guest on a phone adds a YouTube song; it downloads, gets separated and lyric-aligned, and plays on the TV, and the queue advances when it ends.

### Phase 2: full control and UX

Transport (pause, resume, restart), guide-vocal slider, key change, volume, queue reorder and remove, live updates to phones, QR join code, `/tv` splash page, combined local plus YouTube search, singer-name attribution.

### Phase 3: polish

Surface scoring, duplicate and recently-played handling, pre-seed and favorites tooling, crash recovery, settings UI, cookies for restricted content.

## 7. Known constraints

- Analysis concurrency is **1** on 8 GB VRAM. Expect roughly 5 to 8 minutes from request to playable for a fresh song, so pre-seeding a library before a party matters.
- Trust model is LAN-only. Admin password gates control; guests need no account. Do not expose to the internet.
- Nightingale is GPL-3.0. Fine for personal use; relevant only if the fork is ever distributed.
- Pitch correction (real-time autotune) is out of scope for v1.
- Decide a display policy so Nightingale and any `/tv` page do not fight for the TV screen.
