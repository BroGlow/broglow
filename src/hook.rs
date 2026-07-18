//! Cross-platform hook backend, invoked by Claude Code as `clawlight hook`.
//!
//! This replaces the old bash + jq hook script: the same binary now handles
//! session-state updates on macOS, Linux, and Windows with no shell or `jq`
//! dependency. The on-disk `state.json` schema is unchanged, so the TUI and
//! the menu-bar daemon are unaffected.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};

use chrono::Utc;
use serde_json::Value;

use crate::state::{
    acquire_state_lock, state_file_path, write_state_atomic, HookState, SessionStatus, Status,
};

/// Outcome of loading `state.json` for a read-modify-write cycle.
enum LoadedState {
    /// No state file yet; safe to start from an empty state.
    Fresh(HookState),
    /// Parsed successfully; safe to modify and write back.
    Ok(HookState),
    /// File exists but couldn't be read or parsed (mid-write from another
    /// process, an unknown future schema, a transient AV lock, etc). The
    /// caller MUST NOT write anything: writing here would silently discard
    /// every other session's status, which is worse than doing nothing.
    Unreadable,
}

/// Locked read-modify-write of `state.json`, shared by every ingestion path:
/// the Claude hook backend ([`run`]), the auto-namer ([`run_namer`]), and the
/// normalized harness-event backend ([`run_event`]).
///
/// Acquires the same `.state.lock` all writers hold — so a concurrent hook from
/// another session can't read a stale snapshot between our read and our rename —
/// loads the current state, and, unless the file is [`LoadedState::Unreadable`]
/// (where writing would wipe every other session's status), hands it to
/// `mutate`. Writes the result back atomically only when `mutate` returns
/// `true`. The lock is held for the whole span and released when this returns.
/// Best-effort locking: if the lock can't be taken we proceed unlocked rather
/// than break ingestion.
fn update_state(mutate: impl FnOnce(&mut HookState) -> bool) -> anyhow::Result<()> {
    let _lock = acquire_state_lock();
    let mut state = match read_state_raw() {
        LoadedState::Fresh(s) | LoadedState::Ok(s) => s,
        LoadedState::Unreadable => return Ok(()),
    };
    if mutate(&mut state) {
        write_state_atomic(&state)?;
    }
    Ok(())
}

/// `clawlight hook`: read one hook event as JSON on stdin and update
/// `state.json`. Mirrors the semantics of the previous `hook.sh` exactly.
pub fn run() -> anyhow::Result<()> {
    // A nested Claude session that clawlight spawns for auto-naming sets this,
    // so its own hooks must not recurse back into state tracking.
    if std::env::var_os("CLAWLIGHT_NAMING").is_some() {
        return Ok(());
    }

    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let v: Value = serde_json::from_str(&input).unwrap_or(Value::Null);

    let field = |k: &str| -> Option<String> {
        v.get(k)
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    };
    let hook_event = field("hook_event_name").unwrap_or_default();
    let session_id = field("session_id");
    let cwd = field("cwd");
    let notification_type = field("notification_type");
    let transcript_path = field("transcript_path").unwrap_or_default();

    let session_id = match session_id {
        Some(id) => id,
        None => return Ok(()),
    };

    let status = match hook_event.as_str() {
        "SessionStart" | "UserPromptSubmit" | "PreToolUse" => Status::Active,
        "Stop" => Status::Inactive,
        "Notification" => {
            // idle_prompt is an informational nudge, not a help request — skip
            // it so the icon doesn't flip red on idle warnings.
            if notification_type.as_deref() == Some("idle_prompt") {
                return Ok(());
            }
            Status::NeedsHelp
        }
        "SessionEnd" => Status::Done,
        _ => return Ok(()),
    };

    // Whether to kick off auto-naming after the write. Decided inside the RMW
    // span (it depends on the pre-existing name) and acted on afterward, off the
    // lock, so the detached spawn never blocks other writers.
    let mut spawn_naming = false;

    update_state(|state| {
        // PreToolUse fires very frequently; skip the write if already active.
        if hook_event == "PreToolUse" {
            if let Some(s) = state.sessions.get(&session_id) {
                if s.status == Status::Active {
                    return false;
                }
            }
        }

        // Preserve an existing name across status updates.
        let existing = state.sessions.get(&session_id);
        let existing_name = existing.and_then(|s| s.name.clone());

        // Where the session runs, for click-to-focus. Captured on SessionStart
        // (a resumed session may live in a new window) and backfilled for
        // sessions first seen mid-flight; otherwise carried over unchanged so
        // routine updates skip the capture's `ps` spawn.
        let terminal = match existing.and_then(|s| s.terminal.clone()) {
            Some(t) if hook_event != "SessionStart" => Some(t),
            _ => Some(crate::terminal::capture()),
        };

        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        state.sessions.insert(
            session_id.clone(),
            SessionStatus {
                status: status.clone(),
                last_updated: timestamp,
                project_path: cwd.clone(),
                notification_type: notification_type.clone(),
                name: existing_name.clone(),
                terminal,
                // Claude Code sessions carry no harness tag (absent = claude).
                harness: None,
            },
        );

        // On the first Stop with no name yet, kick off auto-naming below.
        spawn_naming =
            hook_event == "Stop" && existing_name.is_none() && !transcript_path.is_empty();
        true
    })?;

    // In a detached process so the hook returns immediately instead of blocking
    // on the CLI.
    if spawn_naming {
        spawn_namer(&session_id, &transcript_path);
    }

    Ok(())
}

