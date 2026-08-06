# Phase 1 Implementation Plan: Party Layer (Path B)

> **STATUS: COMPLETE AND MERGED (2026-08-05).** Every step below (1 to 5) is built, tested, and shipped, plus the online-song-search feature, a round of owner UX feedback, and two code-review passes. It landed as PR #1 (`EllenOrange/nightingale`), **merged to `master`**. The per-step "results" write-ups and the review-fix notes are in `CLAUDE.md` (the sections headed "Phase 1 Step N results", "Online song search", "Feedback round", and "Code-review fixes"). This file is kept as the original plan of record; do not re-execute it. See the "Where things stand / next" note at the top of `CLAUDE.md` for what a fresh session should do.

Self-contained plan for a fresh session. Read `CLAUDE.md` first for background; this file assumes it.

**Architecture decision (settled, do not relitigate):** Path B. The party layer is built **into this fork**, as pages in the existing React app plus routes in the axum server. The design brief's filesystem IPC (`commands/*.json`, `state.json`, `events.log`) is **not** being implemented; it was designed around a limitation that does not exist. See `CLAUDE.md` Sections 4 and 5.1.

**Conventions:** no em dashes anywhere (use commas, colons, parentheses, or a spaced hyphen). Keep party code in clearly separated modules so the diff against upstream stays legible.

---

## 0. Ground truth (verified this session, cite rather than re-derive)

| Fact | Location |
| --- | --- |
| Playback starts from router state | `client/src/components/menu/song-list/song-details-sidebar.tsx:83` calls `navigate("/playback", { state: { song } })` |
| Playback route requires `song` in location state | `client/src/pages/playback/playback.tsx:19-31`, remounts per `song.file_hash` |
| Transport-agnostic `invoke` / `listen` | `client/src/bridge/runtime.ts:154,158`; web transport posts to `/api/cmd/<name>` and multiplexes `/ws` |
| Server routes | `client/src-server/src/main.rs:77-82` |
| Command dispatch (~490 lines) | `client/src-server/src/commands.rs` |
| Event bus (Tauri-style `{type, payload}` broadcast) | `client/src-server/src/events.rs` |
| Shared jukebox state, **currently unused by frontend** | `client/src-server/src/jukebox.rs`, WS frames in `ws.rs:99-121` |
| `Song` type (what playback needs) | `client/src/types/Song.ts`, key field `file_hash` |

**Command args are camelCase.** `fileHash`, not `file_hash`. This is a Tauri serde convention carried into the web API and it will silently 400 otherwise.

**The server does not play audio; the browser does.** There is no server-side play/pause/stop. This is why Step 1 exists.

### Working test fixture

Bad Romance is already downloaded, analyzed, and known-good:

- file: `C:\Users\Eleanor Todd\Music\Karaoke\Lady Gaga - Bad Romance.mp4`
- `file_hash`: `7df0c494f3c8a7ccb974b6265e5b4f0e`
- 98 segments, 525 word timings, 81% coverage, source `lyrics`

### Environment

```bash
# pnpm lives here because corepack hit EPERM on Program Files
%APPDATA%\npm\pnpm.cmd
```

- `cargo desktop dev` is **broken on Windows** (`xtask` uses `Command::new("pnpm")`, which cannot resolve `.cmd`). Use `pnpm tauri dev` from `client/`.
- Do **not** run `target/debug/Nightingale.exe` directly; debug builds load the frontend from `devUrl` and show no window.
- Never run the desktop app and `server.exe` at once; they share `songs.db`.
- Active analyzer config: `separator = "demucs"`, `align_backend = "whisperx"`.

```bash
# server (has the HTTP API; use for anything scripted)
./target/debug/server.exe --bind 127.0.0.1:8080 --data "C:\Users\Eleanor Todd\Documents\Nightengale-Plus\.nightingale" --library "C:\Users\Eleanor Todd\Music\Karaoke"
```

---

## 1. Testing strategy (read before writing code)

**There is no test infrastructure in this repo.** No `test` script in `client/package.json`, no `#[cfg(test)]` in `app-core` or `client/src-server`. Part of this plan is establishing the minimum that makes the work verifiable.

Three tiers, in order of cost:

