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

## Language Support

The language is auto-detected from the audio. You can override it per song from the song-list controls. Nightingale includes Noto Sans CJK fonts for Chinese, Japanese, and Korean lyrics.

## CJK Languages

Japanese (`ja`) and Chinese (`zh`) take a dedicated forced-alignment path because their wav2vec2 alignment models are character-level CTC checkpoints, not word/space-segmented. Nightingale:

1. **Transcribes** with Whisper or Parakeet as usual.
2. **Cleans** the text down to the alignment vocab (drops punctuation and other out-of-vocab symbols).
3. **Aligns** per character with a wav2vec2 model:
   - Japanese: `vumichien/wav2vec2-large-xlsr-japanese-hiragana` — feeds [fugashi](https://github.com/polm/fugashi)-derived hiragana readings into a hiragana-vocab CTC model. This sidesteps the dense kanji vocabulary of the default checkpoint and matches the acoustic prior of natural Japanese speech far better.
   - Chinese: `jonatasgrosman/wav2vec2-large-xlsr-53-chinese-zh-cn` with [jieba](https://github.com/fxsjy/jieba) tokenization for display.
4. **Reattributes** per-character timing back onto fugashi (ja) or jieba (zh) tokens for word-level highlighting.

Korean (`ko`) uses `kresnik/wav2vec2-large-xlsr-korean`, which is already eojeol-segmented and bypasses the character-retokenization step.

For all three languages, every word is annotated with a romanized **reading** that appears above the original token during playback:

- Japanese — Hepburn romaji via [pykakasi](https://github.com/miurahr/pykakasi)
- Chinese — tone-mark pinyin via [pypinyin](https://github.com/mozillazg/python-pinyin)
- Korean — academic Revised Romanization via [hangul-romanize](https://github.com/youknowone/hangul-romanize)

Heavy CJK modules are imported lazily on first use, so non-CJK songs don't pay the startup cost.

## Highlighting

During playback, lyrics are displayed with word-by-word highlighting:

- **Current word** — highlighted in the accent color
- **Sung words** — shown in a completed state
- **Upcoming words** — shown in a dimmer color
- **Next line** — previewed below the current line
- **Reading** — for CJK songs, the romanized reading is shown above each token in a smaller weight
