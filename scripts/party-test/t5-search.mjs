/**
 * T5 (automated integration): online song search endpoints.
 *
 * Covers the two external-facing steps of the "not in my library" flow:
 * LRCLIB free-text search and YouTube candidate listing. Does NOT enqueue (that
 * would start a real multi-minute download); the enqueue-with-canonical-
 * metadata path is a thin extension of party_queue_add covered by T3.
 *
 * Requires Node 24+ and network access. Usage: node scripts/party-test/t5-search.mjs [baseUrl]
 */

const BASE = process.argv[2] ?? "http://127.0.0.1:8080";

class TestError extends Error {}
const fail = (msg) => {
  throw new TestError(msg);
};

const post = async (name, body) => {
  const res = await fetch(`${BASE}/api/cmd/${name}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body ?? {}),
  });
  if (!res.ok) fail(`${name} -> HTTP ${res.status}: ${await res.text().catch(() => "")}`);
  return res.json();
};

const main = async () => {
  // LRCLIB: a well-known song must return lyric-available matches.
  const tracks = await post("party_search_lrclib", { query: "bohemian rhapsody queen" });
  if (!Array.isArray(tracks) || tracks.length === 0) fail("LRCLIB returned no matches");
  for (const t of tracks) {
    if (!t.has_plain && !t.has_synced) fail(`LRCLIB match without lyrics: ${JSON.stringify(t)}`);
    if (!t.track_name || !t.artist_name) fail(`LRCLIB match missing name: ${JSON.stringify(t)}`);
  }
  const queenHit = tracks.some((t) => /queen/i.test(t.artist_name));
  if (!queenHit) fail("expected a Queen match in LRCLIB results");
  console.log(`ok: LRCLIB returned ${tracks.length} lyric-available matches`);

  // Empty query is handled gracefully (no crash, empty list).
  const empty = await post("party_search_lrclib", { query: "   " });
  if (!Array.isArray(empty) || empty.length !== 0) fail("empty LRCLIB query should return []");
  console.log("ok: empty LRCLIB query returns []");

  // YouTube: candidates come back with usable fields.
  const videos = await post("party_youtube_candidates", {
    query: "Queen Bohemian Rhapsody official",
    limit: 5,
  });
  if (!Array.isArray(videos) || videos.length === 0) fail("no YouTube candidates");
  for (const v of videos) {
    if (!v.videoId) fail(`candidate missing videoId: ${JSON.stringify(v)}`);
    if (!/^https:\/\/www\.youtube\.com\/watch\?v=/.test(v.url)) fail(`bad url: ${v.url}`);
    if (!/^https:\/\/i\.ytimg\.com\/vi\//.test(v.thumbnail)) fail(`bad thumbnail: ${v.thumbnail}`);
    if (typeof v.title !== "string") fail("candidate missing title");
  }
  console.log(`ok: YouTube returned ${videos.length} candidates with url + thumbnail`);

  // UTF-8: yt-dlp output must not be mangled (en dash etc). If the official
  // Queen video is present it carries a non-ASCII dash; assert no replacement
  // character leaked through when it does.
  const mangled = videos.find((v) => v.title.includes("�"));
  if (mangled) fail(`title has a replacement char (encoding bug): ${mangled.title}`);
  console.log("ok: candidate titles are valid UTF-8 (no mojibake)");

  console.log("PASS: t5-search");
};

main().then(
  () => {
    process.exitCode = 0;
  },
  (e) => {
    console.error(e instanceof TestError ? `FAIL: ${e.message}` : String(e));
    process.exitCode = 1;
  },
);
