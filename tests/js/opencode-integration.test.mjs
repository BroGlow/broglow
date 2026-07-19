// Integration tests for the opencode ↔ clawlight boundary.
//
// Unlike the Rust end-to-end tests (which feed normalized JSON straight to
// `clawlight event`), these drive the REAL embedded plugin — the exact
// `assets/opencode-plugin.js` that ships — with the opencode event shapes
// captured from a live opencode session, and assert the resulting `state.json`
// the clawlight binary writes. This is the contract that actually breaks if the
// plugin's event mapping or the binary's ingestion drift apart.
//
// Run: `node --test tests/js/`  (the Rust binary is auto-located or built.)
// Unix only: sessions land in `$HOME/.claude/clawlight/state.json`, sandboxed
// per test via a throwaway HOME — the same reason the Rust suite is unix-gated.

import { test, before } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  mkdtempSync,
  readFileSync,
  writeFileSync,
  existsSync,
  mkdirSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, "..", "..");
const isWindows = process.platform === "win32";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Locate the built clawlight binary, building a debug one if none exists. */
function findBinary() {
  if (process.env.CLAWLIGHT_BIN && existsSync(process.env.CLAWLIGHT_BIN)) {
    return process.env.CLAWLIGHT_BIN;
  }
  for (const rel of ["target/debug/clawlight", "target/release/clawlight"]) {
    const abs = join(REPO, rel);
    if (existsSync(abs)) return abs;
  }
  execFileSync("cargo", ["build"], { cwd: REPO, stdio: "inherit" });
  return join(REPO, "target/debug/clawlight");
}

/** Write the shipped plugin with the binary path + version substituted in,
 *  exactly as `clawlight install` does, and return the file path. */
function writeSubstitutedPlugin(bin) {
  const src = readFileSync(join(REPO, "assets/opencode-plugin.js"), "utf8")
    .replaceAll("{{VERSION}}", "test")
    .replaceAll("{{BIN}}", bin.replaceAll("\\", "\\\\").replaceAll('"', '\\"'));
  const dir = mkdtempSync(join(tmpdir(), "clw-plugin-"));
  const file = join(dir, "clawlight.mjs");
  writeFileSync(file, src);
  return file;
}

const BIN = isWindows ? null : findBinary();
const PLUGIN_FILE = isWindows ? null : writeSubstitutedPlugin(BIN);

let plugin; // the plugin module (factory reused; each call = fresh instance)
before(async () => {
  if (isWindows) return;
  plugin = await import(pathToFileURL(PLUGIN_FILE));
});

/** Fresh sandbox HOME; the spawned `clawlight event` children inherit
 *  process.env.HOME, so point it here before feeding events. */
function sandbox() {
  const home = mkdtempSync(join(tmpdir(), "clw-home-"));
  process.env.HOME = home;
  return home;
}

function statePath(home) {
  return join(home, ".claude", "clawlight", "state.json");
}

function readState(home) {
  const p = statePath(home);
  return existsSync(p) ? JSON.parse(readFileSync(p, "utf8")) : { sessions: {} };
}

function seedState(home, sessions) {
  const dir = join(home, ".claude", "clawlight");
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "state.json"), JSON.stringify({ sessions }));
}

/** The plugin fires `clawlight event` fire-and-forget, so poll `state.json`
 *  until `pred(state)` holds or we time out. */
async function waitFor(home, pred, timeout = 6000) {
  const start = Date.now();
  let last;
  while (Date.now() - start < timeout) {
    last = readState(home);
    try {
      if (pred(last)) return last;
    } catch {}
    await sleep(40);
  }
  throw new Error(`timeout; last state = ${JSON.stringify(last)}`);
}

/** Instantiate a fresh plugin and return a helper that emits one opencode bus
 *  event through its real `event` hook. */
function newSession(ctx = { directory: "/tmp/oc-proj" }) {
  const hooks = plugin.clawlight(ctx);
  return async (type, properties = {}) => {
    const h = await hooks;
    await h.event({ event: { type, properties } });
  };
}

const info = (id, extra = {}) => ({ sessionID: id, info: { id, ...extra } });