/// `clawlight event`: read one normalized harness event as JSON on stdin and
/// update `state.json`. This is the ingestion path for non-Claude harnesses
/// (opencode today; Codex/Copilot adapters later emit the same shape). The
/// on-disk schema and every reader (TUI, tray, LED) are shared with the Claude
/// hook path, so a harness session written here shows up everywhere a Claude
/// session does with no reader changes.
///
/// Input shape:
/// ```json
/// {
///   "harness": "opencode",
///   "event": "working | idle | needs_input | resumed | ended | title | reconnected",
///   "session_id": "ses_...",          // required except for `reconnected`
///   "title": "current session title", // optional
///   "directory": "/path/to/project"   // optional
/// }
/// ```
///
/// The five status verbs (`working`/`idle`/`needs_input`/`resumed`/`ended`) are
/// deliberately harness-agnostic; `title` updates the name only, and
/// `reconnected` triggers the restart sweep ([`sweep_reconnected`]).
pub fn run_event() -> anyhow::Result<()> {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let v: Value = serde_json::from_str(&input).unwrap_or(Value::Null);

    let field = |k: &str| -> Option<String> {
        v.get(k)
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    };
    // `harness` is required and never defaulted: a generic ingestion path must
    // not silently attribute a mislabeled event to one specific harness (which
    // would give it the wrong badge and expose it to that harness's sweeps). A
    // missing harness — a buggy adapter, or a bare `reconnected` — is dropped.
    let Some(harness) = field("harness") else {
        return Ok(());
    };
    let event = field("event").unwrap_or_default();
    let session_id = field("session_id");
    let title = field("title");
    let directory = field("directory");

    // `reconnected` is a maintenance sweep, not a per-session status — it
    // carries no session id.
    if event == "reconnected" {
        return sweep_reconnected(&harness);
    }

    let session_id = match session_id {
        Some(id) => id,
        None => return Ok(()),
    };

    // A pure title change updates the name and nothing else: an idle session
    // must not flip green just because opencode renamed it. No-op if we haven't
    // seen the session yet — its `created` (working) event will arrive.
    if event == "title" {
        let Some(title) = title else { return Ok(()) };
        return update_state(|state| match state.sessions.get_mut(&session_id) {
            // opencode re-emits `session.updated` several times per turn with an
            // unchanged title; skip the write unless the name actually changed.
            Some(s) if s.name.as_deref() != Some(title.as_str()) => {
                s.name = Some(title.clone());
                s.last_updated = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                true
            }
            _ => false,
        });
    }

    let status = match event.as_str() {
        "working" | "resumed" => Status::Active,
        "idle" => Status::Inactive,
        "needs_input" => Status::NeedsHelp,
        "ended" => Status::Done,
        // Unknown verb (an opencode event we don't map, or a future harness
        // sending something new): drop it rather than guess.
        _ => return Ok(()),
    };

    update_state(|state| {
        let existing = state.sessions.get(&session_id);

        // `ended` for a session we never saw is a no-op: creating a fresh `Done`
        // row (and paying a `terminal::capture` `ps` to do it) would only add a
        // ghost. The plugin's exit handler can emit `ended` for ids whose first
        // event was dropped.
        if event == "ended" && existing.is_none() {
            return false;
        }

        // `working` is chatty (fires on every message/tool step). Skip the write
        // when nothing would change — the same suppression as the Claude
        // PreToolUse path — so the file watchers don't thrash. A title still
        // forces a write.
        if event == "working" && title.is_none() {
            if let Some(s) = existing {
                if s.status == Status::Active {
                    return false;
                }
            }
        }

        // The harness owns its titles: a provided title becomes the name;
        // otherwise keep whatever name we already have.
        let name = title
            .clone()
            .or_else(|| existing.and_then(|s| s.name.clone()));

        // `directory` is optional in the contract, so preserve the existing
        // project path when an event omits it (as `name`/`terminal` are) — a
        // thin-payload adapter must not blank the project label on every status
        // flip.
        let project_path = directory
            .clone()
            .or_else(|| existing.and_then(|s| s.project_path.clone()));

        // Capture the host terminal once, on first sighting (the `created`
        // event), and carry it forward afterward so chatty events don't each
        // spawn a `ps`. The captured owner PID is what lets the reader reap a
        // harness session whose process exited without an `ended` event.
        let terminal = match existing.and_then(|s| s.terminal.clone()) {
            Some(t) => Some(t),
            None => Some(crate::terminal::capture()),
        };

        state.sessions.insert(
            session_id.clone(),
            SessionStatus {
                status: status.clone(),
                last_updated: Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                project_path,
                notification_type: None,
                name,
                terminal,
                harness: Some(harness.clone()),
            },
        );
        true
    })
}

