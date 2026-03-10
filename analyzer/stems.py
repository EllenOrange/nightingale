"""Demucs stem separation: vocals + instrumental."""

import os
import subprocess

import torch

from whisper_compat import progress


def _ensure_wav(audio_path: str, work_dir: str) -> str:
    """Convert input audio to WAV if needed so torchaudio can load it."""
    if audio_path.lower().endswith(".wav"):
        return audio_path
    wav_path = os.path.join(work_dir, "input.wav")
    ffmpeg = os.environ.get("FFMPEG_PATH", "ffmpeg")
    subprocess.run(
        [ffmpeg, "-y", "-i", audio_path, "-ar", "44100", "-ac", "2", "-v", "error", wav_path],
        check=True,
    )
    return wav_path


def separate_stems(audio_path: str, work_dir: str, device: str) -> tuple[str, str]:
    """Run Demucs to separate vocals and instrumental stems.

    Returns (vocals_path, instrumental_path).
    """
    from demucs.apply import apply_model
    from demucs.audio import save_audio
    from demucs.pretrained import get_model
    import torchaudio

    progress(5, "Loading Demucs model...")
    model = get_model("htdemucs")
    actual_device = torch.device(device if device != "mps" else "cpu")
    model.to(actual_device)

    progress(10, "Loading audio file...")
    load_path = _ensure_wav(audio_path, work_dir)
    wav, sr = torchaudio.load(load_path)
    wav = wav.to(actual_device)

    ref = wav.mean(0)
    wav_centered = wav - ref.mean()
    wav_scaled = wav_centered / ref.abs().max().clamp(min=1e-8)

    progress(15, "Separating vocals from instrumentals...")
    sources = apply_model(model, wav_scaled[None], device=actual_device, shifts=1, overlap=0.25)[0]

    source_names = model.sources
    vocals_idx = source_names.index("vocals")

    vocals = sources[vocals_idx] * ref.abs().max() + ref.mean()
    instrumental = wav.to(actual_device) - vocals

    progress(45, "Saving separated stems...")

    vocals_path = os.path.join(work_dir, "vocals.wav")
    instrumental_path = os.path.join(work_dir, "instrumental.wav")

    save_audio(vocals.cpu(), vocals_path, sr)
    save_audio(instrumental.cpu(), instrumental_path, sr)

    progress(50, "Stem separation complete")
    return vocals_path, instrumental_path
