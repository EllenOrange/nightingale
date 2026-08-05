/**
 * T4.1 (automated integration): admin controls are shared.
 *
 * Two WS clients (an "admin" and an "observer") both watch jukebox state. Each
 * control command must change the broadcast state, and the SECOND client must
 * observe the change too, since control is shared across every connected
 * device. Exercises pause/resume, guide vocal, volume, key, and restart.
 *
 * Requires Node 24+. Usage: node scripts/party-test/t4-admin.mjs [baseUrl]
 */

const BASE = process.argv[2] ?? "http://127.0.0.1:8080";
const WS_URL = BASE.replace(/^http/, "ws") + "/ws";
const WAIT_MS = 3000;

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

const openClient = () =>
  new Promise((resolve, reject) => {
    const ws = new WebSocket(WS_URL);
    const client = { ws, jukebox: null };
    ws.addEventListener("message", (ev) => {
      const env = parse(ev.data);
      if (env?.type === "jukebox") client.jukebox = env.payload;
    });
    ws.addEventListener("open", () => resolve(client), { once: true });
    ws.addEventListener("error", () => reject(new TestError(`WS open failed ${WS_URL}`)), {
      once: true,
    });
  });

const bothSee = async (clients, pred, label) => {
  const deadline = Date.now() + WAIT_MS;
  while (Date.now() < deadline) {
    if (clients.every((c) => c.jukebox && pred(c.jukebox))) return;
    await new Promise((r) => setTimeout(r, 80));
  }
  fail(`timeout: ${label} (states=${clients.map((c) => JSON.stringify(c.jukebox)).join(" | ")})`);
};

const main = async () => {
  const admin = await openClient();
  const observer = await openClient();
  const clients = [admin, observer];
  try {
    await post("party_control_pause");
    await bothSee(clients, (j) => j.paused === true, "both see paused=true");
    console.log("ok: pause is shared");

    await post("party_control_resume");
    await bothSee(clients, (j) => j.paused === false, "both see paused=false");
    console.log("ok: resume is shared");

    await post("party_set_guide_vocal", { value: 0.7 });
    await bothSee(clients, (j) => Math.abs((j.guide_vocal ?? -1) - 0.7) < 1e-6, "both see guide 0.7");
    console.log("ok: guide vocal is shared");

    await post("party_set_volume", { value: 0.4 });
    await bothSee(clients, (j) => Math.abs((j.volume ?? -1) - 0.4) < 1e-6, "both see volume 0.4");
    console.log("ok: volume is shared");

    await post("party_set_key", { offset: 3 });
    await bothSee(clients, (j) => j.key_offset === 3, "both see key +3");
    console.log("ok: key offset is shared");

    // Clamp check: out-of-range values are clamped, not stored raw.
    const clamped = await post("party_set_volume", { value: 5 });
    if (clamped.volume !== 1) fail(`volume not clamped: ${clamped.volume}`);
    console.log("ok: out-of-range control is clamped");

    const before = admin.jukebox.restart_token ?? 0;
    await post("party_control_restart");
    await bothSee(clients, (j) => (j.restart_token ?? 0) > before, "both see restart bump");
    console.log("ok: restart token is shared");

    console.log("PASS: t4-admin");
  } finally {
    admin.ws.close();
    observer.ws.close();
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
