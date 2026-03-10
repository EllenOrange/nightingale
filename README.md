<p align="center">
  <img src="assets/images/logo.png" alt="Nightingale" width="400">
</p>

<p align="center">
  Karaoke from any song in your music library, powered by neural networks.
</p>

---

Nightingale scans your music folder, separates vocals from instrumentals with [Demucs](https://github.com/facebookresearch/demucs), transcribes lyrics with word-level timestamps via [WhisperX](https://github.com/m-bain/whisperX), and plays it all back with synchronized highlighting, pitch scoring, profiles, and dynamic backgrounds.

Ships as a single binary. No manual installation of Python, ffmpeg, or ML models required — everything is bootstrapped automatically on first launch.

## Features

🎤 **Stem Separation** — isolates vocals and instrumentals from any audio file using Demucs, with adjustable guide vocal volume

📝 **Word-Level Lyrics** — automatic transcription with alignment, or fetched from [LRCLIB](https://lrclib.net) when available

🎯 **Pitch Scoring** — real-time microphone input with pitch detection, star ratings, and per-song scoreboards

👤 **Profiles** — create and switch between player profiles; scores are tracked per profile

🌌 **6 Background Themes** — 5 GPU shader backgrounds (Plasma, Aurora, Waves, Nebula, Starfield) plus Pixabay video backgrounds with 5 flavors (Nature, Underwater, Space, City, Countryside)

🎮 **Gamepad Support** — full navigation and control via gamepad (D-pad, sticks, face buttons)

📺 **Adaptive UI Scaling** — scales to any resolution including 4K TVs

📦 **Self-Contained** — ffmpeg and uv are bundled in the binary; Python, PyTorch, and ML packages are installed to `~/.nightingale/vendor/` on first run

## Quick start

Download the latest release for your platform from the [Releases](../../releases) page and run it. On first launch, Nightingale will set up its Python environment and download ML models — this takes a few minutes and shows a progress screen.

Supported formats: `.mp3`, `.flac`, `.ogg`, `.wav`, `.m4a`, `.aac`, `.wma`.

## Controls

### Navigation

| Action | Keyboard | Gamepad |
|---|---|---|
| Move | Arrow keys | D-pad / Left stick |
| Confirm / Select | Enter | A (South) |
| Back / Cancel | Escape | B (East) / Start |
| Switch panel | Tab | — |
| Search songs | Type to filter | — |

### Playback

| Action | Key |
|---|---|
| Toggle guide vocals | G |
| Guide volume up/down | + / - |
| Cycle background theme | T |
| Cycle video flavor | F |
| Toggle microphone | M |
| Next microphone | N |
| Toggle fullscreen | F11 |

## How it works

```
Music file (.mp3/.flac/...)
        │
        ▼
  ┌─────────────┐
  │   Demucs     │  ──▶  vocals.ogg + instrumental.ogg
  │ (htdemucs)   │
  └─────────────┘
        │
        ▼
  ┌─────────────┐
  │  LRCLIB      │  ──▶  Fetches synced lyrics if available
  └─────────────┘
        │
        ▼
  ┌─────────────┐
  │  WhisperX    │  ──▶  Transcription + word-level alignment
  │ (large-v3)   │
  └─────────────┘
        │
        ▼
  ┌─────────────┐
  │  Bevy App    │  ──▶  Plays instrumental + synced lyrics
  │  (Rust)      │       with pitch scoring & backgrounds
  └─────────────┘
```

Analysis results are cached at `~/.nightingale/cache/` using blake3 file hashes. Re-analysis only happens if the source file changes or is manually triggered.

## Hardware

The Python analyzer uses PyTorch and auto-detects the best backend:

| Backend | Device | Notes |
|---|---|---|
| CUDA | NVIDIA GPU | Fastest |
| MPS | Apple Silicon | macOS; WhisperX alignment falls back to CPU |
| CPU | Any | Slowest but always works |

A song typically takes 2–5 minutes on GPU, 10–20 minutes on CPU.

## Data storage

Everything lives under `~/.nightingale/`:

```
~/.nightingale/
├── cache/              # Stems, transcripts, lyrics per song
├── config.json         # App settings
├── profiles.json       # Player profiles and scores
├── videos/             # Cached Pixabay video backgrounds
├── sounds/             # Sound effects (celebration)
├── vendor/
│   ├── ffmpeg          # Bundled ffmpeg binary
│   ├── uv              # Bundled uv binary
│   ├── python/         # Python 3.11 installed via uv
│   ├── venv/           # Virtual environment with ML packages
│   ├── analyzer/       # Embedded analyzer Python scripts
│   └── .ready          # Marker indicating setup is complete
└── models/
    ├── torch/          # Demucs model cache
    └── huggingface/    # WhisperX model cache
```

### Video backgrounds

Video backgrounds use the [Pixabay API](https://pixabay.com/api/docs/). The API key is embedded in release builds. For development, create a `.env` file at the project root:

```
PIXABAY_API_KEY=your_key_here
```

In CI, the key is provided via the `PIXABAY_API_KEY` secret. The local release script (`make-release.sh`) sources `.env` automatically.

## Building from source

### Prerequisites

| Tool | Version |
|---|---|
| Rust | 1.85+ (edition 2024) |
| Linux only | `libasound2-dev`, `libudev-dev`, `libwayland-dev`, `libxkbcommon-dev` |

### Development build

```bash
git clone <repo-url> nightingale
cd nightingale
scripts/fetch-vendor-bin.sh   # Downloads ffmpeg + uv for your platform into vendor-bin/
cargo build --release
```

The `build.rs` script creates placeholder files in `vendor-bin/` if the real binaries are missing, so `cargo build` always succeeds — but the resulting binary won't be able to bootstrap without real binaries embedded.

### Local release

```bash
scripts/make-release.sh
```

Fetches vendor binaries (if needed), builds the release binary, and packages it into `nightingale-<target>.tar.gz`.

### CLI flags

| Flag | Description |
|---|---|
| `--setup` | Force re-run of the first-launch bootstrap |

## CI/CD

Pushing a tag matching `v*` triggers the [release workflow](.github/workflows/release.yml), which builds for:

| Platform | Target |
|---|---|
| Linux x86_64 | `x86_64-unknown-linux-gnu` |
| macOS ARM | `aarch64-apple-darwin` |
| macOS Intel | `x86_64-apple-darwin` |
| Windows x86_64 | `x86_64-pc-windows-msvc` |

Each build fetches the correct platform-specific ffmpeg and uv binaries, embeds them via `include_bytes!`, and uploads the packaged archive to a GitHub Release.

## License

MIT
