#!/usr/bin/env python3
"""
Nightingale Song Analyzer
Separates vocals/instrumentals with Demucs and transcribes lyrics with WhisperX.

Usage:
    python analyze.py <audio_path> <output_dir> [--hash <file_hash>]

Outputs (in output_dir):
    {hash}_instrumental.wav
    {hash}_vocals.wav
    {hash}_transcript.json

Progress protocol (parsed by Rust app):
    [nightingale:PROGRESS:<percent>] <message>
"""

import argparse
import hashlib
import json
import os
import sys
import tempfile
from pathlib import Path

import torch

# PyTorch 2.6+ defaults torch.load to weights_only=True, but pyannote
# checkpoints serialize many omegaconf types that aren't in the safe list.
# We trust HuggingFace model checkpoints, so override the default.
_original_torch_load = torch.load
def _patched_torch_load(*args, **kwargs):
    kwargs["weights_only"] = False
    return _original_torch_load(*args, **kwargs)
torch.load = _patched_torch_load


def progress(pct: int, msg: str):
    print(f"[nightingale:PROGRESS:{pct}] {msg}", flush=True)


def detect_device() -> str:
    if torch.cuda.is_available():
        return "cuda"
    if torch.backends.mps.is_available():
        return "mps"
    return "cpu"


def compute_hash(path: str) -> str:
    h = hashlib.blake2b(digest_size=16)
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()


def separate_stems(audio_path: str, work_dir: str, device: str) -> tuple[str, str]:
    """Run Demucs to separate vocals and instrumental stems."""
    from demucs.apply import apply_model
    from demucs.audio import save_audio
    from demucs.pretrained import get_model

    import torchaudio

    progress(5, "Loading Demucs model...")
    model = get_model("htdemucs")
    actual_device = torch.device(device if device != "mps" else "cpu")
    model.to(actual_device)

    progress(10, "Loading audio file...")
    wav, sr = torchaudio.load(audio_path)
    wav = wav.to(actual_device)

    ref = wav.mean(0)
    wav_centered = wav - ref.mean()
    wav_scaled = wav_centered / ref.abs().max().clamp(min=1e-8)

    progress(15, "Separating vocals from instrumentals...")
    sources = apply_model(model, wav_scaled[None], device=actual_device, shifts=1, overlap=0.25)[0]

    source_names = model.sources
    vocals_idx = source_names.index("vocals")

    vocals = sources[vocals_idx] * ref.abs().max() + ref.mean()
    instrumental = (wav.to(actual_device) - (sources[vocals_idx] * ref.abs().max() + ref.mean()))

    progress(45, "Saving separated stems...")

    vocals_path = os.path.join(work_dir, "vocals.wav")
    instrumental_path = os.path.join(work_dir, "instrumental.wav")

    save_audio(vocals.cpu(), vocals_path, sr)
    save_audio(instrumental.cpu(), instrumental_path, sr)

    progress(50, "Stem separation complete")
    return vocals_path, instrumental_path


