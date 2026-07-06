# Lyrics & Transcription

Nightingale provides word-level synchronized lyrics through two sources.

## LRCLIB

[LRCLIB](https://lrclib.net) is queried first for existing synced lyrics. When a match is found, lyrics are used directly without needing transcription. This is faster and often more accurate for well-known songs.

## WhisperX Transcription

When LRCLIB doesn't have lyrics for a song, Nightingale runs ASR over the isolated vocals to:

1. **Transcribe** the audio into text
2. **Align** each word to precise timestamps

This produces word-level timing information that drives the karaoke highlighting during playback.

## Choosing the ASR Engine

Two ASR engines are available, switchable from **Settings → Analysis**:

### Whisper (default)

Uses [WhisperX](https://github.com/m-bain/whisperX) with the `large-v3` model. Broad language coverage, robust on noisy or multilingual material. This is the default and the recommended choice for most users.

### Parakeet v3 (experimental)

[Parakeet TDT 0.6B v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) by NVIDIA. Smaller and noticeably faster than Whisper large-v3. Two interchangeable backends are picked automatically based on your runtime device:

- **CUDA** → NVIDIA NeMo (`nemo_toolkit[asr]`)
- **CPU / MPS** → ONNX Runtime via `onnx-asr`

Parakeet is supported for the following 25 European languages: Bulgarian, Croatian, Czech, Danish, Dutch, English, Estonian, Finnish, French, German, Greek, Hungarian, Italian, Latvian, Lithuanian, Maltese, Polish, Portuguese, Romanian, Russian, Slovak, Slovenian, Spanish, Swedish, Ukrainian.

If Parakeet is selected for an unsupported language, or it produces no usable words for a song, Nightingale falls back to Whisper for that file. Word-level alignment after Parakeet still uses wav2vec2 forced alignment, so timing accuracy is comparable.

## Choosing the Forced-Alignment Backend

Whenever word timestamps are derived from wav2vec2 forced alignment — that is, the Whisper transcription path and the LRCLIB lyrics-alignment path — you can pick how the alignment itself is computed from **Settings → Analysis → Forced alignment**:

### WhisperX (default)

WhisperX's built-in aligner. Emissions come from wav2vec2, then a Viterbi decode + backtrack runs in pure Python on the CPU (even when a GPU is present). Reliable and well-tested; this is the default.

### GPU forced alignment (experimental)

Replaces only the Viterbi core with [`torchaudio.functional.forced_align`](https://docs.pytorch.org/audio/stable/generated/torchaudio.functional.forced_align.html) — a C++/CUDA CTC alignment kernel — while keeping everything else identical (model, dictionary, wildcard handling, character/word/sentence assembly, and the CJK per-character path). It runs on CUDA GPUs and, on Apple Silicon, on the optimized CPU kernel (torchaudio has no MPS kernel), which is still far faster than WhisperX's Python decode. It also speeds up LRCLIB lyrics alignment.

If the kernel fails for a segment (for example the audio is too short for the number of characters) it resorts to that segment's original bounds, and if the backend errors it automatically falls back to WhisperX — so switching it on is safe. This backend does not affect the Parakeet native-timestamp path, which skips forced alignment entirely.

### Qwen aligner (experimental)

Replaces wav2vec2 forced alignment entirely with [Qwen3-ForcedAligner-0.6B](https://huggingface.co/Qwen/Qwen3-ForcedAligner-0.6B-hf) (Apache-2.0), a non-autoregressive model that predicts a start/end timestamp for every token in a single forward pass from the audio and transcript together — no CTC, no phonetic conversion. It tokenizes the display text itself (Japanese via [nagisa](https://github.com/taishi-i/nagisa), Korean via [soynlp](https://github.com/lovit/soynlp), Chinese per character, space-delimited words otherwise) and drops punctuation, so Nightingale only attaches the romanized reading on top; the elaborate wav2vec2 CJK reattribution path is not needed here.

Notes:

- **Languages** — supports 11: Chinese, English, Cantonese, French, German, Italian, Japanese, Korean, Portuguese, Russian, Spanish. Any other language automatically falls back to the wav2vec2 path.
- **Devices** — runs on CUDA (bf16), and unlike the other backends also on Apple Silicon **MPS** (falling back to CPU only where an op is unsupported).
- **Length** — the model handles up to ~5 minutes of audio per pass. The Whisper path aligns each segment against its own slice, so full songs are fine; a single over-long lyrics pass falls back to wav2vec2.
- **Safety** — any failure (unsupported language, over-length audio, OOM after a CPU retry, load error) falls back to WhisperX. The model weights (~1.8 GB) download on first use. Does not affect the Parakeet native-timestamp path.

## Language Support

The language is auto-detected from the audio. You can override it per song from the song-list controls. Nightingale includes Noto Sans CJK fonts for Chinese, Cantonese, Japanese, and Korean lyrics.

## CJK Languages

Japanese (`ja`), Chinese (`zh`), and Cantonese (`yue`) take a dedicated forced-alignment path because their wav2vec2 alignment models are character-level CTC checkpoints, not word/space-segmented. Nightingale:

1. **Transcribes** with Whisper or Parakeet as usual.
2. **Cleans** the text down to the alignment vocab (drops punctuation and other out-of-vocab symbols).
3. **Aligns** per character with a wav2vec2 model:
   - Japanese: `vumichien/wav2vec2-large-xlsr-japanese-hiragana` — feeds [fugashi](https://github.com/polm/fugashi)-derived hiragana readings into a hiragana-vocab CTC model. This sidesteps the dense kanji vocabulary of the default checkpoint and matches the acoustic prior of natural Japanese speech far better.
   - Chinese and Cantonese: `jonatasgrosman/wav2vec2-large-xlsr-53-chinese-zh-cn` with [jieba](https://github.com/fxsjy/jieba) tokenization for display. Cantonese is written in the same Han characters, so it reuses the Chinese CTC model and per-character grouping; only its romanized reading differs.
4. **Reattributes** per-character timing back onto fugashi (ja) or jieba (zh/yue) tokens for word-level highlighting.

Korean (`ko`) uses `kresnik/wav2vec2-large-xlsr-korean`, which is already eojeol-segmented and bypasses the character-retokenization step.

For all four languages, every word is annotated with a romanized **reading** that appears above the original token during playback:

- Japanese — Hepburn romaji via [pykakasi](https://github.com/miurahr/pykakasi)
- Chinese — tone-mark pinyin via [pypinyin](https://github.com/mozillazg/python-pinyin)
- Cantonese — Jyutping via [ToJyutping](https://github.com/CanCLID/ToJyutping)
- Korean — academic Revised Romanization via [hangul-romanize](https://github.com/youknowone/hangul-romanize)

Heavy CJK modules are imported lazily on first use, so non-CJK songs don't pay the startup cost.

## Editing Lyrics

When the automatic transcript needs a human pass, use a ready non-USDX song's **Actions** button and choose **Edit lyrics**. The dialog has two halves:

- **Edit tab** — a textarea seeded with the current transcript, one karaoke line per row. Saving re-runs alignment with your edits, so timing stays accurate to the audio. Dirty state is tracked; closing without saving discards the edit.
- **LRCLIB matches tab** — visible when [LRCLIB](https://lrclib.net) returns more than one candidate for the song's metadata. Each candidate shows its track / artist / album / duration and the lyric body; arrows above the card let you carousel through them, and **Use these** copies the candidate's lines into the editor. Save to commit them with realignment.

Saved lyrics replace the cached transcript for that song's blake3 hash, so subsequent plays pick up the edit immediately. CJK alignment paths are skipped on edits — the editor saves a flat per-line transcript and lets the alignment stage re-derive per-character timings on the next analyzer pass.

## Highlighting

During playback, lyrics are displayed with word-by-word highlighting:

- **Current word** — highlighted in the accent color
- **Sung words** — shown in a completed state
- **Upcoming words** — shown in a dimmer color
- **Next line** — previewed below the current line
- **Reading** — for CJK songs, the romanized reading is shown above each token in a smaller weight
