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


import re

BANNED_WORDS = {
    "dimatorzok", "dimatorsok", "dima_torzok",
    "amara.org",
}

ATTRIBUTION_WORDS = {
    "субтитры", "субтитр", "подписи", "титры",
    "сделал", "сделала", "сделали",
    "создал", "создала", "создали",
    "делал", "делала", "делали",
    "создавал", "создавала", "создавали",
    "подготовил", "подготовила", "подготовили",
    "редактировал", "редактировала", "редактировали",
    "выполнил", "выполнила", "выполнили",
    "subtitles", "subtitle", "captions", "caption",
    "transcribed", "transcript", "transcription",
}


def _remove_hallucinations(all_words: list[dict]) -> list[dict]:
    if not all_words:
        return all_words

    to_remove: set[int] = set()

    for i, w in enumerate(all_words):
        clean = re.sub(r"[.,!?;:\"\']", "", w["word"]).lower()
        if clean in BANNED_WORDS:
            to_remove.add(i)
            for j in range(max(0, i - 4), i):
                neighbor = re.sub(r"[.,!?;:\"\']", "", all_words[j]["word"]).lower()
                if neighbor in ATTRIBUTION_WORDS:
                    to_remove.add(j)
            for j in range(i + 1, min(len(all_words), i + 3)):
                neighbor = re.sub(r"[.,!?;:\"\']", "", all_words[j]["word"]).lower()
                if neighbor in ATTRIBUTION_WORDS:
                    to_remove.add(j)

    if to_remove:
        removed_text = " ".join(all_words[i]["word"] for i in sorted(to_remove))
        print(f"[nightingale:LOG] Removed hallucination ({len(to_remove)} words): {removed_text}", flush=True)

    return [w for i, w in enumerate(all_words) if i not in to_remove]


def build_segments(all_words: list[dict]) -> list[dict]:
    """Group words into segments based on time gaps."""
    MAX_WORD_GAP = 3.0
    MIN_SENTENCE_GAP = 0.05
    MIN_WORDS_PER_LINE = 3

    def _flush(words):
        return {
            "text": " ".join(wd["word"] for wd in words),
            "start": words[0]["start"],
            "end": words[-1]["end"],
            "words": words,
        }

    segments = []
    current_words = []
    for w in all_words:
        if current_words:
            gap = w["start"] - current_words[-1]["end"]
            last_text = current_words[-1]["word"]
            next_text = w["word"]
            punctuation_end = last_text.rstrip().endswith((".", "!", "?", ","))
            capital_start = next_text[:1].isupper()
            long_enough = len(current_words) >= MIN_WORDS_PER_LINE

            if gap > MAX_WORD_GAP:
                segments.append(_flush(current_words))
                current_words = []
            elif long_enough and gap >= MIN_SENTENCE_GAP and punctuation_end and capital_start:
                segments.append(_flush(current_words))
                current_words = []
        current_words.append(w)

    if current_words:
        segments.append(_flush(current_words))

    merged = True
    while merged:
        merged = False
        i = 0
        while i < len(segments):
            if len(segments[i]["words"]) < MIN_WORDS_PER_LINE and len(segments) > 1:
                if i == 0:
                    neighbor = i + 1
                elif i == len(segments) - 1:
                    neighbor = i - 1
                else:
                    gap_before = segments[i]["start"] - segments[i - 1]["end"]
                    gap_after = segments[i + 1]["start"] - segments[i]["end"]
                    neighbor = i - 1 if gap_before <= gap_after else i + 1
                if neighbor < i:
                    segments[neighbor] = _flush(segments[neighbor]["words"] + segments[i]["words"])
                    segments.pop(i)
                else:
                    segments[neighbor] = _flush(segments[i]["words"] + segments[neighbor]["words"])
                    segments.pop(i)
                merged = True
            else:
                i += 1

    for seg in segments:
        print(f"[nightingale:LOG] Segment [{seg['start']:.1f}-{seg['end']:.1f}]: {seg['text'][:80]}", flush=True)

    MAX_WORDS_PER_LINE = 10
    MIN_SPLIT_SIZE = 4
    split_segments = []
    for seg in segments:
        words = seg["words"]
        if len(words) <= MAX_WORDS_PER_LINE:
            split_segments.append(seg)
            continue
        remaining = words
        while len(remaining) > MAX_WORDS_PER_LINE:
            search_end = min(len(remaining), MAX_WORDS_PER_LINE + MIN_SPLIT_SIZE)
            best_gap = -1.0
            best_idx = search_end // 2
            for j in range(MIN_SPLIT_SIZE, search_end):
                gap = remaining[j]["start"] - remaining[j - 1]["end"]
                if gap > best_gap:
                    best_gap = gap
                    best_idx = j
            split_segments.append(_flush(remaining[:best_idx]))
            remaining = remaining[best_idx:]
        if remaining:
            split_segments.append(_flush(remaining))
    if len(split_segments) != len(segments):
        print(f"[nightingale:LOG] Split long lines: {len(segments)} -> {len(split_segments)} segments (max {MAX_WORDS_PER_LINE} words/line)", flush=True)
    segments = split_segments

    return segments


