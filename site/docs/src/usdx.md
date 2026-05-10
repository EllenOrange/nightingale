# UltraStar Deluxe

Nightingale can play [UltraStar Deluxe](https://usdx.eu/) songs directly from your library, using the pitch and lyric data shipped with each song instead of running its own analyzer. This support is **experimental**.

## How songs are detected

The library scanner picks up two kinds of USDX files:

- `*.usdx` files — always treated as USDX.
- `*.txt` files — only when the file's contents look like a USDX header (so unrelated `.txt` notes in your music folder are ignored).

Sibling media referenced from the file (`#AUDIO`, `#MP3`, `#VOCALS`, `#INSTRUMENTAL`, `#VIDEO`) are de-duped from the scan, so they don't appear as standalone tracks alongside the USDX song.

## Folder layout

A typical USDX bundle keeps every file in one folder:

```text
my-songs/
└── Some Artist - Some Song/
    ├── song.txt
    ├── song.mp3        # referenced via #AUDIO / #MP3
    ├── song.vocals.mp3 # referenced via #VOCALS  (optional)
    ├── song.instr.mp3  # referenced via #INSTRUMENTAL (optional)
    ├── song.mp4        # referenced via #VIDEO (optional)
    └── cover.jpg       # referenced via #COVER (optional)
```

All sibling paths are resolved relative to the folder containing the `.txt`. If a referenced file is missing, the corresponding feature is skipped (e.g. no `#VIDEO` means no source-video background).

## Supported tags

Nightingale reads the following header tags. Anything else is ignored:

| Tag | Required | Notes |
|---|---|---|
| `#TITLE` | yes | Song title. |
| `#ARTIST` | yes | Artist name. |
| `#AUDIO` | yes* | Main audio file. `#MP3` is accepted as a v1.0-style fallback when `#AUDIO` is absent. |
| `#BPM` | yes | UltraStar quarter-note BPM. Comma or period decimal separators are both accepted. |
| `#GAP` | no | Flat offset in milliseconds applied to every timestamp. |
| `#END` | no | Optional end-time hint in milliseconds. |
| `#RELATIVE` | no | When `yes`, line offsets accumulate via `-` separators. Both absolute and relative timing are supported. |
| `#VOCALS` | no | Pre-separated vocals stem. When present, stem separation is skipped. |
| `#INSTRUMENTAL` | no | Pre-separated instrumental. When present, stem separation is skipped. |
| `#VIDEO` | no | Source video to use as the playback background. |
| `#COVER` | no | Album/song cover image. |
| `#LANGUAGE` | no | First entry in a comma-separated list is used. |
| `#EDITION` | no | First entry in a comma-separated list is used. |

\*Either `#AUDIO` or `#MP3` must resolve to a real file.

## Timing model

UltraStar's `#BPM` counts **quarter-note beats per minute**, and a *note beat* is 1/4 of one of those — so `seconds_per_note_beat = 60 / (BPM * 4)`. `#GAP` is a flat millisecond offset added to every timestamp. This matches the formula used by UltraStar Deluxe, Vocaluxe, and Performous, so files authored against any of those tools should play in sync.

Note kinds (`:` regular, `*` golden, `F` freestyle, `R` / `G` rap) are recognised by the parser, but pitch-bonus weighting is currently uniform across all of them.

## How USDX songs flow through the pipeline

1. The `.txt` is parsed into a transcript JSON whose shape is identical to what the analyzer cache writes for non-USDX songs, so playback reuses the existing transcript and lyric path with no special-casing.
2. If `#VOCALS` and `#INSTRUMENTAL` are both provided, [stem separation](./stems.md) is **skipped** entirely. Otherwise, the configured separator runs on `#AUDIO` once and the result is cached like any other song.
3. If `#VIDEO` is set, it plays as the source background. The other shader and Pixabay backgrounds are still available with `T`.
4. Re-scanning the library does not re-parse unchanged USDX files; transcript synthesis is keyed off the file's blake3 hash like every other cache.

## Limitations

- USDX support is **experimental**. File a bug if you hit a song that doesn't parse.
- Medley sections are not yet supported.
- Duet voice tracks (`P1` / `P2`) are parsed as a single voice; per-singer scoring is not differentiated.
- Encodings beyond UTF-8 and the most common Latin code pages may fail to decode cleanly.
- Songs missing any of the required tags (`#TITLE`, `#ARTIST`, `#AUDIO`/`#MP3`, `#BPM`) are rejected at scan time.

## Where to find songs

- [usdb.animux.de](https://usdb.animux.de/) — large community song database.
- [usdx.eu/format](https://usdx.eu/format/) — the format reference, useful if you want to author your own files.
