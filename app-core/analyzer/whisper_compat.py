"""PyTorch / device compatibility helpers for Nightingale analyzer."""

import torch

from gpu import hard_free_gpu, log_vram, reset_peak_stats, vram_snapshot, gpu_model

_original_torch_load = torch.load


def _patched_torch_load(*args, **kwargs):
    kwargs["weights_only"] = False
    return _original_torch_load(*args, **kwargs)


torch.load = _patched_torch_load


def _default_progress_sink(pct: int, msg: str):
    print(f"[nightingale:PROGRESS:{pct}] {msg}", flush=True)


_progress_sink = _default_progress_sink


def set_progress_sink(fn):
    global _progress_sink
    _progress_sink = fn or _default_progress_sink


def progress(pct: int, msg: str):
    _progress_sink(pct, msg)


def detect_device() -> str:
    if torch.cuda.is_available():
        return "cuda"
    if torch.backends.mps.is_available():
        return "mps"
    return "cpu"


def align_device_for(device: str) -> str:
    return "cpu" if device == "mps" else device


def compute_type_for(device: str) -> str:
    if device != "cuda":
        return "float32"
    major, _ = torch.cuda.get_device_capability()
    if major >= 7:
        return "float16"
    return "int8"


def is_oom(err):
    lower = str(err).lower()
    return "out of memory" in lower or "outofmemoryerror" in lower


def free_gpu():
    """Backwards-compatible alias for :func:`gpu.hard_free_gpu`."""
    hard_free_gpu()


def _run_align(raw_segments, audio, language, device):
    import whisperx
    with gpu_model(f"wav2vec2:{device}") as held:
        print(
            f"[nightingale:LOG] Loading align model for language='{language}' on device='{device}'",
            flush=True,
        )
        align_model, metadata = whisperx.load_align_model(
            language_code=language, device=device,
        )
        held.append(align_model)
        return whisperx.align(raw_segments, align_model, metadata, audio, device)


def align_with_fallback(raw_segments, audio, language, device, pre_align_cleanup=None):
    """Run whisperx.align with OOM fallback: retry after cleanup, then CPU."""
    try:
        return _run_align(raw_segments, audio, language, device)
    except Exception as e:
        if not is_oom(e):
            raise
        log_vram("oom:align_attempt1")

    if pre_align_cleanup:
        print(
            f"[nightingale:LOG] Alignment OOM, freeing other models and retrying on {device}",
            flush=True,
        )
        try:
            pre_align_cleanup()
        except Exception:
            pass
        try:
            return _run_align(raw_segments, audio, language, device)
        except Exception as e2:
            if not is_oom(e2):
                raise
            log_vram("oom:align_attempt2")

    print("[nightingale:LOG] Alignment OOM, falling back to CPU", flush=True)
    return _run_align(raw_segments, audio, language, "cpu")
