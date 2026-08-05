/**
 * T1.2 (automated integration): remote-controlled playback.
 *
 * Drives a running server: opens a WS, issues `party_play`, and asserts a
 * `jukebox` frame arrives carrying the requested hash and an advanced
 * play_token. A second play of the same song must advance the token again.
 * Exits non-zero on failure so it can gate later work.
 *
 * Robust to pre-existing jukebox state: the server holds state in memory for
 * its whole lifetime, so a re-run sees a connect snapshot that already carries
 * a prior hash/token. We baseline off that snapshot and only accept frames
 * whose token strictly advances past it.
 *
 * Requires Node 24+ (global WebSocket + fetch). No dependencies.
 *
 * Usage:
 *   node scripts/party-test/t1-remote-play.mjs [baseUrl] [fileHash]
 * Defaults: http://127.0.0.1:8080  and the Bad Romance fixture hash.
 */

const BASE = process.argv[2] ?? "http://127.0.0.1:8080";
const FILE_HASH = process.argv[3] ?? "7df0c494f3c8a7ccb974b6265e5b4f0e";
const WS_URL = BASE.replace(/^http/, "ws") + "/ws";
const TIMEOUT_MS = 2000;

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
  if (!res.ok) fail(`${name} returned HTTP ${res.status}: ${await res.text().catch(() => "")}`);
  return res.status === 204 ? null : res.json().catch(() => null);
};

const parse = (data) => {
  try {
    return JSON.parse(data);
  } catch {
    return null;
  }
};

/**
 * Open a WS and resolve once OPEN, capturing the baseline play_token from the
 * connect snapshot (the server sends one jukebox frame immediately). Resolving
 * only after OPEN means no broadcast can race the POST that follows.
 */
const openSocket = () =>
  new Promise((resolve, reject) => {
    const ws = new WebSocket(WS_URL);
    let baseline = 0;
    const onMsg = (ev) => {
      const env = parse(ev.data);
      if (env?.type === "jukebox" && typeof env.payload?.play_token === "number") {
        baseline = env.payload.play_token;
      }
    };
    ws.addEventListener("message", onMsg);
    ws.addEventListener("open", () => {
      // Give the connect snapshot a moment to land so the baseline is accurate.
      setTimeout(() => {
        ws.removeEventListener("message", onMsg);
        resolve({ ws, baseline });
      }, 150);
    });
    ws.addEventListener("error", () => reject(new TestError(`could not open WS to ${WS_URL}`)));
  });

/** Wait for a `jukebox` frame with matching hash and play_token > afterToken. */
const waitForPlay = (ws, hash, afterToken) =>
  new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new TestError(`no jukebox frame (hash ${hash}, token > ${afterToken}) in ${TIMEOUT_MS}ms`)),
      TIMEOUT_MS,
    );
    const onMsg = (ev) => {
      const env = parse(ev.data);
      if (env?.type !== "jukebox") return;
      const p = env.payload;
      if (p?.requested_song_hash === hash && p.play_token > afterToken) {
        clearTimeout(timer);
        ws.removeEventListener("message", onMsg);
        resolve(p);
      }
    };
    ws.addEventListener("message", onMsg);
  });

const closeQuietly = (ws) =>
  new Promise((resolve) => {
    if (!ws || ws.readyState === WebSocket.CLOSED) return resolve();
    ws.addEventListener("close", () => resolve(), { once: true });
    ws.close();
    setTimeout(resolve, 300);
  });

const main = async () => {
  const { ws, baseline } = await openSocket();
  try {
    // First play: subscribe (token > baseline), then command.
    const p1 = waitForPlay(ws, FILE_HASH, baseline);
    const cmd1 = await post("party_play", { fileHash: FILE_HASH });
    const frame1 = await p1;

    if (typeof frame1.play_token !== "number") fail("frame missing numeric play_token");
    if (cmd1?.playToken !== frame1.play_token) {
      fail(`command playToken ${cmd1?.playToken} != broadcast play_token ${frame1.play_token}`);
    }
    console.log(`ok: first play, hash=${FILE_HASH}, play_token=${frame1.play_token}`);

    // Second play of the SAME song must advance the token again. This is the
    // core reason play_token exists: a repeated hash looks unchanged otherwise.
    const p2 = waitForPlay(ws, FILE_HASH, frame1.play_token);
    await post("party_play", { fileHash: FILE_HASH });
    const frame2 = await p2;
    console.log(`ok: replay advanced token ${frame1.play_token} -> ${frame2.play_token}`);

    console.log("PASS: t1-remote-play");
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
