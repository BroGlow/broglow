// managed by clawlight v{{VERSION}} — do not edit; `clawlight uninstall` removes this file
//
// A dumb translator between opencode's plugin event bus and clawlight. It
// subscribes to session / permission events, normalizes each into clawlight's
// harness-agnostic verb schema, and hands it to the clawlight binary over
// stdin. It is logic-free on purpose: every mapping decision lives in the Rust
// binary (where it is tested), so this file never needs to change when the
// mapping does, and any failure here is swallowed so it can never crash or
// slow down the opencode host.
//
// The clawlight binary path is baked in at install time (below) so the plugin
// works even when clawlight isn't on opencode's PATH. `clawlight install`
// rewrites this file every run, keeping the path and version in sync.

import { spawn, spawnSync } from "node:child_process";

// Absolute path to the clawlight binary, substituted by `clawlight install`.
const CLAWLIGHT_BIN = "{{BIN}}";

export const clawlight = async ({ directory, worktree, project }) => {
  // Best-effort project directory for the session's state entry.
  const dir =
    worktree || directory || (project && (project.worktree || project.path)) || "";

  // Spawn one `clawlight event` and resolve when it exits (or after a short
  // timeout, so a wedged clawlight can't stall the queue). Detached + unref so
  // the write still completes if opencode exits right after; failures swallowed.
  const spawnEvent = (event, sessionID, title) =>
    new Promise((resolve) => {
      let done = false;
      const finish = () => {
        if (!done) {
          done = true;
          resolve();
        }
      };
      try {
        const child = spawn(CLAWLIGHT_BIN, ["event"], {
          stdio: ["pipe", "ignore", "ignore"],
          detached: true,
        });
        child.on("error", finish);
        child.on("exit", finish);
        child.stdin.on("error", () => {});
        const payload = { harness: "opencode", event, directory: dir };
        if (sessionID) payload.session_id = sessionID;
        if (title) payload.title = title;
        child.stdin.end(JSON.stringify(payload));
        child.unref();
        const t = setTimeout(finish, 3000);
        if (t.unref) t.unref();
      } catch (_) {
        finish();
      }
    });

  // Serialize the sends so `state.json` writes land in the exact order opencode
  // emits events. This is the whole reason the queue exists: the spawns are
  // otherwise concurrent detached processes that the state lock serializes in an
  // arbitrary order, so a `working` write from just before a turn ends could
  // land after `idle`/`ended` and wrongly flip the light back to green. The
  // event handler still returns immediately — the chain drains in the
  // background — so the host is never blocked. Errors are swallowed so one bad
  // spawn can't wedge the queue.
  let queue = Promise.resolve();
  const send = (event, sessionID, title) => {
    queue = queue.then(() => spawnEvent(event, sessionID, title)).catch(() => {});
  };

  // Shutdown layer (a): when this opencode process exits, mark every session it
  // owns as ended so the light doesn't stay green. This runs in the `exit`
  // handler, where only *synchronous* work completes before the process dies —
  // an async spawn would be cut off — so it uses spawnSync. Layer (b), the
  // owner-PID reap on the clawlight side, still covers a hard SIGKILL that skips
  // this entirely.
  const live = new Set();
  const sendSync = (event, sessionID) => {
    try {
      spawnSync(CLAWLIGHT_BIN, ["event"], {
        input: JSON.stringify({
          harness: "opencode",
          event,
          session_id: sessionID,
          directory: dir,
        }),
        stdio: ["pipe", "ignore", "ignore"],
        // Runs in the exit path × one per live session, so keep it short — a
        // wedged clawlight must not hold up the host's shutdown.
        timeout: 500,
      });
    } catch (_) {
      // never throw out of an exit handler
    }
  };
  const markEnded = () => {
    for (const id of live) sendSync("ended", id);
    live.clear();
  };
  try {
    // Only the `exit` event — deliberately NOT SIGINT/SIGTERM. Calling
    // process.exit() from our own signal listener would run alongside (and
    // could pre-empt) opencode's own signal cleanup, turning a cancel into a
    // kill. A signal that ends the process still fires `exit`; anything that
    // skips `exit` (SIGKILL) is caught by clawlight's owner-PID reap instead.
    process.once("exit", markEnded);
  } catch (_) {
    // process may be sandboxed; the owner-PID reap still covers shutdown
  }

  return {
    // opencode delivers every bus message here as `{ event: { type, properties } }`.
    // Field access is defensive: an unexpected shape drops the event, never throws.
    event: async ({ event }) => {
      try {
        const type = event && event.type;
        const p = (event && event.properties) || {};
        const sid =
          p.sessionID ||
          p.session_id ||
          (p.info && p.info.id) ||
          (p.session && p.session.id);

        switch (type) {
          case "session.created":
            if (sid) {
              live.add(sid);
              send("working", sid, (p.info && p.info.title) || p.title);
            }
            break;
          case "session.updated":
            // Title passthrough only — the Rust side updates the name without
            // touching status.
            if (sid) send("title", sid, (p.info && p.info.title) || p.title);
            break;
          case "session.status":
            // The authoritative working/idle signal. We deliberately do NOT map
            // `message.updated`/`tool.execute.before` to working: opencode emits
            // a trailing `message.updated` (persisting the finished message)
            // *after* it goes idle, which would flip the light back to green.
            // `session.status` is the clean edge — busy while the agent works,
            // idle exactly once when it's done and waiting.
            if (sid && p.status && p.status.type === "busy") send("working", sid);
            else if (sid && p.status && p.status.type === "idle") send("idle", sid);
            break;
          case "session.idle":
            // Belt-and-suspenders alongside `session.status` idle (and the
            // signal `opencode run` emits on completion).
            if (sid) send("idle", sid);
            break;
          case "permission.asked":
            if (sid) send("needs_input", sid);
            break;
          case "permission.replied":
            // The agent continues whether the permission was allowed or denied,
            // so clear the red state either way.
            if (sid) send("resumed", sid);
            break;
          case "session.deleted":
            if (sid) {
              live.delete(sid);
              send("ended", sid);
            }
            break;
          case "session.error":
            // Treat an errored session as needing the user — they usually have
            // to act. Revisit once real payloads are logged (spike item).
            if (sid) send("needs_input", sid);
            break;
          case "server.connected":
            // A fresh server: sweep this harness's stale sessions to done.
            send("reconnected");
            break;
          default:
            break;
        }
      } catch (_) {
        // a shape we didn't expect must never crash opencode
      }
    },
  };
};
