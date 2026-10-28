// Integration tests for the opencode ↔ broglow boundary.
//
// Unlike the Rust end-to-end tests (which feed normalized JSON straight to
// `broglow event`), these drive the REAL embedded plugin — the exact
// `assets/opencode-plugin.js` that ships — with the opencode event shapes
// captured from a live opencode session, and assert the resulting `state.json`
// the broglow binary writes. This is the contract that actually breaks if the
// plugin's event mapping or the binary's ingestion drift apart.
//
// Run: `cargo build --locked && node --test tests/js/*.test.mjs`.
// Unix only: sessions land in `$HOME/.broglow/state.json`, sandboxed
// per test via a throwaway HOME — the same reason the Rust suite is unix-gated.

import { test, after } from "node:test";
import assert from "node:assert/strict";
import { execFileSync, fork } from "node:child_process";
import { once } from "node:events";
import {
  mkdtempSync,
  readFileSync,
  writeFileSync,
  existsSync,
  mkdirSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, "..", "..");
const isWindows = process.platform === "win32";

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Locate the built broglow binary, building a debug one if none exists. */
function findBinary() {
  if (process.env.BROGLOW_BIN && existsSync(process.env.BROGLOW_BIN)) {
    return process.env.BROGLOW_BIN;
  }
  for (const rel of ["target/debug/broglow", "target/release/broglow"]) {
    const abs = join(REPO, rel);
    if (existsSync(abs)) return abs;
  }
  execFileSync("cargo", ["build", "--locked"], { cwd: REPO, stdio: "inherit" });
  return join(REPO, "target/debug/broglow");
}

/** Write the shipped plugin with the binary path + version substituted in,
 *  exactly as `broglow install` does, and return the file path. */
function writeSubstitutedPlugin(bin) {
  const src = readFileSync(join(REPO, "assets/opencode-plugin.js"), "utf8")
    .replaceAll("{{VERSION}}", "test")
    .replaceAll("{{BIN}}", bin.replaceAll("\\", "\\\\").replaceAll('"', '\\"'));
  const dir = mkdtempSync(join(tmpdir(), "broglow-plugin-"));
  const file = join(dir, "broglow.mjs");
  writeFileSync(file, src);
  return file;
}

const BIN = isWindows ? null : findBinary();
const PLUGIN_FILE = isWindows ? null : writeSubstitutedPlugin(BIN);
after(() => {
  if (PLUGIN_FILE) rmSync(dirname(PLUGIN_FILE), { recursive: true, force: true });
});

/** A child-only HOME prevents queued plugin work from ever touching the
 *  developer's state, including when an assertion or timeout aborts the test. */
function sandbox(t) {
  const home = mkdtempSync(join(tmpdir(), "broglow-home-"));
  const children = [];
  t.after(async () => {
    for (const child of children) {
      if (child.exitCode !== null || child.signalCode !== null) continue;
      const exited = once(child, "exit");
      child.send({ kind: "stop" });
      await exited;
    }
    rmSync(home, { recursive: true, force: true });
  });
  return {
    home,
    async newSession(ctx = { directory: "/tmp/oc-proj" }) {
      const child = fork(join(HERE, "plugin-child.mjs"), [JSON.stringify(ctx)], {
        env: { ...process.env, HOME: home, PLUGIN_FILE },
        execArgv: [],
        stdio: ["ignore", "inherit", "inherit", "ipc"],
      });
      children.push(child);
      await once(child, "message", { signal: AbortSignal.timeout(6000) });
      const request = async message => {
        const accepted = once(child, "message", { signal: AbortSignal.timeout(6000) });
        child.send(message);
        await accepted;
      };
      return {
        emit: (type, properties = {}) => request({ kind: "event", event: { type, properties } }),
        drain: () => request({ kind: "drain" }),
        close: async () => {
          const exited = once(child, "exit");
          child.send({ kind: "stop" });
          await exited;
        },
      };
    },
  };
}

function statePath(home) {
  return join(home, ".broglow", "state.json");
}

function readState(home) {
  const p = statePath(home);
  return existsSync(p) ? JSON.parse(readFileSync(p, "utf8")) : { sessions: {} };
}

function seedState(home, sessions) {
  const dir = join(home, ".broglow");
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "state.json"), JSON.stringify({ sessions }));
}

