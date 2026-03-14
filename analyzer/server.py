#!/usr/bin/env python3
"""Persistent analyzer server for Nightingale.

Reads JSON commands from stdin, processes songs, writes progress to stdout.
Whisper model is cached between songs for faster batch analysis.

Protocol:
  Stdin  (JSON per line): {"command": "analyze", ...} or {"command": "quit"}
  Stdout (line per msg):  [nightingale:PROGRESS:N] msg
                          [nightingale:DONE]
                          [nightingale:ERROR] msg
                          [nightingale:OOM] msg
"""

import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from whisper_compat import progress, detect_device, compute_type_for, is_oom, free_gpu
from pipeline import run_pipeline

_whisper_model = None
_whisper_key = None  # (model_name, device, compute_type)


def _clear_models():
    global _whisper_model, _whisper_key
    if _whisper_model is not None:
        del _whisper_model
    _whisper_model = None
    _whisper_key = None
    free_gpu()


def _get_whisper(model_name, device, compute_type):
    global _whisper_model, _whisper_key
    key = (model_name, device, compute_type)
    if _whisper_model is not None and _whisper_key == key:
        return _whisper_model
    if _whisper_model is not None:
        del _whisper_model
        _whisper_model = None
        free_gpu()
    import whisperx
    _whisper_model = whisperx.load_model(
        model_name, device, compute_type=compute_type, task="transcribe",
    )
    _whisper_key = key
    return _whisper_model


def process_song(cmd, device):
    audio_path = os.path.abspath(cmd["audio_path"])
    output_dir = os.path.abspath(cmd["cache_path"])
    file_hash = cmd["hash"]
    model_name = cmd.get("model", "large-v3")
    beam_size = cmd.get("beam_size", 8)
    batch_size = cmd.get("batch_size", 8)
    separator = cmd.get("separator", "karaoke")
    lyrics_path = cmd.get("lyrics")
    language_override = cmd.get("language")

    c_type = compute_type_for(device)
    actual_device = "cpu" if device == "mps" else device

    run_pipeline(
        audio_path, output_dir, file_hash, device,
        model_name=model_name,
        beam_size=beam_size,
        batch_size=batch_size,
        separator=separator,
        lyrics_path=lyrics_path,
        language_override=language_override,
        whisper_model=lambda: _get_whisper(model_name, actual_device, c_type),
        pre_align_cleanup=_clear_models,
        free_gpu_fn=lambda: free_gpu(),
    )


def main():
    device = detect_device()
    print(f"[nightingale:SERVER] ready device={device}", flush=True)

    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            cmd = json.loads(line)
        except json.JSONDecodeError as e:
            print(f"[nightingale:ERROR] Invalid JSON: {e}", flush=True)
            continue

        if cmd.get("command") == "quit":
            break

        if cmd.get("command") == "analyze":
            progress(0, "Starting analysis...")
            try:
                process_song(cmd, device)
                print("[nightingale:DONE]", flush=True)
            except Exception as e:
                import traceback
                traceback.print_exc(file=sys.stderr)
                err_str = str(e)
                if is_oom(err_str):
                    _clear_models()
                    print(f"[nightingale:OOM] {err_str}", flush=True)
                else:
                    print(f"[nightingale:ERROR] {err_str}", flush=True)


if __name__ == "__main__":
    main()
