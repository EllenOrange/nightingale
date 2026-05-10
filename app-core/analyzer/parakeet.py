"""Parakeet TDT 0.6B v3 ASR backends.

Two interchangeable backends keyed off the runtime device:
  - CUDA       -> NVIDIA NeMo (`nemo_toolkit[asr]`)
  - cpu / mps  -> ONNX Runtime via `onnx-asr`

Both produce raw segments compatible with ``transcribe.transcribe_vocals``'s
post-processing pipeline (wav2vec2 forced alignment + interpolation), so the
caller can swap engines without further changes downstream.
"""

import os
import subprocess
import tempfile

from whisper_compat import progress, is_oom, free_gpu

PARAKEET_LANGS = {
    "bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "hu",
    "it", "lv", "lt", "mt", "pl", "pt", "ro", "sk", "sl", "es", "sv", "uk", "ru",
}

NEMO_MODEL_ID = "nvidia/parakeet-tdt-0.6b-v3"
ONNX_MODEL_ID = "istupakov/parakeet-tdt-0.6b-v3-onnx"

_nemo_model = None
_onnx_model = None


class ParakeetEmptyOutputError(RuntimeError):
    """Raised when every Parakeet backend produced no usable words.

    Callers should treat this as a signal to fall back to Whisper for the song.
    """


def is_supported(lang: str) -> bool:
    if not lang:
        return False
    return lang.lower() in PARAKEET_LANGS


def free_models():
    """Drop cached Parakeet handles to free RAM/VRAM."""
    global _nemo_model, _onnx_model
    if _nemo_model is not None:
        del _nemo_model
        _nemo_model = None
    if _onnx_model is not None:
        del _onnx_model
        _onnx_model = None


def transcribe(
    vocals_path: str,
    device: str,
    language: str,
    batch_size: int = 8,
    pre_load_cleanup=None,
) -> tuple[list[dict], str]:
    """Run Parakeet ASR over the full vocals file and return word-level data.

    Returns ``(words, backend)`` where ``words`` is a list of
    ``{"word": str, "start": float, "end": float}`` entries with timestamps
    already on the original timeline (Parakeet sees the untrimmed vocals file
    so no offset is needed). The caller is expected to skip wav2vec2 forced
    alignment and build segments directly from these timestamps.

    ``batch_size`` controls NeMo's internal chunk batching (ignored by the
    ONNX backend, which processes the file in a single pass).

    ``pre_load_cleanup`` is a callable that frees other models (Whisper,
    wav2vec2 align, etc.) before each backend load attempt — required so the
    NeMo path on small GPUs doesn't OOM on weights mapping or CUDA stream init.
    """
    if pre_load_cleanup:
        pre_load_cleanup()

    use_cuda = device == "cuda"
    if use_cuda:
        words, backend = _transcribe_nemo_with_fallback(vocals_path, batch_size, pre_load_cleanup)
    else:
        progress(60, "Transcribing with Parakeet v3 (onnx)...")
        words = _transcribe_onnx(vocals_path)
        backend = "onnx"

    for w in words:
        w["start"] = round(w["start"], 3)
        w["end"] = round(w["end"], 3)

    print(
        f"[nightingale:LOG] Parakeet ({backend}) produced {len(words)} words for lang='{language}'",
        flush=True,
    )

    if not words:
        raise ParakeetEmptyOutputError(
            f"Parakeet backend '{backend}' produced no words for lang='{language}'"
        )

    return words, backend