def _normalize(word: str) -> str:
    return re.sub(r"[^\w]", "", word).lower()


def _recover_dropped_words(raw_segments: list[dict], aligned_segments: list[dict]):
    """Compare input vs aligned output and re-inject words the aligner silently dropped.

    Works per-segment using the segment text as the source of truth. Uses a
    sliding-window match so punctuation and minor normalization differences
    don't cause false positives.
    """
    if len(raw_segments) != len(aligned_segments):
        print(
            f"[nightingale:LOG] Segment count mismatch (raw={len(raw_segments)}, "
            f"aligned={len(aligned_segments)}), skipping word recovery",
            flush=True,
        )
        return

    total_recovered = 0

    for seg_i, (raw_seg, aligned_seg) in enumerate(zip(raw_segments, aligned_segments)):
        raw_text = raw_seg.get("text", "")
        raw_words = raw_text.split()
        aligned_words: list[dict] = aligned_seg.get("words", [])

        if not raw_words:
            continue

        aligned_norms = [_normalize(w.get("word", "")) for w in aligned_words]

        matched_raw: set[int] = set()
        matched_aligned: set[int] = set()
        ai = 0
        for ri, rw in enumerate(raw_words):
            rn = _normalize(rw)
            if not rn:
                matched_raw.add(ri)
                continue
            for si in range(ai, min(ai + 8, len(aligned_norms))):
                if si not in matched_aligned and aligned_norms[si] == rn:
                    matched_raw.add(ri)
                    matched_aligned.add(si)
                    ai = si + 1
                    break

        missing_indices = [i for i in range(len(raw_words)) if i not in matched_raw]
        if not missing_indices:
            continue

        seg_start = aligned_seg.get("start", raw_seg.get("start", 0))
        seg_end = aligned_seg.get("end", raw_seg.get("end", 0))
        missing_text = " ".join(raw_words[i] for i in missing_indices)
        print(
            f"[nightingale:LOG] Recovering {len(missing_indices)} dropped words in segment "
            f"[{seg_start:.1f}-{seg_end:.1f}]: {missing_text}",
            flush=True,
        )

        for orig_idx in reversed(missing_indices):
            insert_pos = len(aligned_words)
            for check_ri in range(orig_idx + 1, len(raw_words)):
                check_norm = _normalize(raw_words[check_ri])
                for ai_pos, an in enumerate(aligned_norms):
                    if an == check_norm:
                        insert_pos = ai_pos
                        break
                if insert_pos < len(aligned_words):
                    break

            recovered = {"word": raw_words[orig_idx]}
            aligned_words.insert(insert_pos, recovered)
            aligned_norms.insert(insert_pos, _normalize(raw_words[orig_idx]))
            total_recovered += 1

        aligned_seg["words"] = aligned_words

    if total_recovered > 0:
        print(f"[nightingale:LOG] Total recovered words: {total_recovered}", flush=True)


