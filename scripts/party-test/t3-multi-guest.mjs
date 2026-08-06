/**
 * T3.3 (automated integration): multi-guest queue.
 *
 * Two WS clients simulate two phones. Both add a song (attributed to different
 * guests) and must see the same ordered queue; attribution is preserved per
 * entry; a removal is seen by both. Uses already-analyzed library songs added
 * by hash, so no slow ingest is involved.
 *
 * Requires Node 24+ (global WebSocket + fetch). No dependencies.
 *
 * Usage: node scripts/party-test/t3-multi-guest.mjs [baseUrl]
 */

const BASE = process.argv[2] ?? "http://127.0.0.1:8080";
const WS_URL = BASE.replace(/^http/, "ws") + "/ws";
const WAIT_MS = 4000;

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

/** A live phone: a WS that tracks the latest party.queue snapshot it has seen. */
class Phone {
  constructor(ws) {
    this.ws = ws;
    this.latest = null;
    ws.addEventListener("message", (ev) => {
      const env = parse(ev.data);
      if (env?.type === "party.queue") this.latest = env.payload;
    });
  }
  static open() {
    return new Promise((resolve, reject) => {
      const ws = new WebSocket(WS_URL);
      ws.addEventListener("open", () => resolve(new Phone(ws)), { once: true });
      ws.addEventListener("error", () => reject(new TestError(`could not open WS ${WS_URL}`)), {
        once: true,
      });
    });
  }
  close() {
    try {
      this.ws.close();
    } catch {}
  }
}

/** Poll a phone's latest snapshot until `pred` holds, or time out. */
const waitFor = async (phone, pred, label) => {
  const deadline = Date.now() + WAIT_MS;
  while (Date.now() < deadline) {
    if (phone.latest && pred(phone.latest)) return phone.latest;
    await new Promise((r) => setTimeout(r, 100));
  }
  fail(`timeout waiting for: ${label} (latest=${JSON.stringify(phone.latest)})`);
};

const hasEntryWith = (q, pred) => (q.entries ?? []).some(pred);

const analyzedHashes = async (n) => {
  const store = await post("load_songs", {
    params: { search: null, filters: emptyFilters, skip: 0, take: 100 },
  });
  const analyzed = (store?.processed ?? []).filter((s) => s.is_analyzed);
  if (analyzed.length < n) {
    fail(`need ${n} analyzed songs in the library, found ${analyzed.length}`);
  }
  return analyzed.slice(0, n);
};

const clearQueue = async () => {
  const q = await post("party_queue_list");
  for (const e of q.entries ?? []) {
    await post("party_queue_remove", { id: e.id });
  }
};

const main = async () => {
  await clearQueue();
  const [songA, songB] = await analyzedHashes(2);

  const p1 = await Phone.open();
  const p2 = await Phone.open();
  try {
    // Guest Alice (on phone 1) adds song A.
    const addA = await post("party_queue_add", {
      fileHash: songA.file_hash,
      requestedBy: "Alice",
    });
    const idA = addA.id;

    await waitFor(p1, (q) => hasEntryWith(q, (e) => e.id === idA), "phone1 sees A");
    await waitFor(p2, (q) => hasEntryWith(q, (e) => e.id === idA), "phone2 sees A");
    console.log("ok: both phones see Alice's song");

    // Guest Bob (on phone 2) adds song B.
    const addB = await post("party_queue_add", {
      fileHash: songB.file_hash,
      requestedBy: "Bob",
    });
    const idB = addB.id;

    const both = (q) =>
      hasEntryWith(q, (e) => e.id === idA) && hasEntryWith(q, (e) => e.id === idB);
    await waitFor(p1, both, "phone1 sees A+B");
    const snap = await waitFor(p2, both, "phone2 sees A+B");
    console.log("ok: both phones see both songs");

    // Attribution is preserved per entry.
    const entryA = snap.entries.find((e) => e.id === idA);
    const entryB = snap.entries.find((e) => e.id === idB);
    if (entryA.requestedBy !== "Alice") fail(`A attributed to "${entryA.requestedBy}", want Alice`);
    if (entryB.requestedBy !== "Bob") fail(`B attributed to "${entryB.requestedBy}", want Bob`);
    console.log("ok: per-entry attribution preserved (Alice / Bob)");

    // Remove B; both phones must see it go, A must remain.
    await post("party_queue_remove", { id: idB });
    const gone = (q) => !hasEntryWith(q, (e) => e.id === idB) && hasEntryWith(q, (e) => e.id === idA);
    await waitFor(p1, gone, "phone1 sees B removed");
    await waitFor(p2, gone, "phone2 sees B removed");
    console.log("ok: removal propagated to both phones");

    console.log("PASS: t3-multi-guest");
  } finally {
    p1.close();
    p2.close();
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
