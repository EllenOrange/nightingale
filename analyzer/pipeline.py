"""Shared analysis pipeline used by both server.py and analyze.py."""

import json
import os
import subprocess
import tempfile

from whisper_compat import progress
from stems import separate_stems, separate_stems_uvr
from transcribe import transcribe_vocals
from align import align_lyrics


def ffmpeg_bin():
    return os.environ.get("FFMPEG_PATH", "ffmpeg")


def convert_to_ogg(src_wav, dest_ogg):
    subprocess.run(
        [ffmpeg_bin(), "-y", "-i", src_wav, "-c:a", "libvorbis", "-q:a", "6", "-v", "error", dest_ogg],
        check=True,
    )
    if os.path.isfile(dest_ogg):
        os.remove(src_wav)


def separate_and_cache(audio_path, output_dir, file_hash, separator, device, free_gpu_fn=None):
    """Run stem separation or reuse cached stems. Returns the vocals path."""
    final_vocals_ogg = os.path.join(output_dir, f"{file_hash}_vocals.ogg")
    final_instrumental_ogg = os.path.join(output_dir, f"{file_hash}_instrumental.ogg")
    final_vocals_wav = os.path.join(output_dir, f"{file_hash}_vocals.wav")
    final_instrumental_wav = os.path.join(output_dir, f"{file_hash}_instrumental.wav")

    if os.path.isfile(final_vocals_ogg) and os.path.isfile(final_instrumental_ogg):
        progress(50, "Stems already cached, skipping separation")
        return final_vocals_ogg

    if os.path.isfile(final_vocals_wav) and os.path.isfile(final_instrumental_wav):
        progress(50, "Converting legacy WAV stems to OGG...")
        convert_to_ogg(final_vocals_wav, final_vocals_ogg)
        convert_to_ogg(final_instrumental_wav, final_instrumental_ogg)
        return final_vocals_ogg

    with tempfile.TemporaryDirectory(prefix="nightingale_") as work_dir:
        if separator == "karaoke":
            torch_home = os.environ.get("TORCH_HOME", "")
            models_base = os.path.dirname(torch_home) if torch_home else output_dir
            uvr_models_dir = os.path.join(models_base, "audio_separator")
            os.makedirs(uvr_models_dir, exist_ok=True)
            vp, ip = separate_stems_uvr(audio_path, work_dir, uvr_models_dir)
        else:
            vp, ip = separate_stems(audio_path, work_dir, device)
        progress(51, "Saving stems to cache...")
        convert_to_ogg(vp, final_vocals_ogg)
        convert_to_ogg(ip, final_instrumental_ogg)

    if free_gpu_fn:
        free_gpu_fn()

    return final_vocals_ogg


def transcribe_or_align(
    vocals_path, audio_path, device, *,
    model_name, beam_size=5, batch_size=16,
    lyrics_path=None, language_override=None,
    whisper_model=None, pre_align_cleanup=None,
):
    """Choose between lyrics alignment and full transcription."""
    if lyrics_path and os.path.isfile(lyrics_path):
        print(f"[nightingale:LOG] Using pre-fetched lyrics: {lyrics_path}", flush=True)
        return align_lyrics(
            lyrics_path, vocals_path, device,
            model_name=model_name,
            language_override=language_override,
            whisper_model=whisper_model,
            pre_align_cleanup=pre_align_cleanup,
        )

    return transcribe_vocals(
        vocals_path, audio_path, device,
        model_name=model_name,
        beam_size=beam_size,
        batch_size=batch_size,
        language_override=language_override,
        whisper_model=whisper_model,
        pre_align_cleanup=pre_align_cleanup,
    )


def run_pipeline(
    audio_path, output_dir, file_hash, device, *,
    model_name="large-v3", beam_size=5, batch_size=16,
    separator="karaoke", lyrics_path=None, language_override=None,
    whisper_model=None, pre_align_cleanup=None, free_gpu_fn=None,
):
    """Full analysis pipeline: stem separation -> transcription -> save."""
    os.makedirs(output_dir, exist_ok=True)

    transcript_path = os.path.join(output_dir, f"{file_hash}_transcript.json")
    if os.path.isfile(transcript_path):
        progress(100, "Already analyzed, skipping")
        return

    progress(2, f"Using device: {device}")

    vocals_path = separate_and_cache(
        audio_path, output_dir, file_hash, separator, device,
        free_gpu_fn=free_gpu_fn,
    )

    if callable(whisper_model):
        whisper_model = whisper_model()

    transcript = transcribe_or_align(
        vocals_path, audio_path, device,
        model_name=model_name,
        beam_size=beam_size,
        batch_size=batch_size,
        lyrics_path=lyrics_path,
        language_override=language_override,
        whisper_model=whisper_model,
        pre_align_cleanup=pre_align_cleanup,
    )

    progress(95, "Writing transcript...")
    with open(transcript_path, "w", encoding="utf-8") as f:
        json.dump(transcript, f, ensure_ascii=False, indent=2)