test("full lifecycle: created → message → permission → reply → idle → deleted", {
  skip: isWindows && "unix-only (HOME can't sandbox state on Windows)",
}, async () => {
  const home = sandbox();
  const emit = newSession();
  const id = "ses_life";

  await emit("session.created", info(id, { title: "Refactor tokenizer" }));
  let s = await waitFor(home, (st) => st.sessions[id]?.status === "active");
  assert.equal(s.sessions[id].harness, "opencode");
  assert.equal(s.sessions[id].name, "Refactor tokenizer");
  assert.equal(s.sessions[id].project_path, "/tmp/oc-proj");

  await emit("permission.asked", { sessionID: id });
  s = await waitFor(home, (st) => st.sessions[id]?.status === "needs_help");
  assert.equal(s.sessions[id].status, "needs_help");

  await emit("permission.replied", { sessionID: id });
  await waitFor(home, (st) => st.sessions[id]?.status === "active");

  await emit("session.idle", { sessionID: id });
  await waitFor(home, (st) => st.sessions[id]?.status === "inactive");

  await emit("session.deleted", { sessionID: id });
  s = await waitFor(home, (st) => st.sessions[id]?.status === "done");
  assert.equal(s.sessions[id].status, "done");
  // The name set from the title survives every status transition.
  assert.equal(s.sessions[id].name, "Refactor tokenizer");
});

test("session.updated passes the new title through without changing status", {
  skip: isWindows && "unix-only",
}, async () => {
  const home = sandbox();
  const emit = newSession();
  const id = "ses_title";

  await emit("session.created", info(id, { title: "First title" }));
  await waitFor(home, (st) => st.sessions[id]?.name === "First title");
  await emit("session.idle", { sessionID: id });
  await waitFor(home, (st) => st.sessions[id]?.status === "inactive");

  // A pure title change must update the name but leave the idle session idle.
  await emit("session.updated", info(id, { title: "Renamed session" }));
  const s = await waitFor(home, (st) => st.sessions[id]?.name === "Renamed session");
  assert.equal(s.sessions[id].status, "inactive");
});

test("a burst of working before idle still settles on idle (write ordering)", {
  skip: isWindows && "unix-only",
}, async () => {
  const home = sandbox();
  const emit = newSession();
  const id = "ses_burst";
  await emit("session.created", info(id));
  // The end-of-turn cadence: several `working` events then `idle`, with no gaps.
  // Detached, unordered writes used to let a stray `working` land after `idle`
  // and leave the light stuck green; the plugin serializes sends to prevent it.
  for (let i = 0; i < 8; i++) await emit("message.updated", info(id));
  await emit("session.idle", { sessionID: id });

  const s = await waitFor(home, (st) => st.sessions[id]?.status === "inactive");
  assert.equal(s.sessions[id].status, "inactive");
  // Must STAY inactive once the whole queue has drained (no late working write).
  await sleep(500);
  assert.equal(readState(home).sessions[id].status, "inactive");
});

test("server.connected sweeps this harness's stale sessions to done", {
  skip: isWindows && "unix-only",
}, async () => {
  const home = sandbox();
  // A leftover active opencode session with no owner_pid → stale on reconnect.
  seedState(home, {
    ses_old: {
      status: "active",
      last_updated: "2020-01-01T00:00:00Z",
      project_path: null,
      notification_type: null,
      name: "leftover",
      harness: "opencode",
    },
  });
  const emit = newSession();
  await emit("server.connected", {});
  const s = await waitFor(home, (st) => st.sessions.ses_old?.status === "done");
  assert.equal(s.sessions.ses_old.status, "done");
});

test("the plugin registers only an exit handler, never signal handlers", {
  skip: isWindows && "unix-only",
}, async () => {
  // A signal handler that calls process.exit() would pre-empt opencode's own
  // Ctrl+C cleanup — the one place the "can't hurt the host" rule could break.
  const before = {
    exit: process.listenerCount("exit"),
    sigint: process.listenerCount("SIGINT"),
    sigterm: process.listenerCount("SIGTERM"),
  };
  await plugin.clawlight({ directory: "/tmp/x" });
  assert.equal(process.listenerCount("exit") - before.exit, 1, "one exit handler");
  assert.equal(process.listenerCount("SIGINT") - before.sigint, 0, "no SIGINT handler");
  assert.equal(process.listenerCount("SIGTERM") - before.sigterm, 0, "no SIGTERM handler");
});

test("the plugin flips its live sessions to done on process exit", {
  skip: isWindows && "unix-only",
}, async () => {
  const home = mkdtempSync(join(tmpdir(), "clw-home-"));
  const id = "ses_exit";
  // Run in a dedicated child process so the plugin's real `process.on("exit")`
  // handler fires (it can't in the long-lived test process). The child starts a
  // session and exits; the handler must synchronously mark it done.
  execFileSync(process.execPath, [join(HERE, "exit-child.mjs")], {
    env: { ...process.env, HOME: home, PLUGIN_FILE, SESSION_ID: id },
    stdio: "inherit",
  });
  const s = await waitFor(home, (st) => st.sessions[id]?.status === "done");
  assert.equal(s.sessions[id].status, "done");
  assert.equal(s.sessions[id].harness, "opencode");
});
