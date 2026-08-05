/**
 * T2.2 + T2.3 (automated integration): YouTube ingest end to end.
 *
 * Drives a running server: issues `party_ingest`, watches `party.ingest`
 * events to completion, and asserts the song landed analyzed with real
 * metadata and LRCLIB-sourced lyrics. Then re-runs the same ingest and asserts
 * it is idempotent (no second analysis pass).
 *
 * Fixture: The Beatles - Her Majesty (Remastered 2009), a 26s official Topic
 * track with clean artist/title tags and confirmed LRCLIB coverage. Short on
 * purpose so a real analysis finishes fast.
 *
 * This runs a genuine download + Demucs + WhisperX pass, so it can take a few
 * minutes (longer on the first song of a server's life, cold model load).
 *
 * Requires Node 24+ (global WebSocket + fetch). No dependencies.
 *
 * Usage: node scripts/party-test/t2-ingest.mjs [baseUrl]
 */

import { promises as fs } from "node:fs";

const BASE = process.argv[2] ?? "http://127.0.0.1:8080";
const WS_URL = BASE.replace(/^http/, "ws") + "/ws";
const QUERY = "https://www.youtube.com/watch?v=Mh1hKt5kQ_4";
const TITLE_RE = /her majesty/i;
const READY_TIMEOUT_MS = 12 * 60 * 1000;
const IDEMPOTENT_TIMEOUT_MS = 3 * 60 * 1000;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

class TestError extends Error {}
const fail = (msg) => {
  throw new TestError(msg);
};

const parse = (data) => {
  try {
    return JSON.parse(data);
  } catch {
    return null;
  }
};

const post = async (name, body) => {
  const res = await fetch(`${BASE}/api/cmd/${name}`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body ?? {}),
  });
  if (!res.ok) fail(`${name} -> HTTP ${res.status}: ${await res.text().catch(() => "")}`);
  return res.status === 204 ? null : res.json().catch(() => null);
};

const emptyFilters = {
  artist: null,
  album: null,
  playlist: null,
  query: null,
  status: null,
  transcript_source: null,
  search: null,
};

const findFixtureSongs = async () => {
  const store = await post("load_songs", {
    params: { search: "Her Majesty", filters: emptyFilters, skip: 0, take: 100 },
  });
  return (store?.processed ?? []).filter((s) => TITLE_RE.test(s.title));
};

/**
 * Guarantee a clean slate so T2.2 exercises a real analysis: drop cache, delete
 * the media file, rescan so the DB row is pruned. Best effort; a fresh machine
 * simply has nothing to remove.
 */
const cleanupFixture = async () => {
  const matches = await findFixtureSongs();
  if (matches.length === 0) return;
  console.log(`  cleanup: removing ${matches.length} existing fixture song(s)`);
  for (const s of matches) {
    await post("delete_song_cache", { fileHash: s.file_hash }).catch(() => {});
    await fs.rm(s.path, { force: true }).catch(() => {});
  }
  await post("trigger_scan").catch(() => {});
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    if ((await findFixtureSongs()).length === 0) return;
    await sleep(1000);
  }
  fail("cleanup: fixture rows did not clear after delete + rescan");
};

const openSocket = () =>
  new Promise((resolve, reject) => {
    const ws = new WebSocket(WS_URL);
    ws.addEventListener("open", () => resolve(ws), { once: true });
    ws.addEventListener("error", () => reject(new TestError(`could not open WS ${WS_URL}`)), {
      once: true,
    });
  });

/**
 * Run one ingest and collect its `party.ingest` frames until a terminal
 * stage (ready/error). Returns { ready, stages } where stages is the ordered
 * list of stage names seen for this query.
 */
const runIngest = (ws, timeoutMs) =>
  new Promise((resolve, reject) => {
    const stages = [];
    const timer = setTimeout(
      () => reject(new TestError(`ingest did not finish within ${timeoutMs / 1000}s; stages=${stages}`)),
      timeoutMs,
    );
    const onMsg = (ev) => {
      const env = parse(ev.data);
      if (env?.type !== "party.ingest") return;
      const p = env.payload;
      stages.push(p.stage);
      console.log(`  [ingest] ${p.stage}${p.file_hash ? ` hash=${p.file_hash}` : ""}${p.error ? ` error=${p.error}` : ""}`);
      if (p.stage === "ready") {
        clearTimeout(timer);
        ws.removeEventListener("message", onMsg);
        resolve({ ready: p, stages });
      } else if (p.stage === "error") {
        clearTimeout(timer);
        ws.removeEventListener("message", onMsg);
        reject(new TestError(`ingest error: ${p.error}`));
      }
    };
    ws.addEventListener("message", onMsg);
    post("party_ingest", { query: QUERY }).catch(reject);
  });

const closeQuietly = (ws) =>
  new Promise((resolve) => {
    if (!ws || ws.readyState === WebSocket.CLOSED) return resolve();
    ws.addEventListener("close", () => resolve(), { once: true });
    ws.close();
    setTimeout(resolve, 300);
  });

const main = async () => {
  const ws = await openSocket();
  try {
    console.log("cleanup: ensuring the fixture is not already present...");
    await cleanupFixture();

    console.log("T2.2: first ingest (real download + analysis)...");
    const { ready, stages } = await runIngest(ws, READY_TIMEOUT_MS);

    if (!ready.file_hash) fail("ready frame missing file_hash");
    if (!stages.includes("analyzing")) {
      fail("first run should have analyzed (no 'analyzing' stage seen)");
    }

    // Resolve the full song via the party lookup and assert on it.
    const song = await post("party_song_by_hash", { fileHash: ready.file_hash });
    if (!song) fail(`party_song_by_hash returned null for ${ready.file_hash}`);
    console.log(
      `  song: artist="${song.artist}" title="${song.title}" analyzed=${song.is_analyzed} source=${song.transcript_source}`,
    );

    if (!song.is_analyzed) fail("song is not is_analyzed");
    if (!song.artist || song.artist === "Unknown Artist") {
      fail(`artist not populated (embed-metadata regression): "${song.artist}"`);
    }
    // "Lyrics" = LRCLIB lyrics run through forced alignment (the metadata-driven
    // path we want). "Generated" = slow Whisper fallback, which is the failure
    // this test guards against.
    if (song.transcript_source !== "Lyrics") {
      fail(`expected transcript_source Lyrics (LRCLIB+align), got "${song.transcript_source}"`);
    }
    console.log("T2.2 PASS: analyzed, artist populated, source=Lyrics");

    // T2.3: idempotency. Re-run the same ingest; it must reach ready WITHOUT a
    // second analysis pass, and resolve to the same hash.
    console.log("T2.3: second ingest (should be idempotent)...");
    const second = await runIngest(ws, IDEMPOTENT_TIMEOUT_MS);
    if (second.ready.file_hash !== ready.file_hash) {
      fail(`idempotency: hash changed ${ready.file_hash} -> ${second.ready.file_hash}`);
    }
    if (second.stages.includes("analyzing")) {
      fail("idempotency: second run re-analyzed (saw 'analyzing' stage)");
    }
    console.log("T2.3 PASS: re-ingest was idempotent (no re-analysis, same hash)");

    console.log("PASS: t2-ingest");
  } finally {
    await closeQuietly(ws);
  }
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
