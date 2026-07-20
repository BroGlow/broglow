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
// via e.g. session.status).

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const pluginFile = process.env.PLUGIN_FILE;
const sessionId = process.env.SESSION_ID;
const firstEvent = process.env.FIRST_EVENT || "session.created";
const statePath = join(process.env.HOME, ".claude", "clawlight", "state.json");

const mod = await import(pathToFileURL(pluginFile));
const hooks = await mod.clawlight({ directory: "/tmp/exit-proj" });

// Build the event that first introduces this session, matching how opencode
// shapes each type's properties.
const properties =
  firstEvent === "session.status"
    ? { sessionID: sessionId, status: { type: "busy" } }
    : { sessionID: sessionId, info: { id: sessionId, title: "Exit test" } };
await hooks.event({ event: { type: firstEvent, properties } });

// Ensure the "working" write has landed before exiting, so the exit handler's
// "ended" write is unambiguously the last writer (deterministic final state).
const deadline = Date.now() + 5000;
while (Date.now() < deadline) {
  try {
    const s = JSON.parse(readFileSync(statePath, "utf8"));
    if (s.sessions && s.sessions[sessionId]) break;
  } catch {}
  await new Promise((r) => setTimeout(r, 40));
}

process.exit(0);
