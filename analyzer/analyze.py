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
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from whisper_compat import progress, detect_device
from pipeline import run_pipeline


def compute_hash(path: str) -> str:
    h = hashlib.blake2b(digest_size=16)
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    parser = argparse.ArgumentParser(description="Nightingale Song Analyzer")
    parser.add_argument("audio_path", help="Path to the audio file")
    parser.add_argument("output_dir", help="Directory to write output files")
    parser.add_argument("--hash", dest="file_hash", help="Pre-computed file hash")
    parser.add_argument("--model", default="large-v3", help="Whisper model name")
    parser.add_argument("--beam-size", type=int, default=5, help="Beam size for decoding")
    parser.add_argument("--batch-size", type=int, default=16, help="Batch size for transcription")
    parser.add_argument("--separator", default="karaoke", choices=["karaoke", "demucs"],
                        help="Stem separation method: karaoke (UVR, cleaner) or demucs (faster)")
    parser.add_argument("--lyrics", help="Path to pre-fetched lyrics JSON (align-only mode)")
    parser.add_argument("--language", default=None, help="Override automatic language detection")
    args = parser.parse_args()

    audio_path = os.path.abspath(args.audio_path)
    output_dir = os.path.abspath(args.output_dir)

    if not os.path.isfile(audio_path):
        print(f"[nightingale] ERROR: File not found: {audio_path}", file=sys.stderr)
        sys.exit(1)

    file_hash = args.file_hash or compute_hash(audio_path)
    progress(0, "Starting analysis...")

    device = detect_device()

    run_pipeline(
        audio_path, output_dir, file_hash, device,
        model_name=args.model,
        beam_size=args.beam_size,
        batch_size=args.batch_size,
        separator=args.separator,
        lyrics_path=args.lyrics,
        language_override=args.language,
    )

    progress(100, "DONE")


if __name__ == "__main__":
    main()