1. **Rust unit tests** via `#[cfg(test)]` and `cargo test -p server`. Use for pure logic: queue state transitions, ordering, dedup. No new dependencies.
2. **Integration scripts** in `scripts/party-test/` run with **plain `node`** (Node 24 has a global `WebSocket`, so no `ws` dependency is needed) plus `curl` for HTTP. These drive the real server and assert on real responses.
3. **Manual visual confirmation** for anything involving actual audio or video playback. Automate everything up to the point where a human has to look at a screen, and be explicit about which assertions are automated versus eyeballed.

**Do not add vitest/jest** unless a step genuinely needs component testing. The value here is in integration, not unit-testing React.

Every step below states its tests and an acceptance criterion. A step is not done until its acceptance criterion is demonstrated, not merely believed.

---

## Step 0: Branch hygiene (ALREADY DONE)

The `party-layer` branch exists and already carries `CLAUDE.md` and this plan as its first commit. Start there:

```bash
git checkout party-layer
git log --oneline -1
```

Base commit is `382f0b5`, the state validated end to end. Do not move to a release tag without re-running the Phase 0 checks. `master` is left as a clean mirror of upstream.

Note: `client/src-tauri/Cargo.toml` may show as modified with an empty diff. That is a CRLF line-ending artifact from a build tool, not a real change. Leave it unstaged.

---

## Step 2 note on ordering

Steps are numbered by dependency, not by user value. **Step 1 is the only genuinely unproven piece.** Ingest, analysis, and lyrics are all proven working (Phase 0). If Step 1 fails, the whole Path B design needs revisiting, so do it first and do not build a queue on an unvalidated foundation.

---

## Step 1: Remote-controlled playback (the spike)

**Goal:** a WS message causes the TV browser tab to start playing a specific song.

The pieces already exist and are simply not connected: `jukebox.rs` holds shared state and `ws.rs` rebroadcasts it, but `client/src` has zero references to jukebox.

### Implementation

1. **Server:** extend `JukeboxState` in `client/src-server/src/jukebox.rs` with a play intent, for example `requested_song_hash: Option<String>` plus a monotonically increasing `play_token: u64` so repeat plays of the same song still trigger.
2. **Server:** add a `POST /api/cmd/party_play {fileHash}` arm in `commands.rs` that mutates jukebox state and calls the existing broadcast, reusing `broadcast_jukebox` in `ws.rs`.
3. **Frontend:** add `client/src/hooks/party/use-remote-playback.ts`. It calls `listen("jukebox.state", ...)` from `bridge/runtime.ts`, and when `play_token` changes it loads the `Song` (via the existing songs query or a `load_songs` call filtered by hash) and calls `navigate("/playback", { state: { song } })`, exactly as `song-details-sidebar.tsx:83` does.
4. **Frontend:** mount the hook once, high in the tree in `client/src/App.tsx`, so it is active on the menu screen.

**Design note:** send only the hash over the wire, not the whole `Song`. The client already knows how to fetch songs, and a hash keeps the WS payload stable if the `Song` type changes upstream.

### Tests

**T1.1 (automated, Rust):** unit test for the token increment. Assert that two successive `party_play` calls with the same hash produce different `play_token` values, and that state mutation is visible via `snapshot()`.

```bash
cargo test -p server
```

**T1.2 (automated, integration):** `scripts/party-test/t1-remote-play.mjs`
- open a WS to `ws://127.0.0.1:8080/ws`
- `POST /api/cmd/party_play {"fileHash":"7df0c494f3c8a7ccb974b6265e5b4f0e"}`
- assert a frame arrives within 2s whose payload contains that hash and a `play_token`
- fail with a non-zero exit code otherwise, so it can gate later work

**T1.3 (manual, the real acceptance):** run `server.exe`, open `http://127.0.0.1:8080` in a browser and leave it on the menu. From a second terminal run the `party_play` curl. The browser must navigate to playback and start the song **without any interaction**.

**Acceptance criterion:** T1.3 works from a cold browser tab sitting on the menu. If it only works when the tab is already on the playback route, the hook is mounted too low in the tree.

### Risks

- The WS `listen` resolves only after the socket opens (`runtime.ts` `webListen` awaits `ensureSocket`). Subscribe before issuing commands or the broadcast is lost, since `tokio::broadcast` does not replay.
- Browsers block autoplay with sound until a user gesture. The TV tab may need one interaction after load. **Check this early**, it affects how a party actually starts, and note the finding in `CLAUDE.md`.

