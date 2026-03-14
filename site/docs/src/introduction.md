# Nightingale

**Karaoke from any song in your music library, powered by neural networks.**

Nightingale scans your music folder, separates lead vocals from instrumentals using the UVR Karaoke model (or Demucs), transcribes lyrics with word-level timestamps via WhisperX, and plays it all back with synchronized highlighting, pitch scoring, profiles, and dynamic backgrounds.

Ships as a single binary. No manual installation of Python, ffmpeg, or ML models required — everything is downloaded and bootstrapped automatically on first launch.

<!-- TODO: screenshot of playback UI with lyrics highlighted over a background -->
![Nightingale playback](images/playback.png)

## Key Features

- **Stem Separation** — isolates lead vocals from instrumentals
- **Word-Level Lyrics** — automatic transcription with alignment
- **Pitch Scoring** — real-time microphone input with star ratings
- **Profiles** — per-player score tracking
- **Video Files** — use video files with synchronized background playback
- **7 Background Themes** — GPU shaders, Pixabay videos, source video
- **Gamepad Support** — full navigation via gamepad
- **Self-Contained** — zero manual dependency setup

## Supported Platforms

| Platform | Target |
|---|---|
| Linux x86_64 | `x86_64-unknown-linux-gnu` |
| Linux aarch64 | `aarch64-unknown-linux-gnu` |
| macOS ARM | `aarch64-apple-darwin` |
| macOS Intel | `x86_64-apple-darwin` |
| Windows x86_64 | `x86_64-pc-windows-msvc` |
