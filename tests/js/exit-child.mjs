// Helper process for the "flips to done on exit" integration test.
//
// The plugin's shutdown handler is `process.on("exit")`, which only fires when
// the host process actually exits — so it can't be exercised inside the
// long-lived `node --test` process. This tiny script is spawned as its own
// process: it loads the real (substituted) plugin, starts one session, waits
// for the async "working" write to land, then exits. The plugin's exit handler
// must then synchronously flip that session to "done" (via spawnSync).
//
// Env in: PLUGIN_FILE (substituted plugin path), SESSION_ID, HOME (sandbox),
// FIRST_EVENT (optional; the event type that first introduces the session —
// defaults to session.created, but a resumed/attached session may be first seen
// via e.g. session.status), EXIT_SIGNAL (optional host cleanup scenario).

import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const pluginFile = process.env.PLUGIN_FILE;
const sessionId = process.env.SESSION_ID;
const firstEvent = process.env.FIRST_EVENT || "session.created";
const statePath = join(process.env.HOME, ".broglow", "state.json");

const mod = await import(pathToFileURL(pluginFile));
const hooks = await mod.broglow({ directory: "/tmp/exit-proj" });

// Build the event that first introduces this session, matching how opencode
// shapes each type's properties.
const properties =
  firstEvent === "session.status"
    ? { sessionID: sessionId, status: { type: "busy" } }
    : { sessionID: sessionId, info: { id: sessionId, title: "Exit test" } };
await hooks.event({ event: { type: firstEvent, properties } });

// Ensure the "working" write has landed before exiting, so the exit handler's
// "ended" write is unambiguously the last writer (deterministic final state).
let active = false;
const deadline = Date.now() + 5000;
while (Date.now() < deadline) {
  try {
    const s = JSON.parse(readFileSync(statePath, "utf8"));
    if (s.sessions?.[sessionId]?.status === "active") {
      active = true;
      break;
    }
  } catch {}
  await new Promise((r) => setTimeout(r, 40));
}

assert.ok(active, "the initial event must make the session active before shutdown");

const signal = process.env.EXIT_SIGNAL;
if (signal) {
  // Register after the plugin, as a host may do. A plugin signal listener that
  // exits early would prevent this host cleanup from completing.
  const handled = new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("host signal cleanup did not run")), 1000);
    process.once(signal, () => {
      clearTimeout(timeout);
      writeFileSync(join(process.env.HOME, "host-cleanup"), signal);
      resolve();
    });
  });
  process.kill(process.pid, signal);
  await handled;
}
process.exit(0);
