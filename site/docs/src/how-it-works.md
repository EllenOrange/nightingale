# How It Works

Nightingale's pipeline transforms any audio or video file into a karaoke experience through several stages.

## Pipeline Overview

<pre class="mermaid">
flowchart TD
    A["🎵 Audio or video file"] --> B["UVR Karaoke / Demucs"]
    B --> |"vocals.ogg + instrumental.ogg"| C["LRCLIB"]
    C --> |"Fetches synced lyrics if available"| D["WhisperX (large-v3)"]
    D --> |"Transcription + word-level alignment"| E["Bevy App (Rust)"]
    E --> F["🎤 Plays instrumental + synced lyrics\nwith pitch scoring & backgrounds"]
</pre>

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
