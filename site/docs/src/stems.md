# Stem Separation

Nightingale separates lead vocals from instrumentals so you can sing along to the backing track.

## Models

### UVR Karaoke (Default)

The UVR (Ultimate Vocal Remover) Karaoke model is optimized specifically for karaoke use. It preserves backing vocals in the instrumental track, giving a more natural karaoke experience. Uses ONNX Runtime for inference, with automatic CUDA (NVIDIA) or CoreML (Apple Silicon) acceleration.

### Demucs

[Demucs](https://github.com/facebookresearch/demucs) by Facebook Research provides an alternative separation model. You can switch between models in the settings.

## Video Files

When processing video files (`.mp4`, `.mkv`, etc.), Nightingale first extracts the audio track using ffmpeg, then runs stem separation on the extracted audio. During playback, Nightingale uses the source video when available and falls back to a playable converted video when needed.

## Guide Vocals

After separation, you can control how much of the lead vocals bleed through the instrumental:

- **Toggle**: Press `G` to turn guide vocals on/off
- **Volume**: Press `+` / `-` to adjust the guide vocal level

This is useful for learning new songs or for singers who want a reference pitch.

## UltraStar Bundles Skip Stem Separation

UltraStar Deluxe songs that ship with `#VOCALS` and `#INSTRUMENTAL` files are used directly during playback — no stem separation is run. If only `#AUDIO` is provided, Nightingale falls back to running the configured separator on that file. See [UltraStar Deluxe](./usdx.md) for the supported tags.
