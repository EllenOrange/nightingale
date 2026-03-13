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
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from whisper_compat import progress, detect_device, compute_type_for, is_oom, free_gpu
from stems import separate_stems, separate_stems_uvr, KARAOKE_MODEL
from transcribe import transcribe_vocals
from align import align_lyrics

_whisper_model = None
_whisper_key = None  # (model_name, device, compute_type)


def _clear_models():
    global _whisper_model, _whisper_key
    if _whisper_model is not None:
        del _whisper_model
    _whisper_model = None
    _whisper_key = None
    _free_gpu()


def _get_whisper(model_name, device, compute_type):
    global _whisper_model, _whisper_key
    key = (model_name, device, compute_type)
    if _whisper_model is not None and _whisper_key == key:
        return _whisper_model
    if _whisper_model is not None:
        del _whisper_model
        _whisper_model = None
        _free_gpu()
    import whisperx
    _whisper_model = whisperx.load_model(
        model_name, device, compute_type=compute_type, task="transcribe",
    )
    _whisper_key = key
    return _whisper_model


def _free_gpu():
    free_gpu()


def _ffmpeg_bin():
    return os.environ.get("FFMPEG_PATH", "ffmpeg")


def _convert_to_ogg(src_wav, dest_ogg):
    subprocess.run(
        [_ffmpeg_bin(), "-y", "-i", src_wav, "-c:a", "libvorbis", "-q:a", "6", "-v", "error", dest_ogg],
        check=True,
    )
    if os.path.isfile(dest_ogg):
        os.remove(src_wav)


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

    os.makedirs(output_dir, exist_ok=True)

    transcript_path = os.path.join(output_dir, f"{file_hash}_transcript.json")
    if os.path.isfile(transcript_path):
        progress(100, "Already analyzed, skipping")
        return

    progress(2, f"Using device: {device}")

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
            if separator == "karaoke":
                torch_home = os.environ.get("TORCH_HOME", "")
                models_base = os.path.dirname(torch_home) if torch_home else output_dir
                uvr_models_dir = os.path.join(models_base, "audio_separator")
                os.makedirs(uvr_models_dir, exist_ok=True)
                vp, ip = separate_stems_uvr(audio_path, work_dir, uvr_models_dir)
            else:
                vp, ip = separate_stems(audio_path, work_dir, device)
            progress(51, "Saving stems to cache...")
            _convert_to_ogg(vp, final_vocals_ogg)
            _convert_to_ogg(ip, final_instrumental_ogg)
        _free_gpu()
        vocals_path = final_vocals_ogg

    c_type = compute_type_for(device)
    actual_device = "cpu" if device == "mps" else device
    whisper = _get_whisper(model_name, actual_device, c_type)

    if lyrics_path and os.path.isfile(lyrics_path):
        print(f"[nightingale:LOG] Using pre-fetched lyrics: {lyrics_path}", flush=True)
        transcript = align_lyrics(
            lyrics_path, vocals_path, device,
            model_name=model_name,
            language_override=language_override,
            whisper_model=whisper,
        )
    else:
        transcript = transcribe_vocals(
            vocals_path, audio_path, device,
            model_name=model_name,
            beam_size=beam_size,
            batch_size=batch_size,
            language_override=language_override,
            whisper_model=whisper,
        )

    progress(95, "Writing transcript...")
    with open(transcript_path, "w", encoding="utf-8") as f:
        json.dump(transcript, f, ensure_ascii=False, indent=2)


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