/// Sweep one harness's still-live sessions to `Done` on a harness restart.
///
/// opencode fires no per-session shutdown event when its process just exits, so
/// a `server.connected` (a fresh opencode server coming up) is our signal that
/// any `Active`/`Inactive` session left over from a previous run is stale. To
/// stay safe when a *second* opencode server is genuinely running at the same
/// time, only sweep sessions whose captured owner process is gone (or was never
/// captured — e.g. `opencode serve` with no controlling tty): a live server's
/// sessions keep a live PID and are left alone. This is the backstop for the
/// plugin's own best-effort exit handler.
///
/// Note: the "spare a live second server" guard relies on `owner_pid`, which is
/// only captured on unix (`terminal::capture_tty_owner`). On a platform that
/// can't identify the owner PID, every same-harness session is treated as stale
/// on reconnect — fine for the common single-instance case, but a concurrent
/// second server's idle sessions would be swept (they self-heal on their next
/// event). A future adapter relying on this must not assume the guard holds off
/// unix.
fn sweep_reconnected(harness: &str) -> anyhow::Result<()> {
    update_state(|state| {
        let mut changed = false;
        for s in state.sessions.values_mut() {
            if s.harness.as_deref() != Some(harness) {
                continue;
            }
            if !matches!(s.status, Status::Active | Status::Inactive) {
                continue;
            }
            let host_gone = s
                .terminal
                .as_ref()
                .and_then(|t| t.owner_pid)
                .map(|pid| !crate::terminal::is_alive(pid))
                .unwrap_or(true);
            if host_gone {
                s.status = Status::Done;
                changed = true;
            }
        }
        changed
    })
}