/** The plugin fires `broglow event` fire-and-forget, so poll `state.json`
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

const info = (id, extra = {}) => ({ sessionID: id, info: { id, ...extra } });
const status = (id, type) => ({ sessionID: id, status: { type } });

function usagePath(home, completed) {
  const date = new Date(completed);
  const day = [date.getFullYear(), String(date.getMonth() + 1).padStart(2, "0"),
    String(date.getDate()).padStart(2, "0")].join("-");
  return join(home, ".broglow", `opencode-usage-${day}.json`);
}

function enableUsage(home) {
  const dir = join(home, ".broglow");
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "config.json"), JSON.stringify({ usage_enabled: true }));
}

const assistantUsage = (sessionID, id, completed, total) => ({ info: {
  sessionID, id, role: "assistant", time: { created: completed - 1000, completed },
  tokens: { total, input: 100, output: 30, reasoning: 20, cache: { read: 50, write: 10 } },
  text: "message content must not be persisted by BroGlow",
} });

test("API usage reconciles completed messages across sessions and plugin restarts without changing status", {
  skip: isWindows && "unix-only",
}, async (t) => {
  const { home, newSession } = sandbox(t);
  enableUsage(home);
  const completed = Date.now();
  const first = await newSession();
  await first.emit("session.created", info("ses-api", { title: "API task" }));
  await first.emit("session.idle", { sessionID: "ses-api" });
  await first.drain();
  const originalState = readFileSync(statePath(home), "utf8");

  await first.emit("message.updated", assistantUsage("ses-api", "msg-same", completed));
  await first.emit("message.updated", assistantUsage("ses-api", "msg-same", completed, 200));
  await first.emit("message.updated", assistantUsage("ses-api", "msg-same", completed, 150));
  await first.drain();
  assert.equal(readFileSync(statePath(home), "utf8"), originalState);

  // A fresh plugin instance sees the same completed message again.
  const restarted = await newSession();
  await restarted.emit("message.updated", assistantUsage("ses-api", "msg-same", completed, 200));
  await restarted.emit("message.updated", assistantUsage("ses-other", "msg-same", completed));
  await restarted.drain();
  await restarted.close();
  assert.deepEqual(JSON.parse(readFileSync(usagePath(home, completed), "utf8")), {
    "ses-api": { "msg-same": 200 }, "ses-other": { "msg-same": 190 },
  });
  // Usage-only messages must not enter the live-session set, including on exit.
  assert.equal(readFileSync(statePath(home), "utf8"), originalState);
});

test("usage opt-out and incomplete messages create no tracking files or message sessions", {
  skip: isWindows && "unix-only",
}, async (t) => {
  const { home, newSession } = sandbox(t);
  const plugin = await newSession();
  const completed = Date.now();
  await plugin.emit("message.updated", assistantUsage("ses-api", "msg-api", completed));
  await plugin.drain();
  assert.ok(!existsSync(join(home, ".broglow")), "opt-out writes nothing");

  enableUsage(home);
  const incomplete = assistantUsage("ses-api", "msg-api", completed);
  delete incomplete.info.time.completed;
  await plugin.emit("message.updated", incomplete);
  const user = assistantUsage("ses-api", "msg-user", completed);
  user.info.role = "user";
  await plugin.emit("message.updated", user);
  await plugin.drain();
  await plugin.close();
  assert.ok(!existsSync(usagePath(home, completed)));
  assert.ok(!existsSync(statePath(home)), "message IDs never become sessions, even on exit");
});

test("full lifecycle: created → permission → reply → idle → deleted", {
  skip: isWindows && "unix-only (HOME can't sandbox state on Windows)",
}, async (t) => {
  const { home, newSession } = sandbox(t);
  const { emit } = await newSession();
  const id = "ses_life";

  await emit("session.created", info(id, { title: "Refactor tokenizer" }));
  let s = await waitFor(home, (st) => st.sessions[id]?.status === "active");
  assert.equal(s.sessions[id].harness, "opencode");
  assert.equal(s.sessions[id].name, "Refactor tokenizer");
  assert.equal(s.sessions[id].project_path, "/tmp/oc-proj");

  await emit("permission.asked", { sessionID: id });
  await waitFor(home, (st) => st.sessions[id]?.status === "needs_help");

  await emit("permission.replied", { sessionID: id });
  await waitFor(home, (st) => st.sessions[id]?.status === "active");

  await emit("session.idle", { sessionID: id });
  await waitFor(home, (st) => st.sessions[id]?.status === "inactive");

  await emit("session.deleted", { sessionID: id });
  s = await waitFor(home, (st) => st.sessions[id]?.status === "done");
  // The name set from the title survives every status transition.
  assert.equal(s.sessions[id].name, "Refactor tokenizer");
});

test("a closed session stays done despite trailing events", {
  skip: isWindows && "unix-only",
}, async (t) => {
  const { home, newSession } = sandbox(t);
  const { emit, drain } = await newSession();
  const id = "ses_closed";
  // A session runs, then is closed (session.deleted -> done). opencode can still
  // emit trailing events after the delete — a final message.updated persisting
  // the message, a title update. None may resurrect the closed session: the
  // light must not flip back to green/yellow once it's gone. Same trailing-event
  // class as the stuck-green-when-idle bug, at the end of a session's life.
  await emit("session.created", info(id, { title: "Quick task" }));
  await emit("session.status", status(id, "busy"));
  await emit("session.deleted", { sessionID: id });
  await waitFor(home, (st) => st.sessions[id]?.status === "done");

  // Trailing events opencode really emits after a delete — a final
  // message.updated and a title update — must not revive the session.
  await emit("message.updated", info(id));
  await emit("session.updated", info(id, { title: "After close" }));
  await drain();
  const state = readState(home);
  assert.equal(state.sessions[id].status, "done", "closed session stays done");
  assert.equal(state.sessions[id].name, "After close");
});

test("a busy burst settles on idle, including trailing messages and title updates", {
  skip: isWindows && "unix-only",
}, async (t) => {
  const { home, newSession } = sandbox(t);
  const { emit, drain } = await newSession();
  const id = "ses_trail";
  // Faithful replay of the end-of-turn sequence captured from a real long-lived
  // opencode session (`session.status` drives busy/idle; `message.updated`
  // fires throughout AND once more *after* idle to persist the finished
  // message). If `message.updated` were treated as working, that trailing event
  // would flip the just-idled session back to green — the "stuck green when
  // idle" bug. session.status must be the only working/idle driver.
  await emit("session.created", info(id, { title: "Turn" }));
  await emit("session.status", status(id, "busy"));
  await emit("message.updated", info(id)); // during work — ignored
  await emit("message.updated", info(id));
  for (let i = 0; i < 8; i++) await emit("session.status", status(id, "busy"));
  await emit("session.status", status(id, "idle"));
  await emit("session.idle", { sessionID: id });
  await emit("message.updated", info(id)); // trailing — the bug trigger
  await emit("session.updated", info(id, { title: "Turn done" }));

  // Wait for actual child completion, even if the plugin stops serializing
  // writes: a late working process must not escape the final assertion.
  await drain();
  const s = readState(home);
  assert.equal(s.sessions[id].status, "inactive", "stays inactive after the trailing event");
  assert.equal(s.sessions[id].name, "Turn done");
});

test("server.connected sweeps this harness's stale sessions to done", {
  skip: isWindows && "unix-only",
}, async (t) => {
  const { home, newSession } = sandbox(t);
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
  const { emit } = await newSession();
  await emit("server.connected", {});
  await waitFor(home, (st) => st.sessions.ses_old?.status === "done");
});

for (const { name, firstEvent, signal } of [
  { name: "new session", firstEvent: "session.created" },
  { name: "resumed session", firstEvent: "session.status" },
  { name: "host Ctrl+C cleanup", firstEvent: "session.created", signal: "SIGINT" },
]) {
  test(`process exit ends the ${name}`, {
    skip: isWindows && "unix-only",
  }, async (t) => {
    const { home } = sandbox(t);
    const id = "ses_exit";

    execFileSync(process.execPath, [join(HERE, "exit-child.mjs")], {
      env: { ...process.env, HOME: home, PLUGIN_FILE, SESSION_ID: id, FIRST_EVENT: firstEvent,
        EXIT_SIGNAL: signal || "" },
      stdio: "inherit",
      timeout: 10000,
    });

    const state = readState(home);
    assert.equal(state.sessions[id]?.status, "done");
    assert.equal(state.sessions[id].harness, "opencode");
    if (signal) assert.equal(readFileSync(join(home, "host-cleanup"), "utf8"), signal);
  });
}
