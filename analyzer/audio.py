"""Audio analysis utilities: vocal region detection, silence splitting, normalization."""

import numpy as np


def detect_vocal_region(audio, sr: int = 16000, win_secs: float = 0.5,
                        threshold_pct: float = 0.15, min_consecutive: int = 4,
                        padding: float = 1.0) -> tuple[float, float]:
    """Detect where vocals start and end using RMS energy analysis.

    Returns (vocal_start, vocal_end) in seconds.
    """
    window_samples = int(win_secs * sr)
    rms_values = []
    for start_idx in range(0, len(audio), window_samples):
        chunk = audio[start_idx : start_idx + window_samples]
        rms_values.append(float(np.sqrt(np.mean(chunk ** 2))))

    duration_secs = len(audio) / sr

    if not rms_values:
        return 0.0, duration_secs

    peak_rms = max(rms_values)
    threshold = peak_rms * threshold_pct
    active = [rms >= threshold for rms in rms_values]
    active_count = sum(active)
    print(f"[nightingale:LOG] Vocal detection: peak_rms={peak_rms:.5f}, threshold({threshold_pct*100:.0f}%)={threshold:.5f}, windows={len(rms_values)}, active={active_count}", flush=True)

    top_windows = sorted(enumerate(rms_values), key=lambda x: x[1], reverse=True)[:10]
    for rank, (idx, rms) in enumerate(top_windows):
        t = idx * win_secs
        print(f"[nightingale:LOG]   RMS top {rank+1}: t={t:.1f}s rms={rms:.5f} {'<<ACTIVE>>' if active[idx] else ''}", flush=True)

    vocal_start_win = 0
    for i in range(len(active) - min_consecutive + 1):
        if all(active[i : i + min_consecutive]):
            vocal_start_win = i
            break

    vocal_end_win = len(rms_values) - 1
    for i in range(len(active) - 1, min_consecutive - 2, -1):
        start_check = max(i - min_consecutive + 1, 0)
        if all(active[start_check : start_check + min_consecutive]):
            vocal_end_win = i
            break

    vocal_start = vocal_start_win * win_secs
    vocal_end = min((vocal_end_win + 1) * win_secs, duration_secs)
    print(f"[nightingale:LOG] Sustained activity (>={min_consecutive} consecutive): first at win={vocal_start_win} ({vocal_start:.1f}s), last at win={vocal_end_win} ({vocal_end:.1f}s)", flush=True)

    vocal_start = max(vocal_start - padding, 0.0)
    vocal_end = min(vocal_end + padding, duration_secs)
    print(f"[nightingale:LOG] Vocal region (with {padding}s padding): {vocal_start:.1f}s - {vocal_end:.1f}s (song duration: {duration_secs:.1f}s)", flush=True)

    return vocal_start, vocal_end


def find_vocal_gaps(audio, sr: int = 16000, min_silence: float = 0.3,
                    target_chunk: float = 15.0) -> list[tuple[int, int]]:
    """Split audio into chunks at low-vocal regions, targeting ~target_chunk second chunks.

    Returns list of (start_sample, end_sample) tuples.
    """
    frame_len = int(min_silence * sr)
    hop = frame_len // 2
    n_frames = (len(audio) - frame_len) // hop + 1

    rms_values = []
    frame_positions = []
    for i in range(n_frames):
        start = i * hop
        frame = audio[start:start + frame_len].astype(np.float64)
        rms = float(np.sqrt(np.mean(frame ** 2)))
        rms_values.append(rms)
        frame_positions.append(start + frame_len // 2)

    peak_rms = max(rms_values) if rms_values else 1.0
    threshold = peak_rms * 0.1
    quiet_frames = [pos for rms, pos in zip(rms_values, frame_positions) if rms < threshold]

    print(f"[nightingale:LOG] Vocal gap detection: {len(quiet_frames)}/{len(rms_values)} quiet frames (peak_rms={peak_rms:.5f}, threshold=10%={threshold:.5f}, target={target_chunk}s)", flush=True)

    if not quiet_frames:
        max_samples = int(target_chunk * sr)
        chunks = []
        pos = 0
        while pos < len(audio):
            end = min(pos + max_samples, len(audio))
            chunks.append((pos, end))
            pos = end
        return chunks

    target_samples = int(target_chunk * sr)
    splits = []
    last_split = 0
    for sf in quiet_frames:
        if sf - last_split >= target_samples:
            splits.append(sf)
            last_split = sf

    chunks = []
    prev = 0
    for sp in splits:
        chunks.append((prev, sp))
        prev = sp
    chunks.append((prev, len(audio)))

    max_samples = int(target_chunk * sr)
    final = []
    for s, e in chunks:
        if e <= s:
            continue
        while e - s > max_samples:
            final.append((s, s + max_samples))
            s = s + max_samples
        if e > s:
            final.append((s, e))

    return final


def highpass_filter(audio, sr: int = 16000, cutoff_hz: float = 80.0):
    """Apply a simple highpass filter to remove sub-bass rumble from stems."""
    from scipy.signal import butter, sosfilt
    sos = butter(5, cutoff_hz, btype="high", fs=sr, output="sos")
    filtered = sosfilt(sos, audio.astype(np.float64)).astype(audio.dtype)
    print(f"[nightingale:LOG] Applied highpass filter at {cutoff_hz}Hz", flush=True)
    return filtered


def normalize_rms(audio, target_rms: float = 0.1, max_gain: float = 10.0):
    """Boost audio volume to a target RMS level. Returns normalized audio."""
    raw_rms = float(np.sqrt(np.mean(audio.astype(np.float64) ** 2)))
    if raw_rms > 0:
        gain = min(target_rms / raw_rms, max_gain)
        audio = (audio * gain).astype(audio.dtype)
        print(f"[nightingale:LOG] Normalized vocals: rms {raw_rms:.5f} -> {target_rms} (gain {gain:.2f}x)", flush=True)
    return audio