/// `clawlight name <session_id> <transcript_path>`: generate a concise session
/// title from the transcript via the Claude CLI and write it back to
/// `state.json`. Run detached from the hook so it can take a few seconds.
pub fn run_namer(session_id: &str, transcript_path: &str) -> anyhow::Result<()> {
    let first_prompt = match first_user_prompt(transcript_path) {
        Some(p) if !p.trim().is_empty() => p,
        _ => return Ok(()),
    };

    let prompt = format!(
        "Generate a concise 3-5 word title for this coding session. \
         Output ONLY the title, nothing else. No quotes. User's request: {first_prompt}"
    );

    let output = run_claude_cli(&prompt);

    let name = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => return Ok(()),
    };

    if name.is_empty() {
        return Ok(());
    }

    update_state(|state| match state.sessions.get_mut(session_id) {
        Some(s) => {
            s.name = Some(name);
            true
        }
        None => false,
    })
}

/// Build and run the naming prompt through the `claude` CLI.
///
/// `Command::new("claude")` only resolves `claude.exe` on Windows, not npm's
/// `claude.cmd` shim, so npm installs would silently fail to name sessions.
/// Try the plain name first, and on Windows fall back to the `.cmd` shim if
/// the bare name isn't found.
fn run_claude_cli(prompt: &str) -> std::io::Result<std::process::Output> {
    let build = |program: &str| {
        let mut cmd = std::process::Command::new(program);
        cmd.args(["-p", "--model", "haiku", prompt])
            // CLAWLIGHT_NAMING tells the hooks fired by this nested call to no-op.
            .env("CLAWLIGHT_NAMING", "1")
            // The old bash hook did `unset CLAUDECODE` before invoking a nested
            // `claude` — without it the nested CLI can detect it's running
            // inside a Claude Code session and change behavior.
            .env_remove("CLAUDECODE");
        crate::spawn::configure_detached(&mut cmd);
        cmd
    };

    let result = build("claude").output();

    #[cfg(target_os = "windows")]
    let result = match result {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => build("claude.cmd").output(),
        other => other,
    };

    result
}

/// Read `state.json` without the staleness downgrade that `read_hook_state`
/// applies — the hook must persist exactly what it observes.
fn read_state_raw() -> LoadedState {
    let path = state_file_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return LoadedState::Fresh(HookState::default())
        }
        Err(_) => return LoadedState::Unreadable,
    };
    match serde_json::from_str(&content) {
        Ok(s) => LoadedState::Ok(s),
        Err(_) => LoadedState::Unreadable,
    }
}

/// Extract the first user prompt from a JSONL transcript. `content` may be a
/// plain string or an array of content blocks; only `text` blocks are kept.
/// Streams line-by-line instead of reading the whole file: transcripts can
/// run into the tens of MB, but we only need the first ~500 chars.
fn first_user_prompt(transcript_path: &str) -> Option<String> {
    let file = File::open(transcript_path).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line.ok()?;
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let msg = match v.get("message") {
            Some(m) => m,
            None => continue,
        };
        if msg.get("role").and_then(|r| r.as_str()) != Some("user") {
            continue;
        }
        let text = match msg.get("content") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(arr)) => arr
                .iter()
                .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join(" "),
            _ => continue,
        };
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        // Match the old script: cap to ~500 chars, take the first line.
        let capped: String = text.chars().take(500).collect();
        return Some(capped.lines().next().unwrap_or("").to_string());
    }
    None
}

/// Launch `clawlight name ...` as a detached, windowless child so the hook can
/// return immediately while naming happens in the background.
fn spawn_namer(session_id: &str, transcript_path: &str) {
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut cmd = std::process::Command::new(exe);
    cmd.arg("name")
        .arg(session_id)
        .arg(transcript_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::spawn::configure_detached(&mut cmd);

    let _ = cmd.spawn();
}
