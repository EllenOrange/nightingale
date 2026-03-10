"""Lyrics alignment: align pre-fetched lyrics text to vocals audio using WhisperX."""

import json
import re

from audio import detect_vocal_region
from language import detect_language_multiwindow
from whisper_compat import progress, align_device_for, compute_type_for


def align_lyrics(
    lyrics_path: str,
    vocals_path: str,
    device: str,
    model_name: str = "large-v3-turbo",
) -> dict:
    """Align pre-existing lyrics to vocals audio using WhisperX.

    Steps:
      1. Load lyrics text from JSON
      2. Load vocals audio and detect vocal region via RMS
      3. Detect language
      4. Probe multiple start offsets to find where vocals actually begin
      5. Run final forced alignment from the best offset
      6. Map aligned word timestamps back to original lyric lines
    """
    import whisperx

    progress(55, "Loading lyrics...")
    with open(lyrics_path, "r", encoding="utf-8") as f:
        lyrics_data = json.load(f)

    lines = lyrics_data.get("lines", [])
    print(f"[nightingale:LOG] Lyrics loaded: {len(lines)} lines", flush=True)

    audio = whisperx.load_audio(vocals_path)
    duration_secs = len(audio) / 16000
    print(f"[nightingale:LOG] Vocals audio loaded: {len(audio)} samples ({duration_secs:.1f}s)", flush=True)

    clean_lines = []
    for line in lines:
        text = line.strip() if isinstance(line, str) else str(line).strip()
        if text:
            clean_lines.append(text)

    progress(56, "Detecting vocal regions...")
    vocal_start, vocal_end = detect_vocal_region(audio)

    a_device = align_device_for(device)
    c_type = compute_type_for(device)

    progress(58, "Detecting language...")
    lang_model = whisperx.load_model(
        model_name, a_device, compute_type=c_type, task="transcribe",
    )
    language = detect_language_multiwindow(lang_model, audio)
    del lang_model
    print(f"[nightingale:LOG] Detected language: '{language}'", flush=True)
    progress(59, f"Detected language: {language}")

    best_start = _probe_start_offset(
        whisperx, audio, clean_lines, vocal_start, vocal_end,
        language, a_device, model_name,
    )

    progress(80, f"Final alignment from {best_start:.1f}s...")
    print(f"[nightingale:LOG] Loading align model for language='{language}' on device='{a_device}'", flush=True)
    align_model, metadata = whisperx.load_align_model(language_code=language, device=a_device)
    full_text = " ".join(clean_lines)
    raw_segments = [{"text": full_text, "start": best_start, "end": vocal_end}]
    align_result = whisperx.align(raw_segments, align_model, metadata, audio, a_device)
    del align_model

    segments = _map_words_to_lines(align_result, clean_lines)

    progress(90, f"Alignment complete: {len(segments)} segments, lang={language}")
    if segments:
        print(f"[nightingale:LOG] First segment: '{segments[0]['text'][:100]}'", flush=True)
        print(f"[nightingale:LOG] Last segment: '{segments[-1]['text'][:100]}'", flush=True)

    return {"language": language, "segments": segments, "source": "lyrics"}


def _probe_start_offset(whisperx, audio, clean_lines, vocal_start, vocal_end,
                         language, a_device, model_name) -> float:
    """Try multiple start offsets and pick the one where the first word aligns best."""
    first_line_word_count = len(clean_lines[0].split()) if clean_lines else 5
    full_text = " ".join(clean_lines)

    progress(60, "Aligning lyrics to audio...")
    print(f"[nightingale:LOG] Loading align model for language='{language}' on device='{a_device}'", flush=True)
    align_model, metadata = whisperx.load_align_model(language_code=language, device=a_device)

    early_accept = 0.9
    max_probes = 8
    best_start = vocal_start
    best_score = -1.0

    current_start = vocal_start
    print(f"[nightingale:LOG] Probing for best start offset: max_probes={max_probes}, lines={len(clean_lines)}", flush=True)

    for probe in range(max_probes):
        if current_start >= vocal_end:
            print(f"[nightingale:LOG] Offset {current_start:.1f}s >= vocal_end {vocal_end:.1f}s, stopping probes", flush=True)
            break

        probe_segs = [{"text": full_text, "start": current_start, "end": vocal_end}]
        print(f"[nightingale:LOG] Probe {probe + 1}: segment {current_start:.1f}s-{vocal_end:.1f}s", flush=True)
        progress(62 + probe, f"Probing start offset ({probe + 1}/{max_probes}, {current_start:.1f}s)...")

        result = whisperx.align(probe_segs, align_model, metadata, audio, a_device)
        all_words = _collect_words(result)

        if not all_words:
            print(f"[nightingale:LOG]   no aligned words", flush=True)
            current_start += 5.0
            continue

        first = all_words[0]
        score = first.get("score", 0.0) or 0.0
        print(f"[nightingale:LOG]   first word: {first.get('word', '?')}({first.get('start', '?')}-{first.get('end', '?')}, score={score:.3f})", flush=True)

        first_word_end = first.get("end", current_start + 5.0)
        next_start = max(first_word_end - 3.0, current_start + 2.0)
        step = next_start - current_start

        print(f"[nightingale:LOG] Probe {probe + 1} result: score={score:.3f}, first_word_end={first_word_end}s, next_probe={current_start + step:.1f}s", flush=True)

        if score > best_score:
            best_score = score
            best_start = current_start
            print(f"[nightingale:LOG]   -> New best start! offset={best_start:.1f}s, score={best_score:.3f}", flush=True)

        if score >= early_accept:
            print(f"[nightingale:LOG] Score {score:.3f} >= {early_accept}, early accept", flush=True)
            break

        current_start += step

    del align_model
    print(f"[nightingale:LOG] Best start offset: {best_start:.1f}s (score={best_score:.3f})", flush=True)
    return best_start


