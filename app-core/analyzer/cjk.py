"""CJK (Japanese / Chinese / Korean) tokenization, romanization, and
char-timing reattribution helpers.

The wav2vec2 alignment models nightingale uses for ja/zh
(``jonatasgrosman/wav2vec2-large-xlsr-53-{japanese,chinese-zh-cn}``) are
character-level CTC checkpoints whose vocabs hold only kana+kanji / hanzi —
no punctuation, no romaji. ``clean_for_alignment`` mirrors their
training-time ``CHARS_TO_IGNORE`` so the aligner only sees in-vocab chars.
Per-character timing from those aligners is mapped onto fugashi (ja) or
jieba (zh) tokens for display.

Korean ("ko") uses ``kresnik/wav2vec2-large-xlsr-korean`` and is *not* in
WhisperX's ``LANGUAGES_WITHOUT_SPACES`` list, so its alignment output is
already per-eojeol (whitespace chunks) and bypasses the char-level
retokenization path entirely; only :func:`reading` is involved for ko.

Reading systems used: pykakasi Hepburn (ja), pypinyin tone-mark pinyin
(zh), hangul-romanize academic Revised Romanization (ko). All heavy
modules are imported lazily on first use so non-CJK songs don't pay the
fugashi/pykakasi/jieba/hangul-romanize startup cost.
"""

# Punctuation / symbols / whitespace not present in the wav2vec2 ja/zh
# vocabs. Concatenates the CHARS_TO_IGNORE lists from both model cards plus
# common kaomoji/lyric punctuation we've seen in LRClib payloads.
_NOISE_CHARS = (
    ",?¿.!¡;:\"%~`_+<>=…–—°´«»„“”'’/\\^"
    "。、，、；：！？「」『』【】〝〟〜〽‧～｛｝（）［］〈〉《》"
    "♪♫♬·・…‥─━‐‑‒–—―•※"
)
_NOISE_CHARS_SET = set(_NOISE_CHARS) | {" ", "\t", "\n", "\r", "\u3000"}

_fugashi_tagger = None
_pykakasi_instance = None
_jieba_inited = False
_korean_transliter = None


def is_cjk(lang) -> bool:
    """Languages that go through the char-level alignment + retokenize path."""
    return lang in ("ja", "zh")


def is_korean(lang) -> bool:
    return lang == "ko"


def is_supported_lang(lang) -> bool:
    """Any language for which we attach a romanized reading per token."""
    return is_cjk(lang) or is_korean(lang)


def _has_hangul(text: str) -> bool:
    for ch in text:
        c = ord(ch)
        if 0xAC00 <= c <= 0xD7AF:
            return True
        if 0x1100 <= c <= 0x11FF:
            return True
        if 0x3130 <= c <= 0x318F:
            return True
    return False


def clean_for_alignment(text: str) -> str:
    """Drop chars outside the wav2vec2 ja/zh model vocabulary."""
    if not text:
        return ""
    return "".join(ch for ch in text if ch not in _NOISE_CHARS_SET)


def _get_fugashi():
    global _fugashi_tagger
    if _fugashi_tagger is None:
        import fugashi
        _fugashi_tagger = fugashi.Tagger()
    return _fugashi_tagger


def _get_pykakasi():
    global _pykakasi_instance
    if _pykakasi_instance is None:
        import pykakasi
        _pykakasi_instance = pykakasi.kakasi()
    return _pykakasi_instance


def _ensure_jieba():
    global _jieba_inited
    if not _jieba_inited:
        import jieba
        jieba.initialize()
        _jieba_inited = True


def _get_korean_romanizer():
    global _korean_transliter
    if _korean_transliter is None:
        from hangul_romanize import Transliter
        from hangul_romanize.rule import academic
        _korean_transliter = Transliter(academic)
    return _korean_transliter


def tokenize_japanese(text: str) -> list[str]:
    tagger = _get_fugashi()
    out: list[str] = []
    for t in tagger(text):
        s = getattr(t, "surface", None) or str(t)
        if s:
            out.append(s)
    return out


def tokenize_chinese(text: str) -> list[str]:
    _ensure_jieba()
    import jieba
    return [t for t in jieba.lcut(text, cut_all=False) if t]


def tokenize_korean(text: str) -> list[str]:
    return text.split()


def tokenize(text: str, lang: str) -> list[str]:
    """Word/morpheme tokenization. Token concatenation equals ``text`` for
    ja/zh; for ko it returns whitespace-separated eojeol (concatenation
    equals ``text`` only after collapsing inter-word spaces)."""
    if not text:
        return []
    if lang == "ja":
        return tokenize_japanese(text)
    if lang == "zh":
        return tokenize_chinese(text)
    if lang == "ko":
        return tokenize_korean(text)
    return [text]


