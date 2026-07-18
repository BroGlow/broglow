// Helper process for the "flips to done on exit" integration test.
//
// The plugin's shutdown handler is `process.on("exit")`, which only fires when
// the host process actually exits — so it can't be exercised inside the
// long-lived `node --test` process. This tiny script is spawned as its own
// process: it loads the real (substituted) plugin, starts one session, waits
// for the async "working" write to land, then exits. The plugin's exit handler
// must then synchronously flip that session to "done" (via spawnSync).
//
// Env in: PLUGIN_FILE (substituted plugin path), SESSION_ID, HOME (sandbox).

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

const pluginFile = process.env.PLUGIN_FILE;
const sessionId = process.env.SESSION_ID;
const statePath = join(process.env.HOME, ".claude", "clawlight", "state.json");

const mod = await import(pathToFileURL(pluginFile));
const hooks = await mod.clawlight({ directory: "/tmp/exit-proj" });

await hooks.event({
  event: {
    type: "session.created",
    properties: { sessionID: sessionId, info: { id: sessionId, title: "Exit test" } },
  },
});

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
