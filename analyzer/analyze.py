#!/usr/bin/env python3
"""
Nightingale Song Analyzer
Separates vocals/instrumentals with Demucs and transcribes lyrics with WhisperX.

Usage:
    python analyze.py <audio_path> <output_dir> [--hash <file_hash>]

Outputs (in output_dir):
    {hash}_instrumental.ogg
    {hash}_vocals.ogg
    {hash}_transcript.json

Progress protocol (parsed by Rust app):
    [nightingale:PROGRESS:<percent>] <message>
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from whisper_compat import progress, detect_device
from stems import separate_stems, separate_stems_uvr
from align import align_lyrics
from transcribe import transcribe_vocals


def compute_hash(path: str) -> str:
    h = hashlib.blake2b(digest_size=16)
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()


def _ffmpeg_bin():
    return os.environ.get("FFMPEG_PATH", "ffmpeg")


def _convert_to_ogg(src_wav, dest_ogg):
    subprocess.run(
        [_ffmpeg_bin(), "-y", "-i", src_wav, "-c:a", "libvorbis", "-q:a", "6", "-v", "error", dest_ogg],
        check=True,
    )
    if os.path.isfile(dest_ogg):
        os.remove(src_wav)


def main():
    parser = argparse.ArgumentParser(description="Nightingale Song Analyzer")
    parser.add_argument("audio_path", help="Path to the audio file")
    parser.add_argument("output_dir", help="Directory to write output files")
    parser.add_argument("--hash", dest="file_hash", help="Pre-computed file hash")
    parser.add_argument("--model", default="large-v3-turbo", help="Whisper model name")
    parser.add_argument("--beam-size", type=int, default=5, help="Beam size for decoding")
    parser.add_argument("--batch-size", type=int, default=16, help="Batch size for transcription")
    parser.add_argument("--separator", default="karaoke", choices=["karaoke", "demucs"],
                        help="Stem separation method: karaoke (UVR, cleaner) or demucs (faster)")
    parser.add_argument("--lyrics", help="Path to pre-fetched lyrics JSON (align-only mode)")
    args = parser.parse_args()

    audio_path = os.path.abspath(args.audio_path)
    output_dir = os.path.abspath(args.output_dir)

    if not os.path.isfile(audio_path):
        print(f"[nightingale] ERROR: File not found: {audio_path}", file=sys.stderr)
        sys.exit(1)

    os.makedirs(output_dir, exist_ok=True)

    file_hash = args.file_hash or compute_hash(audio_path)
    progress(0, "Starting analysis...")

    transcript_path = os.path.join(output_dir, f"{file_hash}_transcript.json")
    if os.path.isfile(transcript_path):
        progress(100, "Already analyzed, skipping")
        sys.exit(0)

    device = detect_device()
    progress(2, f"Using device: {device}")

    # --- Stem separation ---
    final_vocals_ogg = os.path.join(output_dir, f"{file_hash}_vocals.ogg")
    final_instrumental_ogg = os.path.join(output_dir, f"{file_hash}_instrumental.ogg")
    final_vocals_wav = os.path.join(output_dir, f"{file_hash}_vocals.wav")
    final_instrumental_wav = os.path.join(output_dir, f"{file_hash}_instrumental.wav")

    if os.path.isfile(final_vocals_ogg) and os.path.isfile(final_instrumental_ogg):
        progress(50, "Stems already cached, skipping separation")
        vocals_path = final_vocals_ogg
    elif os.path.isfile(final_vocals_wav) and os.path.isfile(final_instrumental_wav):
        progress(50, "Converting legacy WAV stems to OGG...")
        _convert_to_ogg(final_vocals_wav, final_vocals_ogg)
        _convert_to_ogg(final_instrumental_wav, final_instrumental_ogg)
        vocals_path = final_vocals_ogg
    else:
        with tempfile.TemporaryDirectory(prefix="nightingale_") as work_dir:
            if args.separator == "karaoke":
                torch_home = os.environ.get("TORCH_HOME", "")
                models_base = os.path.dirname(torch_home) if torch_home else output_dir
                uvr_models_dir = os.path.join(models_base, "audio_separator")
                os.makedirs(uvr_models_dir, exist_ok=True)
                vocals_path, instrumental_path = separate_stems_uvr(audio_path, work_dir, uvr_models_dir)
            else:
                vocals_path, instrumental_path = separate_stems(audio_path, work_dir, device)
            progress(92, "Saving stems to cache...")
            _convert_to_ogg(vocals_path, final_vocals_ogg)
            _convert_to_ogg(instrumental_path, final_instrumental_ogg)
        vocals_path = final_vocals_ogg

    # --- Lyrics alignment or transcription ---
    if args.lyrics and os.path.isfile(args.lyrics):
        print(f"[nightingale:LOG] Using pre-fetched lyrics: {args.lyrics}", flush=True)
        transcript = align_lyrics(
            args.lyrics, vocals_path, device,
            model_name=args.model,
        )
    else:
        transcript = transcribe_vocals(
            vocals_path, audio_path, device,
            model_name=args.model,
            beam_size=args.beam_size,
            batch_size=args.batch_size,
        )

    # --- Write output ---
    progress(95, "Writing transcript...")
    with open(transcript_path, "w", encoding="utf-8") as f:
        json.dump(transcript, f, ensure_ascii=False, indent=2)

    progress(100, "DONE")


if __name__ == "__main__":
    main()
