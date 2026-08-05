/**
 * T3 acceptance (automated): playback auto-advances from one queued song to the
 * next without intervention.
 *
 * Adds two analyzed library songs. The first must auto-promote to `playing` and
 * fire a jukebox play for its hash. On `party_song_ended` for that song, it must
 * go `done`, the second must go `playing`, and a fresh jukebox play must fire
 * for the second song's hash. The browser navigation itself is Step 1's proven
 * path; here we assert the server-side coordination that drives it.
 *
 * Requires Node 24+. Usage: node scripts/party-test/t3b-autoadvance.mjs [baseUrl]
 */

const BASE = process.argv[2] ?? "http://127.0.0.1:8080";
const WS_URL = BASE.replace(/^http/, "ws") + "/ws";
const WAIT_MS = 5000;

class TestError extends Error {}
const fail = (msg) => {
  throw new TestError(msg);
};
const parse = (d) => {
  try {
    return JSON.parse(d);
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

const analyzedHashes = async (n) => {
  const store = await post("load_songs", {
    params: { search: null, filters: emptyFilters, skip: 0, take: 100 },
  });
  const analyzed = (store?.processed ?? []).filter((s) => s.is_analyzed);
  if (analyzed.length < n) fail(`need ${n} analyzed songs, found ${analyzed.length}`);
  return analyzed.slice(0, n);
};

const clearQueue = async () => {
  const q = await post("party_queue_list");
  for (const e of q.entries ?? []) await post("party_queue_remove", { id: e.id });
};

const main = async () => {
  await clearQueue();
  const [songA, songB] = await analyzedHashes(2);
  if (songA.file_hash === songB.file_hash) fail("need two distinct song hashes");

  // Track queue snapshots and jukebox play events off one socket.
  let queue = null;
  const plays = []; // {hash, token}
  let baseToken = 0;
  const ws = await new Promise((resolve, reject) => {
    const s = new WebSocket(WS_URL);
    s.addEventListener("open", () => resolve(s), { once: true });
    s.addEventListener("error", () => reject(new TestError("WS open failed")), { once: true });
  });
  ws.addEventListener("message", (ev) => {
    const env = parse(ev.data);
    if (env?.type === "party.queue") queue = env.payload;
    else if (env?.type === "jukebox" && typeof env.payload?.play_token === "number") {
      const p = env.payload;
      if (p.requested_song_hash) plays.push({ hash: p.requested_song_hash, token: p.play_token });
      else baseToken = Math.max(baseToken, p.play_token);
    }
  });

  const waitUntil = async (pred, label) => {
    const deadline = Date.now() + WAIT_MS;
    while (Date.now() < deadline) {
      if (pred()) return;
      await new Promise((r) => setTimeout(r, 100));
    }
    fail(`timeout: ${label} (queue=${JSON.stringify(queue)})`);
  };
  const statusOf = (hash) => queue?.entries?.find((e) => e.fileHash === hash)?.status;
  const playedAfter = (hash, token) => plays.some((p) => p.hash === hash && p.token > token);

  try {
    // Add A; it should auto-play.
    await post("party_queue_add", { fileHash: songA.file_hash, requestedBy: "Alice" });
    await waitUntil(() => statusOf(songA.file_hash) === "playing", "A becomes playing");
    await waitUntil(() => playedAfter(songA.file_hash, baseToken), "jukebox plays A");
    const tokenA = plays.find((p) => p.hash === songA.file_hash).token;
    console.log(`ok: song A auto-plays (status=playing, play_token=${tokenA})`);

    // Add B; A is playing, so B waits at ready.
    await post("party_queue_add", { fileHash: songB.file_hash, requestedBy: "Bob" });
    await waitUntil(() => statusOf(songB.file_hash) === "ready", "B waits at ready");
    console.log("ok: song B queued as ready while A plays");

    // A ends -> advance.
    await post("party_song_ended", { fileHash: songA.file_hash });
    await waitUntil(() => statusOf(songA.file_hash) === "done", "A becomes done");
    await waitUntil(() => statusOf(songB.file_hash) === "playing", "B becomes playing");
    await waitUntil(() => playedAfter(songB.file_hash, tokenA), "jukebox plays B after A");
    console.log("ok: on song end, A done and B auto-plays");

    console.log("PASS: t3b-autoadvance");
  } finally {
    try {
      ws.close();
    } catch {}
    await clearQueue();
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
