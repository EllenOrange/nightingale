# Lyrics & Transcription

Nightingale provides word-level synchronized lyrics through two sources.

## LRCLIB

[LRCLIB](https://lrclib.net) is queried first for existing synced lyrics. When a match is found, lyrics are used directly without needing transcription. This is faster and often more accurate for well-known songs.

## WhisperX Transcription

When LRCLIB doesn't have lyrics for a song, Nightingale uses [WhisperX](https://github.com/m-bain/whisperX) with the large-v3 model to:

1. **Transcribe** the isolated vocals into text
2. **Align** each word to precise timestamps in the audio

This produces word-level timing information that drives the karaoke highlighting during playback.

## Language Support

WhisperX supports a wide range of languages. The language is auto-detected from the audio. Nightingale includes CJK font support (Noto Sans CJK) for Chinese, Japanese, and Korean lyrics.

## Highlighting

During playback, lyrics are displayed with word-by-word highlighting:

- **Current word** — highlighted in the accent color
- **Sung words** — shown in a completed state
- **Upcoming words** — shown in a dimmer color
- **Next line** — previewed below the current line