def _transcribe_nemo_with_fallback(
    vocals_path: str, batch_size: int, pre_load_cleanup=None,
) -> tuple[list[dict], str]:
    """Try NeMo on CUDA; retry once after cleanup; finally fall back to ONNX on CPU."""
    current_batch = max(1, batch_size)
    for attempt in (1, 2):
        progress(60, f"Transcribing with Parakeet v3 (nemo, attempt {attempt}, batch={current_batch})...")
        try:
            return _transcribe_nemo(vocals_path, current_batch), "nemo"
        except Exception as e:
            if not is_oom(e):
                raise
            print(
                f"[nightingale:LOG] Parakeet NeMo OOM on attempt {attempt} (batch={current_batch}); "
                f"freeing models and retrying",
                flush=True,
            )
            free_models()
            if pre_load_cleanup:
                pre_load_cleanup()
            free_gpu()
            current_batch = max(1, current_batch // 2)

    print(
        "[nightingale:LOG] Parakeet NeMo OOM'd twice; falling back to ONNX on CPU",
        flush=True,
    )
    progress(60, "Transcribing with Parakeet v3 (onnx, CPU fallback)...")
    free_models()
    free_gpu()
    return _transcribe_onnx(vocals_path), "onnx-cpu-fallback"


def _ensure_wav_16k_mono(src_path: str, work_dir: str) -> str:
    """Re-encode ``src_path`` to a 16kHz mono PCM WAV inside ``work_dir``."""
    out = os.path.join(work_dir, "parakeet_input.wav")
    ffmpeg = os.environ.get("FFMPEG_PATH", "ffmpeg")
    subprocess.run(
        [ffmpeg, "-y", "-i", src_path, "-ar", "16000", "-ac", "1", "-v", "error", out],
        check=True,
    )
    return out


def _load_nemo():
    global _nemo_model
    if _nemo_model is not None:
        return _nemo_model
    import nemo.collections.asr as nemo_asr

    print(f"[nightingale:LOG] Loading NeMo model {NEMO_MODEL_ID}", flush=True)
    _nemo_model = nemo_asr.models.ASRModel.from_pretrained(model_name=NEMO_MODEL_ID)
    _nemo_model.eval()
    _nemo_model.cuda()
    return _nemo_model


def _transcribe_nemo(vocals_path: str, batch_size: int = 8) -> list[dict]:
    model = _load_nemo()

    with tempfile.TemporaryDirectory(prefix="nightingale_parakeet_") as work_dir:
        wav_path = _ensure_wav_16k_mono(vocals_path, work_dir)
        outputs = model.transcribe([wav_path], timestamps=True, batch_size=batch_size)

    if not outputs:
        return []
    out = outputs[0]
    timestamp = getattr(out, "timestamp", None) or {}
    word_entries = timestamp.get("word") or []

    words: list[dict] = []
    for entry in word_entries:
        text = (entry.get("word") or entry.get("token") or "").strip()
        start = entry.get("start")
        end = entry.get("end")
        if not text or start is None or end is None:
            continue
        words.append({"word": text, "start": float(start), "end": float(end)})
    return words


def _load_onnx():
    global _onnx_model
    if _onnx_model is not None:
        return _onnx_model
    import onnx_asr

    print(f"[nightingale:LOG] Loading ONNX model {ONNX_MODEL_ID} (CPU)", flush=True)
    cpu_providers = ["CPUExecutionProvider"]
    try:
        _onnx_model = onnx_asr.load_model(
            ONNX_MODEL_ID, quantization="int8", providers=cpu_providers,
        )
    except TypeError:
        try:
            _onnx_model = onnx_asr.load_model(ONNX_MODEL_ID, providers=cpu_providers)
        except TypeError:
            _onnx_model = onnx_asr.load_model(ONNX_MODEL_ID)
    return _onnx_model


def _transcribe_onnx(vocals_path: str) -> list[dict]:
    model = _load_onnx()

    with tempfile.TemporaryDirectory(prefix="nightingale_parakeet_") as work_dir:
        wav_path = _ensure_wav_16k_mono(vocals_path, work_dir)
        result = model.recognize(wav_path, timestamps=True)

    return _extract_words_from_onnx_result(result)


def _extract_words_from_onnx_result(result) -> list[dict]:
    """Normalise the various shapes onnx-asr can return into a flat word list."""
    candidates = result if isinstance(result, list) else [result]
    words: list[dict] = []
    for cand in candidates:
        if isinstance(cand, dict):
            ws = (
                cand.get("word_timestamps")
                or cand.get("words")
                or cand.get("timestamps")
                or []
            )
        else:
            ws = (
                getattr(cand, "word_timestamps", None)
                or getattr(cand, "words", None)
                or getattr(cand, "timestamps", None)
                or []
            )
        for w in ws or []:
            if isinstance(w, dict):
                text = (w.get("word") or w.get("text") or w.get("token") or "").strip()
                start = w.get("start")
                end = w.get("end")
            else:
                text = str(
                    getattr(w, "word", "") or getattr(w, "text", "") or getattr(w, "token", "")
                ).strip()
                start = getattr(w, "start", None)
                end = getattr(w, "end", None)
            if not text or start is None or end is None:
                continue
            words.append({"word": text, "start": float(start), "end": float(end)})
    return words