---

## Step 2: Ingest command (YouTube to ready-to-sing)

**Goal:** one call takes a YouTube URL or search string and produces an analyzed, LRCLIB-lyriced song.

This automates exactly the manual sequence proven in Phase 0.

### Implementation

Add `client/src-server/src/party/ingest.rs`:

1. Resolve the query. Accept a URL directly, or `ytsearch1:<query>`.
2. Shell out to yt-dlp:
   ```
   -f "bv*+ba/b" -S "res:1080" --merge-output-format mp4
   --embed-metadata --windows-filenames --no-playlist
   --ffmpeg-location <data>/vendor/ffmpeg.exe
   -o "<library>/%(artist)s - %(title)s.%(ext)s"
   ```
   **`--embed-metadata` is mandatory.** Without tags the file indexes as "Unknown Artist" and Nightingale's `search_lrclib_lyrics` returns an empty array, silently falling back to slow, lossy Whisper transcription. This is a proven failure, not a hypothetical.
3. `app_core::start_scan()` and wait for the new hash to appear.
4. `app_core::enqueue_one(&hash)`, watch the analysis queue.
5. Fetch LRCLIB and `save_lyrics_and_realign`. Prefer Nightingale's own `search_lrclib_lyrics` (now that metadata exists); fall back to a direct `https://lrclib.net/api/search?track_name=&artist_name=` query.
6. Emit progress events on the existing `EventBus` at each transition.

**Use plain lyric lines, never LRCLIB's timed LRC.** LRCLIB entries match the album cut, and music videos differ in length (Bad Romance: 295s versus 308.4s), so raw timings are offset. Plain lines trigger forced alignment against the actual audio and get both the words and the timing right.

### Tests

**T2.1 (automated, Rust):** unit test the yt-dlp argument builder as a pure function returning `Vec<String>`. Assert `--embed-metadata` is always present, the output template is under the library dir, and a URL is not double-wrapped in `ytsearch1:`. This is cheap and guards the one flag whose absence breaks everything downstream.

**T2.2 (automated, integration):** `scripts/party-test/t2-ingest.mjs` against a **short, unambiguous** video (pick something under 60s to keep the loop fast; do not use Bad Romance, it is already cached).
- assert the file lands in the library directory
- assert `load_songs` reports non-empty `artist` (that is the `--embed-metadata` regression test)
- assert it reaches `is_analyzed: true`
- assert the transcript's `source` is `lyrics`, not `generated`, when LRCLIB has the track

**T2.3 (automated):** re-run the same ingest and assert it does not redownload or re-analyze (idempotency by hash).

**Acceptance criterion:** a single call, given only a search string, yields `is_analyzed: true` with `source: "lyrics"` and a populated `artist` field.

### Risks

- yt-dlp may need a JS runtime (deno) for some videos. It worked without one this session; if extraction starts failing, install deno and note it.
- Age-restricted or region-locked videos need `--cookies-from-browser`. Out of scope for v1, but fail with a clear error rather than hanging.

---

## Step 3: Party queue and guest page

**Goal:** multiple guests add songs from phones and see a shared queue.

### Implementation

1. **State:** a queue in `client/src-server/src/party/queue.rs`. Entries: `{id, file_hash, title, artist, requested_by, status, added_at, error}`. Status machine: `queued -> downloading -> analyzing -> ready -> playing -> done`, with `error` reachable from any state. Persist to `<data>/party_queue.json` using temp-file-plus-rename so a crash cannot leave a torn file.
2. **Routes:** `party_queue_list`, `party_queue_add {query, requestedBy}`, `party_queue_remove {id}`, `party_queue_reorder {id, position}`.
3. **Guest page:** `client/src/pages/party/party.tsx` at route `/party`. Name entry stored in `localStorage`, search, add, live queue. Reuse existing song-list components rather than rebuilding them.
4. **Auto-advance:** on `song_ended`, mark `done`, pick the next `ready` entry, and issue the Step 1 play. If the next entry is not ready, wait and show "processing next".

**Concurrency:** analysis stays at **1** (8 GB VRAM). Queue additions beyond the first must wait rather than running in parallel.