def align_and_build_segments(raw_segments: list[dict], audio, language: str, device: str) -> dict:
    """Run WhisperX forced alignment, interpolate unaligned words, and build final segments."""
    import whisperx

    align_device = "cpu" if device == "mps" else device

    input_words_by_seg: list[list[str]] = []
    total_input_words = 0
    for seg in raw_segments:
        seg_text = seg.get("text", "")
        words = seg_text.split()
        input_words_by_seg.append(words)
        total_input_words += len(words)
    print(f"[nightingale:LOG] Pre-alignment: {len(raw_segments)} segments, {total_input_words} words total", flush=True)

    progress(80, f"Aligning word timestamps (lang={language})...")
    print(f"[nightingale:LOG] Loading align model for language='{language}' on device='{align_device}'", flush=True)
    align_model, metadata = whisperx.load_align_model(language_code=language, device=align_device)
    result = whisperx.align(raw_segments, align_model, metadata, audio, align_device)

    output_segments = result.get("segments", [])
    total_output_words = sum(len(s.get("words", [])) for s in output_segments)
    print(f"[nightingale:LOG] Post-alignment: {len(output_segments)} segments, {total_output_words} words total", flush=True)

    _recover_dropped_words(raw_segments, output_segments)

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

    for seg in output_segments:
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
                print(f"[nightingale:LOG] WARNING: Dropping word with no timestamps after interpolation: '{e['word']}'", flush=True)
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

    all_words = _remove_hallucinations(all_words)
    segments = build_segments(all_words)

    progress(90, f"Transcription complete: {len(segments)} segments, lang={language}")
    if segments:
        print(f"[nightingale:LOG] First segment: '{segments[0]['text'][:100]}'", flush=True)
        if segments[0].get("words"):
            print(f"[nightingale:LOG] First word: '{segments[0]['words'][0]}'", flush=True)
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
    duration_secs = len(audio) / 16000
    print(f"[nightingale:LOG] Vocals audio loaded: {len(audio)} samples ({duration_secs:.1f}s) from {vocals_path}", flush=True)
    print(f"[nightingale:LOG] Settings: model={model_name}, beam_size={beam_size}, batch_size={batch_size}", flush=True)

    asr_options = {
        "beam_size": beam_size,
        "initial_prompt": "Song lyrics transcription for karaoke.",
    }

    vad_options = {
        "vad_onset": 0.12,
        "vad_offset": 0.05,
        "min_duration_on": 0.15,
        "min_duration_off": 0.6,
    }

    model = whisperx.load_model(
        model_name, device, compute_type=compute_type, task="transcribe",
        asr_options=asr_options, vad_options=vad_options,
    )

    progress(58, "Detecting language from vocals (multi-window)...")
    language = detect_language_multiwindow(model, audio)
    print(f"[nightingale:LOG] Final detected language: '{language}'", flush=True)
    progress(59, f"Detected language: {language}")

    model = whisperx.load_model(
        model_name, device, compute_type=compute_type,
        task="transcribe", language=language,
        asr_options=asr_options, vad_options=vad_options,
    )
    print(f"[nightingale:LOG] Model loaded with lang={language}, tokenizer={model.tokenizer}", flush=True)

    progress(60, "Transcribing vocals...")
    result = model.transcribe(
        audio,
        batch_size=batch_size,
        task="transcribe",
        language=language,
        chunk_size=15,
    )

    result_language = result.get("language", language)
    raw_segments = result.get("segments", [])
    total_raw_words = sum(len(s.get("text", "").split()) for s in raw_segments)
    print(f"[nightingale:LOG] Transcribe returned language='{result_language}', segments={len(raw_segments)}, ~{total_raw_words} words", flush=True)

    if raw_segments:
        covered = sum(s.get("end", 0) - s.get("start", 0) for s in raw_segments)
        print(f"[nightingale:LOG] Transcribed coverage: {covered:.1f}s / {duration_secs:.1f}s ({covered/duration_secs*100:.0f}%)", flush=True)

        gaps = []
        for i in range(1, len(raw_segments)):
            gap_start = raw_segments[i - 1].get("end", 0)
            gap_end = raw_segments[i].get("start", 0)
            gap = gap_end - gap_start
            if gap > 5.0:
                gaps.append((gap_start, gap_end, gap))
        if gaps:
            print(f"[nightingale:LOG] Large gaps (>5s) in transcript:", flush=True)
            for gs, ge, g in gaps:
                print(f"[nightingale:LOG]   {gs:.1f}s - {ge:.1f}s ({g:.1f}s gap)", flush=True)

        for i, seg in enumerate(raw_segments):
            print(f"[nightingale:LOG] Seg {i}: [{seg.get('start',0):.1f}-{seg.get('end',0):.1f}] {seg.get('text','')[:80]}", flush=True)

    progress(75, f"Language: {result_language}")

    return align_and_build_segments(raw_segments, audio, result_language, device)


def main():
    parser = argparse.ArgumentParser(description="Nightingale Song Analyzer")
    parser.add_argument("audio_path", help="Path to the audio file")
    parser.add_argument("output_dir", help="Directory to write output files")
    parser.add_argument("--hash", dest="file_hash", help="Pre-computed file hash (skip computing)")
    parser.add_argument("--model", default="large-v3-turbo", help="Model name (large-v3, large-v3-turbo)")
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
