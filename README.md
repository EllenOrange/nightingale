# Nightingale — AI Karaoke from Any Song

Turn any music folder into a karaoke machine. Nightingale scans your library, separates vocals from instrumentals using AI, transcribes lyrics with word-level timestamps, and plays it all back with synchronized highlighting and dynamic shader backgrounds.

## Prerequisites

### System

| Dependency | Version | Why |
|---|---|---|
| **Rust** | 1.85+ (edition 2024) | Builds the Bevy app |
| **Python** | 3.10+ | Runs the Demucs/WhisperX analyzer |
| **ffmpeg** | any recent | Required by both Demucs and WhisperX |

### Hardware

The Python analyzer uses PyTorch and will auto-detect the best available backend:

| Backend | Device | Notes |
|---|---|---|
| **CUDA** | NVIDIA GPU | Fastest. Needs `torch` built with CUDA support. |
| **MPS** | Apple Silicon (M1/M2/M3/M4) | Works on macOS. WhisperX falls back to CPU for alignment. |
| **CPU** | Any | Slowest but always works. |

A song typically takes 2–5 minutes to analyze on GPU, 10–20 minutes on CPU.

## Setup

### 1. Clone and build the Rust app

```bash
git clone <repo-url> nightingale
cd nightingale
cargo build --release
```

### 2. Set up the Python analyzer

```bash
cd analyzer
./setup.sh
```

This creates a virtualenv at `analyzer/.venv` and installs `demucs`, `whisperx`, `torch`, and `torchaudio`.

**NVIDIA GPU users**: if you need a specific CUDA version of PyTorch, install it manually before running `setup.sh`:

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install torch torchaudio --index-url https://download.pytorch.org/whl/cu121
pip install -r requirements.txt
```

### 3. Verify ffmpeg is installed

```bash
ffmpeg -version
```

If missing: `brew install ffmpeg` (macOS) or `sudo apt install ffmpeg` (Linux).

## Running

```bash
cargo run --release
```

On launch you'll see a folder picker. Select the root of your music library (it scans recursively for `.mp3`, `.flac`, `.ogg`, `.wav`, `.m4a`, `.aac`, `.wma` files).

After scanning, click any song to start analysis (first time only — results are cached). Once analysis finishes, karaoke playback begins automatically.

## Controls

| Key | Action |
|---|---|
| **Click a song** | Analyze (if needed) and play |
| **Type** | Search/filter songs by title or artist |
| **Backspace** | Delete search character |
| **ESC** (in menu) | Clear search |
| **ESC** (in player) | Return to menu |
| **G** | Toggle guide vocals on/off (30% volume) |
| **+** / **-** | Adjust guide vocal volume |
| **T** | Cycle background theme (Plasma / Aurora / Waves) |

## How it works

```
Music File (.mp3/.flac/...)
        │
        ▼
  ┌─────────────┐
  │   Demucs     │  ──▶  instrumental.wav + vocals.wav
  │ (htdemucs)   │
  └─────────────┘
        │ vocals.wav
        ▼
  ┌─────────────┐
  │  WhisperX    │  ──▶  transcript.json (word-level timestamps)
  │ (large-v3)   │
  └─────────────┘
        │
        ▼
  ┌─────────────┐
  │  Bevy App    │  ──▶  Plays instrumental + synced lyrics
  │  (Rust)      │       with dynamic shader backgrounds
  └─────────────┘
```

Analysis results are cached at `~/.nightingale/cache/` using blake3 file hashes. Re-analyzing only happens if the source file changes.

## Project structure

```
nightingale/
├── analyzer/
│   ├── analyze.py          # Demucs + WhisperX pipeline
│   ├── requirements.txt
│   └── setup.sh            # Virtualenv bootstrap
├── assets/
│   └── shaders/            # WGSL fragment shaders for backgrounds
│       ├── plasma.wgsl
│       ├── aurora.wgsl
│       └── waves.wgsl
├── src/
│   ├── main.rs             # Bevy app entry, folder picker
│   ├── states.rs           # AppState enum
│   ├── scanner/            # Folder scan + metadata (lofty)
│   ├── analyzer/           # Python subprocess orchestrator + cache
│   ├── menu/               # Song list UI with search
│   └── player/             # Audio, lyrics sync, background shaders
├── Cargo.toml
└── README.md
```

## License

MIT