### Tests

**T3.1 (automated, Rust):** state machine unit tests. Every illegal transition is rejected (`done -> playing`, `queued -> playing` without `ready`). Table-driven; this is the highest-value unit test in the plan because the transitions are pure logic and easy to get subtly wrong.

**T3.2 (automated, Rust):** persistence round-trip. Write a queue, reload, assert equality. Assert a partial write cannot be observed (write to temp, rename, never write in place).

**T3.3 (automated, integration):** `scripts/party-test/t3-multi-guest.mjs`
- two WS clients simulating two phones
- both add a song; assert both see both entries
- assert `requested_by` attribution is preserved per entry
- remove one; assert both clients see the removal

**T3.4 (manual):** open `/party` on an actual phone on the LAN, add a song, watch it appear on the TV browser. This is the real requirement 1 and cannot be fully automated.

**Acceptance criterion:** two devices queue songs, both see the same ordered queue, and playback advances automatically from one to the next without intervention.

---

## Step 4: Admin controls

**Goal:** transport plus live audio controls.

**No authentication.** Decided by the owner: this runs on a home LAN only, so the brief's password-protected admin (Section 7.6) is deliberately dropped. Anyone on the network can control playback, which is the correct trust model for a house party and matches how the rest of the server already behaves (there is no auth anywhere in it today). Consequence to respect: **never expose this to the internet.** If that ever changes, auth becomes a prerequisite, not an enhancement.

This also means `/admin` is a convenience view, not a security boundary. Do not build UI that implies otherwise.

### Implementation

1. Extend the jukebox state with `paused`, `guide_vocal`, `key_offset`, `volume`. The frontend playback session subscribes and applies them, reusing the existing controls rather than reimplementing.
2. Routes: `party_control_{pause,resume,skip,restart}` and `party_set_{guide_vocal,key,volume}`.
3. Admin page at `/admin`: reorder, remove, clear, retry failed, plus the transport and audio controls.

### Tests

**T4.1 (automated, integration):** `scripts/party-test/t4-admin.mjs` asserts each control changes the broadcast jukebox state, and that a second connected client observes the change (this is the property that matters, since control is shared).

**T4.2 (manual):** during playback, move the guide-vocal slider and confirm the audible level changes; change key and confirm pitch shifts.

**Acceptance criterion:** admin controls work from a second device while a song plays, and the change is visible to every connected client.

---

## Step 5: QR join and TV idle page

1. `GET /qr` renders a QR for `http://<host>:<port>/party`.
2. `GET /tv` shows now-playing plus up-next plus the QR, for display during gaps.
3. **Decide the display policy** so `/tv` and Nightingale's own playback do not fight for the screen (flagged in the brief and still unresolved).

**Test:** manual. Scan the QR with a phone that has never seen the app and confirm it lands on the guest page.

---

## Definition of done for Phase 1

A guest on a phone adds a YouTube song; it downloads with metadata, separates with Demucs, gets LRCLIB lyrics force-aligned, and plays on the TV with synced highlighting, and the queue advances automatically when it ends.

Mapped to the brief's acceptance criteria: 1 (multi-guest queue), 2 (original video), 3 (auto separation), 4 (auto download and processing), 7 (synced lyrics). Criterion 5 (guide vocal) lands in Step 4. Criterion 6 is partial: key change in Step 4, scoring deferred, pitch correction out of scope.

---

## Known limitations to accept, not fix

- **Overlapping lead and backing vocals** separate imperfectly. Inherent to source separation, confirmed audibly. Not a tuning problem.
- **Roughly 4 to 5 minutes** from request to singable for a fresh song. Pre-seed a library before a party.
- **Analysis concurrency is 1** on 8 GB VRAM.
- **Three upstream Windows bugs** documented in `CLAUDE.md` Section 3.1. The orphaned-`uv` one is worth fixing in this fork since we are already patching (reap child processes on exit).

## Deployment (unresolved, needs its own pass)

`scripts/install.sh` assumes systemd, Caddy, and avahi, none of which exist on Windows. mDNS needs Bonjour or a static LAN IP. Microphone capture needs HTTPS or a secure context, which matters for scoring. Treat as a separate work item after Phase 1 functions locally.