def _collect_words(align_result: dict) -> list[dict]:
    """Extract all words with timestamps from alignment result."""
    words = []
    for seg in align_result.get("segments", []):
        for w in seg.get("words", []):
            word_text = w.get("word", "").strip()
            if word_text and "start" in w and "end" in w:
                words.append(w)
    return words


def _map_words_to_lines(align_result: dict, clean_lines: list[str]) -> list[dict]:
    """Map aligned word timestamps back to original lyric lines."""
    all_aligned_words = _collect_words(align_result)
    print(f"[nightingale:LOG] Final alignment: {len(all_aligned_words)} words aligned", flush=True)

    word_times: dict[str, list[tuple]] = {}
    for w in all_aligned_words:
        key = re.sub(r"[^\w]", "", w["word"]).lower()
        if key not in word_times:
            word_times[key] = []
        word_times[key].append((w["start"], w["end"], w.get("score")))

    used_counts: dict[str, int] = {}
    segments = []

    for line_text in clean_lines:
        line_words = line_text.split()
        word_entries = []

        for word_text in line_words:
            key = re.sub(r"[^\w]", "", word_text).lower()
            idx = used_counts.get(key, 0)
            times_list = word_times.get(key, [])
            if idx < len(times_list):
                start, end, score = times_list[idx]
                entry = {"word": word_text, "start": round(start, 3), "end": round(end, 3)}
                if score is not None:
                    entry["score"] = round(score, 3)
                used_counts[key] = idx + 1
            else:
                entry = {"word": word_text, "start": None, "end": None, "estimated": True}
            word_entries.append(entry)

        _interpolate_missing(word_entries)

        valid_words = [e for e in word_entries if e["start"] is not None]
        if not valid_words:
            continue

        segments.append({
            "text": line_text,
            "start": valid_words[0]["start"],
            "end": valid_words[-1]["end"],
            "words": valid_words,
        })

    print(f"[nightingale:LOG] Lyrics alignment: {len(segments)} lines preserved, {sum(len(s['words']) for s in segments)} words", flush=True)

    MAX_WORDS_PER_LINE = 10
    split_segments = []
    for seg in segments:
        words = seg["words"]
        if len(words) <= MAX_WORDS_PER_LINE:
            split_segments.append(seg)
            continue
        for chunk in [words[i:i+MAX_WORDS_PER_LINE] for i in range(0, len(words), MAX_WORDS_PER_LINE)]:
            split_segments.append({
                "text": " ".join(w["word"] for w in chunk),
                "start": chunk[0]["start"],
                "end": chunk[-1]["end"],
                "words": chunk,
            })

    return split_segments


def _interpolate_missing(word_entries: list[dict]):
    """Fill in timestamps for words the aligner couldn't place, using neighbors."""
    unset = [i for i, e in enumerate(word_entries) if e["start"] is None]
    set_entries = [e for e in word_entries if e["start"] is not None]

    if not unset or not set_entries:
        return

    for ui in unset:
        prev_end = set_entries[0]["start"]
        next_start = set_entries[-1]["end"]
        for j in range(ui - 1, -1, -1):
            if word_entries[j]["start"] is not None:
                prev_end = word_entries[j]["end"]
                break
        for j in range(ui + 1, len(word_entries)):
            if word_entries[j]["start"] is not None:
                next_start = word_entries[j]["start"]
                break
        mid = (prev_end + next_start) / 2
        word_entries[ui]["start"] = round(prev_end, 3)
        word_entries[ui]["end"] = round(mid, 3)