def detect_language_multiwindow(model, audio, sample_rate=16000, window_secs=30) -> str:
    """Detect language by sampling multiple 30s windows and voting."""
    from whisperx.audio import log_mel_spectrogram
    from collections import Counter

    window_samples = window_secs * sample_rate
    total_samples = len(audio)
    n_mels = model.model.feat_kwargs.get("feature_size") or 80

    offsets = [0]
    if total_samples > window_samples:
        offsets.append(total_samples // 2 - window_samples // 2)
    if total_samples > window_samples * 2:
        offsets.append(total_samples // 4)
        offsets.append(total_samples * 3 // 4 - window_samples)

    votes = []
    for offset in offsets:
        offset = max(0, min(offset, total_samples - window_samples))
        chunk = audio[offset : offset + window_samples]
        padding = max(0, window_samples - len(chunk))
        segment = log_mel_spectrogram(chunk, n_mels=n_mels, padding=padding)
        encoder_output = model.model.encode(segment)
        results = model.model.model.detect_language(encoder_output)
        lang_token, prob = results[0][0]
        lang = lang_token[2:-2]
        print(f"[nightingale:LOG] Window @{offset/sample_rate:.0f}s: lang={lang} prob={prob:.2f}", flush=True)
        votes.append((lang, prob))

    lang_scores: dict[str, float] = {}
    for lang, prob in votes:
        lang_scores[lang] = lang_scores.get(lang, 0.0) + prob

    best_lang = max(lang_scores, key=lambda l: lang_scores[l])
    print(f"[nightingale:LOG] Language scores: {lang_scores} -> '{best_lang}'", flush=True)
    return best_lang


def transcribe_vocals(vocals_path: str, original_audio_path: str, device: str) -> dict:
    """Transcribe vocals with WhisperX to get word-level timestamps."""
    import whisperx

    compute_type = "float16" if device == "cuda" else "float32"
    if device == "mps":
        device = "cpu"

    progress(55, "Loading WhisperX model...")
    audio = whisperx.load_audio(vocals_path)
    print(f"[nightingale:LOG] Vocals audio loaded: {len(audio)} samples from {vocals_path}", flush=True)

    model = whisperx.load_model(
        "large-v3-turbo", device, compute_type=compute_type, task="transcribe"
    )

    progress(58, "Detecting language from vocals (multi-window)...")
    language = detect_language_multiwindow(model, audio)
    print(f"[nightingale:LOG] Final detected language: '{language}'", flush=True)
    progress(59, f"Detected language: {language}")

    model = whisperx.load_model(
        "large-v3-turbo", device, compute_type=compute_type,
        task="transcribe", language=language,
    )
    print(f"[nightingale:LOG] Model loaded with lang={language}, tokenizer={model.tokenizer}", flush=True)

    progress(60, "Transcribing vocals...")
    result = model.transcribe(audio, batch_size=8, task="transcribe", language=language)

    result_language = result.get("language", language)
    print(f"[nightingale:LOG] Transcribe returned language='{result_language}', segments={len(result.get('segments', []))}", flush=True)
    if result.get("segments"):
        first_seg = result["segments"][0]
        print(f"[nightingale:LOG] First segment text: '{first_seg.get('text', '')[:100]}'", flush=True)
        print(f"[nightingale:LOG] First segment time: {first_seg.get('start')} -> {first_seg.get('end')}", flush=True)
    progress(75, f"Language: {result_language}")

    progress(80, f"Aligning word timestamps (lang={result_language})...")
    print(f"[nightingale:LOG] Loading align model for language='{result_language}' on device='{device}'", flush=True)
    align_model, metadata = whisperx.load_align_model(language_code=result_language, device=device)
    result = whisperx.align(result["segments"], align_model, metadata, audio, device)

    MAX_WORD_DURATION = 5.0
    EDGE_CONFIDENCE_THRESHOLD = 0.5

    segments = []
    for seg in result["segments"]:
        words = []
        for w in seg.get("words", []):
            if "start" not in w or "end" not in w:
                continue
            start = w["start"]
            end = w["end"]
            duration = end - start
            if duration > MAX_WORD_DURATION:
                new_start = end - 0.5
                print(f"[nightingale:LOG] Fixing misaligned word '{w['word'].strip()}' ({duration:.1f}s): {start:.1f}->{new_start:.1f}", flush=True)
                start = new_start
            word_entry = {
                "word": w["word"].strip(),
                "start": round(start, 3),
                "end": round(end, 3),
            }
            if "score" in w:
                word_entry["score"] = round(w["score"], 3)
            words.append(word_entry)
        if words:
            scores = [w["score"] for w in words if "score" in w]
            avg_score = sum(scores) / len(scores) if scores else 0.0
            segments.append({
                "text": " ".join(w["word"] for w in words),
                "start": words[0]["start"],
                "end": words[-1]["end"],
                "words": words,
                "_avg_score": avg_score,
            })
            print(f"[nightingale:LOG] Segment [{words[0]['start']:.1f}-{words[-1]['end']:.1f}] avg_score={avg_score:.2f}: {segments[-1]['text'][:80]}", flush=True)

    while segments and segments[0]["_avg_score"] < EDGE_CONFIDENCE_THRESHOLD:
        dropped = segments.pop(0)
        print(f"[nightingale:LOG] Dropping low-confidence leading segment (score={dropped['_avg_score']:.2f}): {dropped['text'][:60]}", flush=True)

    while segments and segments[-1]["_avg_score"] < EDGE_CONFIDENCE_THRESHOLD:
        dropped = segments.pop()
        print(f"[nightingale:LOG] Dropping low-confidence trailing segment (score={dropped['_avg_score']:.2f}): {dropped['text'][:60]}", flush=True)

    for seg in segments:
        del seg["_avg_score"]

    progress(90, f"Transcription complete: {len(segments)} segments, lang={result_language}")
    if segments:
        print(f"[nightingale:LOG] First aligned segment: '{segments[0]['text'][:100]}'", flush=True)
        print(f"[nightingale:LOG] First word: '{segments[0]['words'][0]}'", flush=True)
        print(f"[nightingale:LOG] Last segment: '{segments[-1]['text'][:100]}'", flush=True)
    return {"language": result_language, "segments": segments}


def main():
    parser = argparse.ArgumentParser(description="Nightingale Song Analyzer")
    parser.add_argument("audio_path", help="Path to the audio file")
    parser.add_argument("output_dir", help="Directory to write output files")
    parser.add_argument("--hash", dest="file_hash", help="Pre-computed file hash (skip computing)")
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

    final_vocals = os.path.join(output_dir, f"{file_hash}_vocals.wav")
    final_instrumental = os.path.join(output_dir, f"{file_hash}_instrumental.wav")

    if os.path.isfile(final_vocals) and os.path.isfile(final_instrumental):
        progress(50, "Stems already cached, skipping separation")
        vocals_path = final_vocals
    else:
        with tempfile.TemporaryDirectory(prefix="nightingale_") as work_dir:
            vocals_path, instrumental_path = separate_stems(audio_path, work_dir, device)
            progress(92, "Saving stems to cache...")
            import shutil
            shutil.move(vocals_path, final_vocals)
            shutil.move(instrumental_path, final_instrumental)
        vocals_path = final_vocals

    transcript = transcribe_vocals(vocals_path, audio_path, device)

    progress(95, "Writing transcript...")
    with open(transcript_path, "w", encoding="utf-8") as f:
        json.dump(transcript, f, ensure_ascii=False, indent=2)

    progress(100, "DONE")


if __name__ == "__main__":
    main()
