/**
 * T3d (automated integration): the auto-advance watchdog respects pause.
 *
 * Regression for the review finding that the watchdog advanced a paused song
 * once its wall-clock duration elapsed. Here we simulate the TV heartbeating
 * `party_progress` with paused=true for well past the song's duration + grace,
 * and assert the song is NOT advanced. Then we heartbeat a playing position past
 * the end and assert it DOES advance (the position-based safety net still works).
 *
 * Slow by nature (waits past the short fixture's ~26s duration). Requires Node
 * 24+. Usage: node scripts/party-test/t3d-watchdog-pause.mjs [baseUrl]
 */

const BASE = process.argv[2] ?? "http://127.0.0.1:8080";
const SHORT = "e72ad12d9ba7e5af8320712f212a1146"; // Her Majesty, ~26s
const LONG = "7df0c494f3c8a7ccb974b6265e5b4f0e"; // Bad Romance

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

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const statusOf = async (hash) => {
  const q = await post("party_queue_list");
  return q.entries.find((e) => e.fileHash === hash)?.status;
};

const main = async () => {
  const q = await post("party_queue_list");
  for (const e of q.entries) await post("party_queue_remove", { id: e.id });

  await post("party_queue_add", { fileHash: SHORT, requestedBy: "Pause" });
  await post("party_queue_add", { fileHash: LONG, requestedBy: "Pause" });
  if ((await statusOf(SHORT)) !== "playing") fail("short song did not start playing");

  // Heartbeat PAUSED for well past duration (26s) + grace (6s). If the watchdog
  // ignored pause it would advance around 32s; we go to ~42s.
  console.log("heartbeating paused=true for ~42s (song is 26s)...");
  const pausedUntil = Date.now() + 42000;
  while (Date.now() < pausedUntil) {
    await post("party_progress", { positionMs: 5000, paused: true });
    await sleep(2000);
  }

  if ((await statusOf(SHORT)) !== "playing") {
    fail("watchdog advanced a PAUSED song (should have held)");
  }
  console.log("ok: paused song held past duration + grace (not advanced)");

  // Now report playing, position past the end: the watchdog should advance.
  console.log("heartbeating playing position past the end...");
  await post("party_progress", { positionMs: 40000, paused: false });
  const deadline = Date.now() + 8000;
  let advanced = false;
  while (Date.now() < deadline) {
    if ((await statusOf(SHORT)) === "done" && (await statusOf(LONG)) === "playing") {
      advanced = true;
      break;
    }
    await sleep(500);
  }
  if (!advanced) fail("watchdog did not advance once position passed the end");
  console.log("ok: advanced once a playing position passed the end");

  const q2 = await post("party_queue_list");
  for (const e of q2.entries) await post("party_queue_remove", { id: e.id });
  console.log("PASS: t3d-watchdog-pause");
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
