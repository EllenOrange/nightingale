# How It Works

Nightingale's pipeline transforms any audio or video file into a karaoke experience through several stages.

## Pipeline Overview

<pre class="mermaid">
flowchart TD
    A["🎵 Audio or video file"] --> B["UVR Karaoke / Demucs"]
    A2["🎼 USDX bundle (.txt / .usdx)"] --> E["Tauri App (Rust + React)"]
    B --> |"vocals + instrumental"| C["LRCLIB"]
    C --> |"synced lyrics if available"| D["WhisperX (large-v3) or Parakeet v3 (exp.)"]
    D --> |"word-level alignment, CJK readings"| E
    E --> F["🎤 Plays instrumental + synced lyrics\nwith pitch scoring, key/tempo controls,\nmic monitoring, and audio-reactive backgrounds"]
</pre>

USDX bundles bypass stem separation and transcription entirely — the `.txt` is parsed into a transcript JSON shaped exactly like the analyzer cache, so playback reuses the existing pipeline. See [UltraStar Deluxe](./usdx.md).

## Analyzer Server

The analyzer is a long-lived Python process that Nightingale spawns once on startup and talks to over a token-authenticated loopback TCP socket using newline-delimited JSON (NDJSON). Per-song startup costs (model load, CUDA init, Python imports) are paid once at boot, after which `analyze` requests stream `progress` events and complete with `done` or `error` messages. This makes back-to-back analyses noticeably faster than the previous per-song subprocess model.

## Caching

Analysis results are cached in your configured data folder (`cache/`) using blake3 file hashes. Re-analysis only happens if the source file changes, if you trigger it manually, or when creating shifted key/tempo variants.

## Hardware Acceleration

The Python analyzer uses PyTorch and auto-detects the best backend:

| Backend | Device | Notes |
|---|---|---|
| CUDA | NVIDIA GPU | Fastest |
| MPS | Apple Silicon | macOS; WhisperX alignment falls back to CPU |
| CPU | Any | Slowest but always works |

<br />

The UVR Karaoke model uses ONNX Runtime and enables CUDA acceleration automatically on NVIDIA GPUs, or CoreML on Apple Silicon.

A song typically takes 2–5 minutes on GPU, 10–20 minutes on CPU.
