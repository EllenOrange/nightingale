<p align="center">
  <img src="client/src/assets/images/logo.png" alt="Nightingale" width="400">
</p>

<p align="center">
  Karaoke from any song in your music library, powered by neural networks.
</p>

---

Nightingale scans your music folder, separates lead vocals from instrumentals using the [UVR Karaoke model](https://github.com/Anjok07/ultimatevocalremovergui) (or [Demucs](https://github.com/facebookresearch/demucs)), transcribes lyrics with word-level timestamps via [WhisperX](https://github.com/m-bain/whisperX), and plays it all back with synchronized highlighting, pitch scoring, key/tempo controls, profiles, and dynamic backgrounds.

Ships as a single binary. No manual installation of Python, ffmpeg, or ML models required — everything is downloaded and bootstrapped automatically on first launch.

## Features

🎤 **Stem Separation** — isolates lead vocals from instrumentals using the UVR Karaoke model (default) or Demucs, with adjustable guide vocal volume. The karaoke model preserves backing vocals in the instrumental for a more natural sound

📝 **Word-Level Lyrics** — automatic transcription with alignment, or fetched from [LRCLIB](https://lrclib.net) when available

🈯 **CJK Lyric Support** — Japanese, Chinese, and Korean songs get per-character forced alignment and romanized readings (Hepburn / pinyin / Revised Romanization) shown above each token

🗣️ **Pluggable ASR Engines** — choose Whisper (default, broad language coverage) or **Parakeet v3 (experimental)** for ~25 European languages, with NeMo on CUDA and ONNX Runtime everywhere else

🎼 **UltraStar Deluxe Songs (experimental)** — drop USDX song folders (`.txt` or `.usdx` plus sibling audio/vocals/instrumental/video) into your library; pitch and lyric data come from the file directly, no analyzer pass needed. See [docs/usdx](site/docs/src/usdx.md)

🎯 **Pitch Scoring** — real-time microphone input with pitch detection, star ratings, and per-song scoreboards

🎚️ **Key & Tempo Shifts** — adjust song key and tempo after analysis, with cached playback variants for quick retries

👤 **Profiles** — create and switch between player profiles; scores are tracked per profile

🎬 **Video Files** — drop video files (`.mp4`, `.mkv`, etc.) into your music folder; vocals are separated from the audio track and the original video plays as a synchronized background

🌌 **Audio-Reactive Backgrounds** — 10 GPU shaders that react to your microphone in real time (Plasma, Waves, Nebula, Starfield, Sonar, Voronoi, Vortex, Metaballs, Spectrum, Oscilloscope), Pixabay video loops in 5 flavors (Nature, Underwater, Space, City, Countryside), plus source-video playback for video files

🧭 **Sidebar + Library Filters** — quick filters, metadata cleanup buckets, artist/album groups, and an **Analyze All** action for bulk analysis

🎙️ **Mic Monitoring** — optionally route your live mic into playback for low-latency practice and monitoring, with an adjustable monitor gain (0–200%) in Settings

🎮 **Gamepad Support** — full navigation and control via gamepad (D-pad, sticks, face buttons)

📺 **Adaptive UI Scaling** — scales to any resolution including 4K TVs

📦 **Self-Contained** — ffmpeg, uv, Python, PyTorch, and ML packages are downloaded automatically during setup. Video backgrounds are pre-downloaded so the first session is ready to go

⬆️ **In-App Updates** — on macOS and Windows, auto-checks for new releases at launch, badges the sidebar avatar when one is available, and downloads and installs signed updates with one click. Linux is manual: the **Update** entry opens GitHub Releases for you to grab the new build

## Quick start

Download the latest release for your platform from the [Releases](../../releases) page and run it. On first launch, Nightingale shows setup steps, lets you pick a data folder, then installs the Python environment and ML models automatically.

## Updates

On macOS and Windows, Nightingale checks for new releases once at launch. When one is available, the sidebar avatar grows a small green dot and the **Update** entry in the dropdown menu opens a dialog with the release notes. Click **Install & Restart** and the app downloads the signed bundle, installs it, and relaunches. On Windows the installer runs in `passive` mode — a small progress window flashes and the app comes back automatically once the install finishes.

### Linux

Auto-update is **not supported on Linux** — the app ships without the updater plugin. The **Update** entry still appears in the sidebar menu, but it just opens a dialog explaining this with a one-click button to the [Releases](../../releases) page so you can grab the new `.deb` or `.rpm` and install it the usual way for your distro.

### macOS

macOS quarantines files downloaded from the internet. Since Nightingale isn't signed with an Apple Developer ID, Gatekeeper will block it with a message like _"app is damaged and can't be opened"_. To fix this, remove the quarantine attribute after moving the Nightingale.app to Applications:

```bash
xattr -cr /Applications/Nightingale.app
```

### Supported formats

Audio: `.mp3`, `.flac`, `.ogg`, `.wav`, `.m4a`, `.aac`, `.wma`. Video: `.mp4`, `.mkv`, `.avi`, `.webm`, `.mov`, `.m4v`. UltraStar: `.usdx`, plus `.txt` files whose contents look like USDX.

## Controls

### Navigation

| Action           | Keyboard       | Gamepad            |
| ---------------- | -------------- | ------------------ |
| Move             | Arrow keys     | D-pad / Left stick |
| Confirm / Select | Enter          | A (South)          |
| Back / Cancel    | Escape         | B (East) / Start   |
| Switch panel     | Tab            | —                  |
| Search songs     | Type to filter | —                  |

### Playback

| Action                  | Keyboard          | Gamepad   |
| ----------------------- | ----------------- | --------- |
| Pause / Resume          | Space             | Start     |
| Exit to menu            | Escape            | B (East)  |
| Toggle guide vocals     | G                 | —         |
| Guide volume up/down    | + / -             | —         |
| Cycle background theme  | T                 | —         |
| Cycle video flavor      | F                 | —         |
| Toggle microphone       | M                 | —         |
| Next microphone         | N                 | —         |
| Toggle mic monitoring   | R                 | —         |
| Toggle fullscreen       | F11               | —         |
| Skip Intro / Skip Outro | On-screen buttons | A (South) |

## How it works

```mermaid
flowchart TD
    A["Audio or video file"] --> B["UVR Karaoke / Demucs"]
    A2["USDX bundle (.txt / .usdx)"] --> E["Tauri App (Rust + React)"]
    B -->|"vocals + instrumental"| C["LRCLIB"]
    C -->|"synced lyrics if available"| D["WhisperX or Parakeet v3 (exp.)"]
    D -->|"word-level alignment, CJK reading"| E
    E --> F["Plays instrumental + synced lyrics with pitch scoring, key/tempo, mic monitoring, audio-reactive backgrounds"]
```

The analyzer runs as a persistent local process: Nightingale starts it once and talks to it over a token-authenticated loopback TCP socket using newline-delimited JSON, so per-song startup overhead (model load, CUDA init) is paid only once.

Analysis results are cached using blake3 file hashes. Re-analysis only happens if the source file changes, the user triggers it manually, or you choose to shift key/tempo and create playback variants. USDX songs skip stem separation entirely when `#VOCALS` and `#INSTRUMENTAL` are provided.

## Hardware

The Python analyzer uses PyTorch and auto-detects the best backend:

| Backend | Device        | Notes                                       |
| ------- | ------------- | ------------------------------------------- |
| CUDA    | NVIDIA GPU    | Fastest                                     |
| MPS     | Apple Silicon | macOS; WhisperX alignment falls back to CPU |
| CPU     | Any           | Slowest but always works                    |

The UVR Karaoke model uses ONNX Runtime and enables CUDA acceleration automatically on NVIDIA GPUs, or CoreML on Apple Silicon.

A song typically takes 2–5 minutes on GPU, 10–20 minutes on CPU.

## Data storage

During setup, you can choose where Nightingale stores data (default: `~/.nightingale`). Most runtime data is stored in that selected data folder, while `config.json` and `nightingale.log` remain in `~/.nightingale`.

Typical selected data folder layout:

```
<selected-data-folder>/
├── cache/               # Stems, transcripts, lyrics, shifted variants, covers, playable videos
├── songs.db             # SQLite song library and analysis metadata
├── profiles.json        # Player profiles and scores
├── videos/              # Cached Pixabay video backgrounds
├── sounds/              # Sound effects (celebration)
├── vendor/
│   ├── ffmpeg           # Downloaded ffmpeg binary
│   ├── uv               # Downloaded uv binary
│   ├── python/          # Python 3.10 installed via uv
│   ├── venv/            # Virtual environment with ML packages
│   ├── analyzer/        # Extracted analyzer Python scripts
│   └── .ready           # Marker indicating setup is complete
└── models/
    ├── torch/           # Demucs model cache
    ├── huggingface/     # WhisperX model cache
    └── audio_separator/ # UVR Karaoke model cache
```

`~/.nightingale/config.json` stores app settings, including the selected data folder path.

### Video backgrounds

Pixabay video backgrounds use the [Pixabay API](https://pixabay.com/api/docs/). The API key is embedded in release builds. For development, create a `.env` file at the project root:

```
PIXABAY_API_KEY=your_key_here
```

## Building from source

### Prerequisites

| Tool       | Version                                                                                                               |
| ---------- | --------------------------------------------------------------------------------------------------------------------- |
| Rust       | 1.85+ (workspace uses edition 2024)                                                                                   |
| Node.js    | 20+                                                                                                                   |
| pnpm       | latest                                                                                                                |
| Linux only | `libwebkit2gtk-4.1-dev`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libxdo-dev`, `libasound2-dev` |

### Development

```bash
git clone <repo-url> nightingale
cd nightingale
cargo desktop dev
```

### Release build

```bash
cargo desktop build
```

## Supported platforms

| Platform       | Target                      |
| -------------- | --------------------------- |
| Linux x86_64   | `x86_64-unknown-linux-gnu`  |
| Linux aarch64  | `aarch64-unknown-linux-gnu` |
| macOS ARM      | `aarch64-apple-darwin`      |
| macOS Intel    | `x86_64-apple-darwin`       |
| Windows x86_64 | `x86_64-pc-windows-msvc`    |

## Releasing

Releases are cut by [`.github/workflows/release.yml`](.github/workflows/release.yml) on any `v*` tag push. The workflow:

1. Verifies the tag matches the `version` in [`client/src-tauri/tauri.conf.json`](client/src-tauri/tauri.conf.json), [`client/src-tauri/Cargo.toml`](client/src-tauri/Cargo.toml), and [`client/package.json`](client/package.json).
2. Extracts the matching `## [<version>]` section from [`CHANGELOG.md`](CHANGELOG.md) as the release body.
3. Creates a draft release and, in parallel, builds and uploads:
   - Linux x86_64: `.deb`, `.rpm` (on `ubuntu-22.04`)
   - Linux aarch64: `.deb`, `.rpm` (on `ubuntu-24.04-arm`)
   - macOS ARM / Intel: `.dmg` + `.app.tar.gz` (+ `.sig`) for the in-app updater
   - Windows x86_64: `*-setup.exe` (NSIS, + `.sig`), `*_en-US.msi` (+ `.sig`)
   - `latest.json` covering `darwin-aarch64`, `darwin-x86_64`, and `windows-x86_64` — Linux is intentionally absent since the updater plugin isn't compiled in for Linux.
4. Leaves the release as a draft. Smoke-test the artifacts from the draft, then flip it to **Published** with the "Set as the latest release" checkbox in the GitHub Releases UI to make `https://github.com/rzru/nightingale/releases/latest/download/latest.json` (the URL hard-coded in [`tauri.conf.json`](client/src-tauri/tauri.conf.json)) resolve to it and start rolling out the in-app update.

Cutting a release:

```bash
# bump versions in client/src-tauri/tauri.conf.json, client/src-tauri/Cargo.toml, client/package.json
# add a `## [<version>] - YYYY-MM-DD` section to CHANGELOG.md
git tag v<version>
git push origin v<version>
```

Required repository secrets:

| Secret                                | Purpose                                                                                                                            |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `TAURI_SIGNING_PRIVATE_KEY`           | Minisign private key whose public counterpart is the `pubkey` in [`tauri.conf.json`](client/src-tauri/tauri.conf.json). Generate once with `pnpm tauri signer generate`. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`  | Password for the signing key. Omit the secret entirely if the key was generated passwordless — GitHub rejects empty-string secrets, and a missing one resolves to empty at workflow runtime, which is what `minisign` expects. |
| `PIXABAY_API_KEY`                     | Embedded at compile time so release builds can fetch video backgrounds.                                                            |

## License

GPL-3.0-or-later — see [LICENSE](LICENSE).
