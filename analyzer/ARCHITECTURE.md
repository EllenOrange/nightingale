# Analyzer Architecture

## File Structure

```
analyzer/
├── analyze.py          CLI entry point & orchestrator
├── stems.py            Demucs stem separation
├── transcribe.py       WhisperX full-audio transcription (generated mode)
├── align.py            LRCLIB lyrics alignment (lyrics mode)
├── audio.py            Audio utilities (vocal detection, normalization)
├── language.py         Multi-window language detection
├── hallucination.py    Hallucination/attribution word filtering
└── whisper_compat.py   PyTorch compatibility, device detection, progress helper
```

## Pipeline Flow

```mermaid
flowchart TD
    CLI["analyze.py<br/>Parse args, check cache, detect device"]

    CLI --> STEMS

    subgraph STEMS["Stem Separation (stems.py)"]
        S1[Load Demucs htdemucs model] --> S2[Load audio file]
        S2 --> S3[Apply model — separate sources]
        S3 --> S4["Save vocals.wav + instrumental.wav"]
    end

    STEMS --> BRANCH{LRCLIB lyrics<br/>JSON available?}

    BRANCH -- YES --> ALIGN
    BRANCH -- NO --> TRANSCRIBE

    subgraph TRANSCRIBE["Transcription Mode (transcribe.py)"]
        direction TB
        T1["Load vocals audio"] --> T2["Detect vocal region (RMS)<br/>(audio.py)"]
        T2 --> T3["Trim to vocal region"]
        T3 --> T4["Normalize RMS → 0.1"]
        T4 --> T5["Detect language<br/>multi-window voting<br/>(language.py)"]
        T5 --> T6["Load WhisperX model<br/>(VAD disabled)"]
        T6 --> T7["Transcribe full audio<br/>in one pass"]
        T7 --> T8["Offset timestamps<br/>to original timeline"]
        T8 --> T9["Filter hallucinations<br/>(hallucination.py)"]
        T9 --> T10["Forced alignment<br/>(WhisperX align)"]
        T10 --> T11["Recover dropped words"]
        T11 --> T12["Interpolate unaligned words"]
        T12 --> T13["Remove word-level<br/>hallucinations"]
        T13 --> T14["Build display segments<br/>(gaps, punctuation, splitting)"]
    end

    subgraph ALIGN["Lyrics Alignment Mode (align.py)"]
        direction TB
        A1["Load lyrics JSON<br/>(plain text lines)"] --> A2["Load vocals audio"]
        A2 --> A3["Detect vocal region (RMS)<br/>(audio.py)"]
        A3 --> A4["Detect language<br/>(language.py)"]
        A4 --> A5["Probe start offset<br/>(up to 8 attempts)"]

        subgraph PROBE["Start Offset Probing"]
            P1["Build single segment<br/>(all text, offset→end)"]
            P1 --> P2["Run whisperx.align()"]
            P2 --> P3["Score first word confidence"]
            P3 --> P4{"Score ≥ 0.9?"}
            P4 -- YES --> P5["Accept offset"]
            P4 -- NO --> P6["Step = first_word.end − 3s"]
            P6 --> P1
        end

        A5 --> PROBE
        PROBE --> A6["Final alignment<br/>from best offset"]
        A6 --> A7["Map aligned words<br/>back to lyric lines"]
        A7 --> A8["Interpolate missing<br/>word timestamps"]
        A8 --> A9["Split lines > 10 words"]
    end

    TRANSCRIBE --> OUTPUT
    ALIGN --> OUTPUT

    OUTPUT["Write transcript JSON<br/>Clean up temp files"]
```

## Output Format

```json
{
  "language": "en",
  "source": "lyrics | generated",
  "segments": [
    {
      "text": "Something ugly this way comes",
      "start": 46.005,
      "end": 48.465,
      "words": [
        { "word": "Something", "start": 46.005, "end": 46.765, "score": 0.757 }
      ]
    }
  ]
}
```

## Module Dependencies

```mermaid
graph LR
    A[analyze.py] --> WC[whisper_compat.py]
    A --> ST[stems.py]
    A --> AL[align.py]
    A --> TR[transcribe.py]

    ST --> WC
    AL --> AU[audio.py]
    AL --> LA[language.py]
    AL --> WC
    TR --> AU
    TR --> HA[hallucination.py]
    TR --> LA
    TR --> WC
```

## Key Design Decisions

**Single-pass transcription** — No audio chunking. The full vocal region is
transcribed in one WhisperX pass with VAD disabled. This avoids word-splitting
at chunk boundaries and gives Whisper maximum context.

**Vocal region trimming** — Before transcription, RMS energy analysis detects
where vocals actually start/end. The audio is trimmed to this region, preventing
Whisper from hallucinating on silent intros/outros. Timestamps are offset back
to the original timeline after transcription.

**Single-segment alignment** — For lyrics mode, all text is passed as one
concatenated segment spanning the full vocal region, letting the aligner freely
find word positions without artificial time constraints.

**Start offset probing** — Up to 8 alignment attempts with different start
offsets, scored by the first word's confidence. The step between probes is
derived from where the aligner placed the first word (`end - 3s`), converging
on the actual vocal start.

**Vocal region detection** — RMS energy on the Demucs vocals stem. Requires
4 consecutive active windows (2s sustained) at 15% of peak RMS to avoid
triggering on backing vocal bleed.
