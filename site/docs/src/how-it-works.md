# How It Works

Nightingale's pipeline transforms any audio or video file into a karaoke experience through several stages.

## Pipeline Overview

```
Audio or video file
        │
        ▼
  ┌─────────────────┐
  │  UVR Karaoke /   │  ──▶  vocals.ogg + instrumental.ogg
  │  Demucs          │       (extracts audio track from videos)
  └─────────────────┘
        │
        ▼
  ┌─────────────────┐
  │  LRCLIB          │  ──▶  Fetches synced lyrics if available
  └─────────────────┘
        │
        ▼
  ┌─────────────────┐
  │  WhisperX        │  ──▶  Transcription + word-level alignment
  │  (large-v3)      │
  └─────────────────┘
        │
        ▼
  ┌─────────────────┐
  │  Bevy App        │  ──▶  Plays instrumental + synced lyrics
  │  (Rust)          │       with pitch scoring & backgrounds
  └─────────────────┘
```

## Caching

Analysis results are cached at `~/.nightingale/cache/` using blake3 file hashes. Re-analysis only happens if the source file changes or is manually triggered from the UI.

## Hardware Acceleration

The Python analyzer uses PyTorch and auto-detects the best backend:

| Backend | Device | Notes |
|---|---|---|
| CUDA | NVIDIA GPU | Fastest |
| MPS | Apple Silicon | macOS; WhisperX alignment falls back to CPU |
| CPU | Any | Slowest but always works |

The UVR Karaoke model uses ONNX Runtime and enables CUDA acceleration automatically on NVIDIA GPUs, or CoreML on Apple Silicon.

A song typically takes 2–5 minutes on GPU, 10–20 minutes on CPU.
