/**
 * T3c (automated integration): auto-advance survives a missing TV report.
 *
 * The TV normally reports party_song_ended, but a party must not stall if the
 * TV tab is closed/asleep/stale. This queues two analyzed songs, never sends
 * party_song_ended, and asserts the server's duration watchdog advances anyway:
 * the first song goes done and the second starts.
 *
 * Slow by nature: it waits out the first song's real duration. Uses the short
 * Her Majesty fixture (~26s) first to keep it as quick as possible.
 *
 * Requires Node 24+. Usage: node scripts/party-test/t3c-watchdog.mjs [baseUrl]
 */

const BASE = process.argv[2] ?? "http://127.0.0.1:8080";
const SHORT = "e72ad12d9ba7e5af8320712f212a1146"; // Her Majesty, ~26s
const LONG = "7df0c494f3c8a7ccb974b6265e5b4f0e"; // Bad Romance
const WATCHDOG_GRACE = 6; // must match the server constant
const MARGIN = 8; // extra slack for scheduling

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

const queueStatus = async (hash) => {
  const q = await post("party_queue_list");
  return q.entries.find((e) => e.fileHash === hash)?.status;
};

const durationOf = async (hash) => {
  const store = await post("load_songs", {
    params: { search: null, filters: emptyFilters, skip: 0, take: 100 },
  });
  const s = (store?.processed ?? []).find((x) => x.file_hash === hash);
  if (!s) fail(`fixture ${hash} not in library (analyze it first)`);
  if (!s.is_analyzed) fail(`fixture ${hash} is not analyzed`);
  return s.duration_secs;
};

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const main = async () => {
  // Clear, then queue short-first so the wait is minimal.
  const q = await post("party_queue_list");
  for (const e of q.entries) await post("party_queue_remove", { id: e.id });

  const dur = await durationOf(SHORT);
  await durationOf(LONG); // presence/analyzed check

  await post("party_queue_add", { fileHash: SHORT, requestedBy: "Watchdog" });
  await post("party_queue_add", { fileHash: LONG, requestedBy: "Watchdog" });

  // First should be playing, second ready. (No TV, so no song_ended will fire.)
  if ((await queueStatus(SHORT)) !== "playing") fail("short song did not start playing");
  if ((await queueStatus(LONG)) !== "ready") fail("long song is not waiting at ready");
  console.log(`ok: short song playing (${Math.round(dur)}s), long song ready; sending NO song_ended`);

  const waitMs = (dur + WATCHDOG_GRACE + MARGIN) * 1000;
  console.log(`waiting ${Math.round(waitMs / 1000)}s for the watchdog...`);
  await sleep(waitMs);

  const shortStatus = await queueStatus(SHORT);
  const longStatus = await queueStatus(LONG);
  if (shortStatus !== "done") fail(`short song should be done, is ${shortStatus}`);
  if (longStatus !== "playing") fail(`long song should be playing, is ${longStatus}`);
  console.log("ok: watchdog advanced with no TV report (short done, long playing)");

  // Cleanup.
  const q2 = await post("party_queue_list");
  for (const e of q2.entries) await post("party_queue_remove", { id: e.id });
  console.log("PASS: t3c-watchdog");
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
