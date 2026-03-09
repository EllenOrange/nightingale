#!/usr/bin/env python3
"""
Nightingale Song Analyzer
Separates vocals/instrumentals with Demucs and transcribes lyrics with WhisperX or Voxtral Realtime.

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


def build_segments(all_words: list[dict]) -> list[dict]:
    """Group words into segments based on time gaps; filter low-confidence edges if scores exist."""
    MAX_WORD_GAP = 3.0
    SENTENCE_GAP = 0.3
    EDGE_CONFIDENCE_THRESHOLD = 0.5

    def _flush(words):
        scores = [wd["score"] for wd in words if "score" in wd]
        avg_score = sum(scores) / len(scores) if scores else 0.0
        return {
            "text": " ".join(wd["word"] for wd in words),
            "start": words[0]["start"],
            "end": words[-1]["end"],
            "words": words,
            "_avg_score": avg_score,
        }

    segments = []
    current_words = []
    for w in all_words:
        if current_words:
            gap = w["start"] - current_words[-1]["end"]
            last_text = current_words[-1]["word"]
            next_text = w["word"]
            punctuation_end = last_text.rstrip().endswith((".", "!", "?"))
            capital_start = next_text[:1].isupper()
            if gap > MAX_WORD_GAP or (gap >= SENTENCE_GAP and (punctuation_end or capital_start)):
                segments.append(_flush(current_words))
                current_words = []
        current_words.append(w)

    if current_words:
        segments.append(_flush(current_words))

    for seg in segments:
        print(f"[nightingale:LOG] Segment [{seg['start']:.1f}-{seg['end']:.1f}] avg_score={seg['_avg_score']:.2f}: {seg['text'][:80]}", flush=True)

    has_scores = any("score" in w for w in all_words)
    if has_scores:
        while segments and segments[0]["_avg_score"] < EDGE_CONFIDENCE_THRESHOLD:
            dropped = segments.pop(0)
            print(f"[nightingale:LOG] Dropping low-confidence leading segment (score={dropped['_avg_score']:.2f}): {dropped['text'][:60]}", flush=True)

        while segments and segments[-1]["_avg_score"] < EDGE_CONFIDENCE_THRESHOLD:
            dropped = segments.pop()
            print(f"[nightingale:LOG] Dropping low-confidence trailing segment (score={dropped['_avg_score']:.2f}): {dropped['text'][:60]}", flush=True)

    for seg in segments:
        if "_avg_score" in seg:
            del seg["_avg_score"]

    return segments


def transcribe_voxtral(vocals_path: str, device: str) -> dict:
    """Transcribe vocals with Voxtral Realtime, deriving word timestamps from token positions."""
    from transformers import VoxtralRealtimeForConditionalGeneration, AutoProcessor
    import torchaudio
    from langdetect import detect as detect_lang

    SECS_PER_TOKEN = 0.08
    REPO_ID = "mistralai/Voxtral-Mini-4B-Realtime-2602"

    progress(55, "Loading Voxtral Realtime model...")
    processor = AutoProcessor.from_pretrained(REPO_ID)

    if device == "cuda":
        dtype = torch.bfloat16
    elif device == "mps":
        dtype = torch.float16
    else:
        dtype = torch.float32

    model = VoxtralRealtimeForConditionalGeneration.from_pretrained(
        REPO_ID, torch_dtype=dtype, device_map="auto"
    )
    print(f"[nightingale:LOG] Voxtral model loaded on {model.device} with dtype={model.dtype}", flush=True)

    progress(60, "Loading audio for Voxtral...")
    waveform, sr = torchaudio.load(vocals_path)
    if waveform.shape[0] > 1:
        waveform = waveform.mean(dim=0, keepdim=True)
    target_sr = processor.feature_extractor.sampling_rate
    if sr != target_sr:
        resampler = torchaudio.transforms.Resample(sr, target_sr)
        waveform = resampler(waveform)
    audio_array = waveform.squeeze(0).numpy()

    audio_duration = len(audio_array) / target_sr
    max_tokens = int(audio_duration / SECS_PER_TOKEN) + 100
    print(f"[nightingale:LOG] Audio loaded: {len(audio_array)} samples at {target_sr}Hz ({audio_duration:.1f}s), max_tokens={max_tokens}", flush=True)

    progress(65, f"Transcribing with Voxtral Realtime ({audio_duration:.0f}s audio)...")
    inputs = processor(audio_array, return_tensors="pt")
    inputs = inputs.to(model.device, dtype=model.dtype)

    with torch.no_grad():
        outputs = model.generate(**inputs, max_new_tokens=max_tokens)
    token_ids = outputs[0]

    progress(75, "Extracting word timestamps from token positions...")

    special_ids = set(processor.tokenizer.all_special_ids)

    token_offset = 0
    for t in token_ids:
        if t.item() in special_ids:
            token_offset += 1
        else:
            break

    print(f"[nightingale:LOG] Token sequence: {len(token_ids)} total, {token_offset} leading special tokens skipped", flush=True)

    words = []
    current_word = {"text": "", "start": None, "end": None}

    for i, token_id in enumerate(token_ids):
        tid = token_id.item()
        if tid in special_ids:
            if current_word["text"]:
                words.append(current_word)
                current_word = {"text": "", "start": None, "end": None}
            continue

        token_text = processor.tokenizer.decode(tid)
        pos = i - token_offset
        time_start = round(pos * SECS_PER_TOKEN, 3)
        time_end = round((pos + 1) * SECS_PER_TOKEN, 3)

        is_word_start = token_text.startswith((" ", "\u2581")) or not current_word["text"]

        if is_word_start and current_word["text"]:
            words.append(current_word)
            current_word = {"text": "", "start": None, "end": None}

        clean_text = token_text.lstrip(" \u2581")
        if not clean_text:
            continue

        if current_word["start"] is None:
            current_word["start"] = time_start
        current_word["end"] = time_end
        current_word["text"] += clean_text

    if current_word["text"]:
        words.append(current_word)

    all_words = [
        {"word": w["text"], "start": w["start"], "end": w["end"]}
        for w in words if w["text"].strip()
    ]

    full_text = " ".join(w["word"] for w in all_words)
    try:
        language = detect_lang(full_text)
    except Exception:
        language = "en"

    print(f"[nightingale:LOG] Voxtral transcription: {len(all_words)} words, language='{language}'", flush=True)
    if all_words:
        print(f"[nightingale:LOG] First word: {all_words[0]}", flush=True)
        print(f"[nightingale:LOG] Last word: {all_words[-1]}", flush=True)
        print(f"[nightingale:LOG] Text preview: {full_text[:200]}", flush=True)

    segments = build_segments(all_words)

    progress(90, f"Transcription complete: {len(segments)} segments, lang={language}")
    if segments:
        print(f"[nightingale:LOG] First segment: '{segments[0]['text'][:100]}'", flush=True)
        print(f"[nightingale:LOG] Last segment: '{segments[-1]['text'][:100]}'", flush=True)

    return {"language": language, "segments": segments}


def transcribe_vocals(
    vocals_path: str,
    original_audio_path: str,
    device: str,
    model_name: str = "large-v3-turbo",
    beam_size: int = 5,
    batch_size: int = 8,
) -> dict:
    """Transcribe vocals with WhisperX to get word-level timestamps."""
    import whisperx

    compute_type = "float16" if device == "cuda" else "float32"
    if device == "mps":
        device = "cpu"

    progress(55, f"Loading WhisperX model ({model_name})...")
    audio = whisperx.load_audio(vocals_path)
    print(f"[nightingale:LOG] Vocals audio loaded: {len(audio)} samples from {vocals_path}", flush=True)
    print(f"[nightingale:LOG] Settings: model={model_name}, beam_size={beam_size}, batch_size={batch_size}", flush=True)

    asr_options = {
        "beam_size": beam_size,
        "initial_prompt": "Song lyrics:",
    }

    model = whisperx.load_model(
        model_name, device, compute_type=compute_type, task="transcribe",
        asr_options=asr_options,
    )

    progress(58, "Detecting language from vocals (multi-window)...")
    language = detect_language_multiwindow(model, audio)
    print(f"[nightingale:LOG] Final detected language: '{language}'", flush=True)
    progress(59, f"Detected language: {language}")

    model = whisperx.load_model(
        model_name, device, compute_type=compute_type,
        task="transcribe", language=language,
        asr_options=asr_options,
    )
    print(f"[nightingale:LOG] Model loaded with lang={language}, tokenizer={model.tokenizer}", flush=True)

    progress(60, "Transcribing vocals...")
    result = model.transcribe(
        audio,
        batch_size=batch_size,
        task="transcribe",
        language=language,
    )

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

    def _interpolate_range(entries, start_idx, end_idx, gap_start, gap_end):
        n = end_idx - start_idx
        if n <= 0:
            return
        if gap_end > gap_start:
            d = (gap_end - gap_start) / n
            for j in range(n):
                entries[start_idx + j]["start"] = gap_start + j * d
                entries[start_idx + j]["end"] = gap_start + (j + 1) * d
        else:
            for j in range(n):
                entries[start_idx + j]["start"] = gap_start
                entries[start_idx + j]["end"] = gap_start + 0.1

    all_words = []
    total_aligned = 0
    total_interpolated = 0

    for seg in result["segments"]:
        raw_words = seg.get("words", [])
        if not raw_words:
            continue

        seg_start = seg.get("start", 0)
        seg_end = seg.get("end", 0)

        entries = []
        for w in raw_words:
            word_text = w.get("word", "").strip()
            if not word_text:
                continue
            has_ts = "start" in w and "end" in w
            entry = {
                "word": word_text,
                "start": w.get("start"),
                "end": w.get("end"),
                "score": w.get("score"),
                "aligned": has_ts,
            }
            if has_ts:
                duration = entry["end"] - entry["start"]
                if duration > MAX_WORD_DURATION:
                    print(f"[nightingale:LOG] Clamping long word '{word_text}' ({duration:.1f}s)", flush=True)
                    entry["start"] = entry["end"] - 0.5
            entries.append(entry)

        if not entries:
            continue

        anchors = [(i, e) for i, e in enumerate(entries) if e["aligned"]]

        if not anchors:
            n = len(entries)
            dur = (seg_end - seg_start) / n if seg_end > seg_start else 0.1
            for j, e in enumerate(entries):
                e["start"] = seg_start + j * dur
                e["end"] = seg_start + (j + 1) * dur
        else:
            first_idx = anchors[0][0]
            if first_idx > 0:
                _interpolate_range(entries, 0, first_idx, seg_start, entries[first_idx]["start"])

            for ai in range(len(anchors) - 1):
                a_idx = anchors[ai][0]
                b_idx = anchors[ai + 1][0]
                if b_idx - a_idx > 1:
                    _interpolate_range(entries, a_idx + 1, b_idx, entries[a_idx]["end"], entries[b_idx]["start"])

            last_idx = anchors[-1][0]
            if last_idx < len(entries) - 1:
                _interpolate_range(entries, last_idx + 1, len(entries), entries[last_idx]["end"], seg_end)

        seg_interpolated = sum(1 for e in entries if not e["aligned"])
        total_aligned += len(entries) - seg_interpolated
        total_interpolated += seg_interpolated

        if seg_interpolated > 0:
            print(f"[nightingale:LOG] Interpolated {seg_interpolated}/{len(entries)} words in segment [{seg_start:.1f}-{seg_end:.1f}]", flush=True)

        for e in entries:
            if e["start"] is None or e["end"] is None:
                continue
            word_entry = {
                "word": e["word"],
                "start": round(e["start"], 3),
                "end": round(e["end"], 3),
            }
            if e.get("score") is not None:
                word_entry["score"] = round(e["score"], 3)
            if not e["aligned"]:
                word_entry["estimated"] = True
            all_words.append(word_entry)

    print(f"[nightingale:LOG] Word stats: {total_aligned} aligned, {total_interpolated} interpolated, {len(all_words)} total", flush=True)

    segments = build_segments(all_words)

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
    parser.add_argument("--model", default="large-v3-turbo", help="Model name (large-v3, large-v3-turbo, voxtral-realtime)")
    parser.add_argument("--beam-size", type=int, default=5, help="Beam size for decoding")
    parser.add_argument("--batch-size", type=int, default=8, help="Batch size for transcription")
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

    if args.model == "voxtral-realtime":
        transcript = transcribe_voxtral(vocals_path, device)
    else:
        transcript = transcribe_vocals(
            vocals_path, audio_path, device,
            model_name=args.model,
            beam_size=args.beam_size,
            batch_size=args.batch_size,
        )

    progress(95, "Writing transcript...")
    with open(transcript_path, "w", encoding="utf-8") as f:
        json.dump(transcript, f, ensure_ascii=False, indent=2)

    progress(100, "DONE")


if __name__ == "__main__":
    main()
