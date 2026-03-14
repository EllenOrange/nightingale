# Troubleshooting

## First Launch Takes Too Long

The initial setup downloads Python, PyTorch, ML models, and video backgrounds. On a slow connection this can take 10–20 minutes. Subsequent launches skip setup entirely.

## Analysis Fails

If song analysis fails:

1. Check the error message in the UI for details
2. Ensure you have enough disk space (~5 GB for models and cache)
3. Try re-running with `--setup` to reset the vendor environment
4. GPU memory errors may occur with very long songs — CPU fallback will be used automatically

## No Sound

- Verify your audio output device is correctly configured
- Check that the audio file format is supported (`.mp3`, `.flac`, `.ogg`, `.wav`, `.m4a`, `.aac`, `.wma`)
- Try a different audio file to rule out file-specific issues

## Microphone Not Detected

- Press `N` to cycle through available microphones
- Ensure microphone permissions are granted to the application
- On macOS, check System Settings > Privacy & Security > Microphone

## GPU Acceleration Not Working

The analyzer auto-detects the best backend:

- **NVIDIA GPU**: Requires CUDA-compatible drivers
- **Apple Silicon**: Uses MPS backend (some operations fall back to CPU)
- **CPU**: Always works as a fallback

Check the setup progress screen for which backend was detected.

## Reset Everything

To completely reset Nightingale, delete the data directory:

```bash
rm -rf ~/.nightingale
```

The next launch will re-run setup from scratch.