def reading(text: str, lang: str):
    """Romanized reading: pykakasi Hepburn (ja), tone-mark pinyin (zh),
    Revised Romanization (ko)."""
    if not text:
        return None
    if not clean_for_alignment(text):
        return None
    if lang == "ja":
        try:
            chunks = _get_pykakasi().convert(text)
            r = "".join(c.get("hepburn", "") for c in chunks).strip()
            return r or None
        except Exception:
            return None
    if lang == "zh":
        try:
            from pypinyin import pinyin, Style
            chunks = pinyin(text, style=Style.TONE, heteronym=False, errors="ignore")
            parts = [c[0] for c in chunks if c and c[0]]
            r = " ".join(parts).strip()
            return r or None
        except Exception:
            return None
    if lang == "ko":
        if not _has_hangul(text):
            return None
        try:
            r = _get_korean_romanizer().translit(text).strip()
            return r or None
        except Exception:
            return None
    return None


def attribute_chars_to_tokens(
    tokens: list[str],
    chars_with_ts: list[dict],
    fallback_start=None,
    fallback_end=None,
) -> list[dict]:
    """Map per-character timestamps onto tokens.

    ``chars_with_ts`` is the WhisperX char-level alignment output for the
    text obtained by ``clean_for_alignment("".join(tokens))``. Each token's
    timing window is taken from the first/last char it contains; tokens that
    consist purely of punctuation are emitted with ``_punct: True`` and no
    timestamps so the caller can fold them into a neighbour via
    :func:`merge_punct`.
    """
    cleaned_lengths = [len(clean_for_alignment(t)) for t in tokens]
    expected = sum(cleaned_lengths)
    actual = len(chars_with_ts)

    if expected != actual:
        ts_starts = [c.get("start") for c in chars_with_ts if c.get("start") is not None]
        ts_ends = [c.get("end") for c in chars_with_ts if c.get("end") is not None]
        seg_start = ts_starts[0] if ts_starts else fallback_start
        seg_end = ts_ends[-1] if ts_ends else fallback_end
        if seg_start is None:
            seg_start = 0.0
        if seg_end is None or seg_end <= seg_start:
            seg_end = seg_start + 0.1
        timed_tokens = max(1, sum(1 for length in cleaned_lengths if length > 0))
        step = (seg_end - seg_start) / timed_tokens
        out: list[dict] = []
        idx = 0
        for tok, length in zip(tokens, cleaned_lengths):
            if length == 0:
                out.append({"word": tok, "_punct": True})
                continue
            s = seg_start + idx * step
            e = seg_start + (idx + 1) * step
            out.append({"word": tok, "start": s, "end": e, "estimated": True})
            idx += 1
        return out

    out: list[dict] = []
    cursor = 0
    for tok, length in zip(tokens, cleaned_lengths):
        if length == 0:
            out.append({"word": tok, "_punct": True})
            continue
        slice_chars = chars_with_ts[cursor:cursor + length]
        cursor += length
        ts_starts = [c.get("start") for c in slice_chars if c.get("start") is not None]
        ts_ends = [c.get("end") for c in slice_chars if c.get("end") is not None]
        scores = [c.get("score") for c in slice_chars if c.get("score") is not None]

        entry: dict = {"word": tok}
        if ts_starts and ts_ends:
            entry["start"] = ts_starts[0]
            entry["end"] = ts_ends[-1]
            if scores:
                entry["score"] = sum(scores) / len(scores)
        else:
            entry["estimated"] = True
        out.append(entry)
    return out


def merge_punct(entries: list[dict]) -> list[dict]:
    """Fold punctuation-only tokens into the adjacent timed token's text.

    Glues trailing punctuation onto the previous word, leading punctuation
    onto the following word. The displayable on-screen ``word`` keeps the
    original punctuation; timing/reading stay with the timed token.
    """
    out: list[dict] = []
    pending_prefix: list[str] = []
    for e in entries:
        if e.get("_punct"):
            if out:
                out[-1]["word"] = out[-1]["word"] + e["word"]
            else:
                pending_prefix.append(e["word"])
            continue
        cleaned = {k: v for k, v in e.items() if k != "_punct"}
        if pending_prefix:
            cleaned["word"] = "".join(pending_prefix) + cleaned["word"]
            pending_prefix = []
        out.append(cleaned)
    if pending_prefix and out:
        out[-1]["word"] = out[-1]["word"] + "".join(pending_prefix)
    return out


def attach_reading(entries: list[dict], lang: str) -> None:
    """Attach a ``reading`` field to each entry that has displayable text."""
    for e in entries:
        if "word" not in e:
            continue
        r = reading(e["word"], lang)
        if r:
            e["reading"] = r
