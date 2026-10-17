//! Cross-platform hook backend, invoked by Claude Code as `broglow hook`.
//!
//! This replaces the old bash + jq hook script: the same binary now handles
//! session-state updates on macOS, Linux, and Windows with no shell or `jq`
//! dependency. The on-disk `state.json` schema is unchanged, so the TUI and
//! the menu-bar daemon are unaffected.

use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};

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
/// If locking fails, skip this update rather than risk clobbering another writer.
fn update_state(mutate: impl FnOnce(&mut HookState) -> bool) -> anyhow::Result<()> {
    let Some(_lock) = acquire_state_lock() else {
        return Ok(());
    };
    let mut state = match read_state_raw() {
        LoadedState::Fresh(s) | LoadedState::Ok(s) => s,
        LoadedState::Unreadable => return Ok(()),
    };
    if mutate(&mut state) {
        write_state_atomic(&state)?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
#[test]
fn startup_backfill_titles_only_unnamed_live_claude_sessions() {
    // Isolate the real filesystem path in a child, never change this runner's HOME.
    if std::env::var_os("BROGLOW_TEST_BACKFILL").is_none() {
        let home = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "hook::startup_backfill_titles_only_unnamed_live_claude_sessions",
                "--nocapture",
            ])
            .env("HOME", home.path())
            .env("BROGLOW_TEST_BACKFILL", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let project = dirs::home_dir()
        .unwrap()
        .join(".claude")
        .join("projects")
        .join("-work-app");
    std::fs::create_dir_all(&project).unwrap();
    for id in ["idle", "named", "done", "codex"] {
        std::fs::write(
            project.join(format!("{id}.jsonl")),
            r#"{"type":"custom-title","customTitle":"Claude Title"}"#,
        )
        .unwrap();
    }
    std::fs::write(
        project.join("untitled.jsonl"),
        r#"{"message":{"role":"user","content":"hi"}}"#,
    )
    .unwrap();

    let session = |status: &str, name: Option<&str>, harness: Option<&str>| {
        serde_json::json!({
            "status": status, "last_updated": "2026-01-01T00:00:00Z",
            "project_path": "/work/app", "notification_type": null,
            "name": name, "harness": harness,
        })
    };
    let state = serde_json::json!({ "sessions": {
        "idle": session("inactive", None, None),
        "named": session("inactive", Some("Kept"), None),
        "done": session("done", None, None),
        "codex": session("inactive", None, Some("codex")),
        "untitled": session("inactive", None, None),
        "missing": session("active", None, None),
    }});
    std::fs::create_dir_all(state_file_path().parent().unwrap()).unwrap();
    std::fs::write(state_file_path(), state.to_string()).unwrap();

    backfill_claude_titles().unwrap();

    let after: Value =
        serde_json::from_str(&std::fs::read_to_string(state_file_path()).unwrap()).unwrap();
    let name = |id: &str| after["sessions"][id]["name"].clone();
    assert_eq!(name("idle"), "Claude Title");
    assert_eq!(name("named"), "Kept", "Never overwrite a name");
    for id in ["done", "codex", "untitled", "missing"] {
        assert_eq!(name(id), Value::Null, "{id} stays unnamed");
    }
    assert!(find_claude_transcript("../escape").is_none());
}

#[cfg(all(test, unix))]
#[test]
fn dismissed_session_returns_on_activity() {
    // Isolate the real filesystem path in a child, never change this runner's HOME.
    if std::env::var_os("BROGLOW_TEST_DISMISS").is_none() {
        let home = tempfile::tempdir().unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "hook::dismissed_session_returns_on_activity",
                "--nocapture",
            ])
            .env("HOME", home.path())
            .env("BROGLOW_TEST_DISMISS", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let read = || -> Value {
        serde_json::from_str(&std::fs::read_to_string(state_file_path()).unwrap()).unwrap()
    };
    let event = |id, event| {
        ingest_event(&serde_json::json!({
            "harness": "codex", "session_id": id, "event": event,
            "title": "Keep this conversation", "directory": "/project"
        }))
        .unwrap()
    };
    crate::state::clear_session("missing").unwrap();
    crate::state::clear_paused_sessions().unwrap();
    assert!(!state_file_path().exists());
    event("working", "working");
    event("paused", "idle");
    let original = read();
    crate::state::clear_session("paused").unwrap();
    assert_eq!(
        read()["sessions"]["working"],
        original["sessions"]["working"]
    );
    assert!(read()["sessions"].get("paused").is_none());
    assert_eq!(
        crate::state::aggregate(
            &crate::state::read_hook_state(),
            crate::config::YellowMode::AnyInactive
        ),
        crate::state::Aggregate::Green
    );
    crate::state::clear_session("paused").unwrap(); // Repeated clicks are harmless.
    event("paused", "resumed");
    assert_eq!(read()["sessions"]["paused"]["status"], "active");
    assert_eq!(read()["sessions"]["paused"]["harness"], "codex");
    for status in ["working", "needs_input", "idle"] {
        event("paused", status);
        crate::state::clear_session("paused").unwrap();
        assert!(read()["sessions"].get("paused").is_none());
    }
    crate::state::clear_session("working").unwrap();
    assert_eq!(
        crate::state::aggregate(
            &crate::state::read_hook_state(),
            crate::config::YellowMode::AnyInactive
        ),
        crate::state::Aggregate::None
    );
    event("paused-1", "idle");
    event("paused-2", "idle");
    event("resumed", "idle");
    event("resumed", "resumed"); // Resumed after the UI showed it as paused.
    event("working", "working");
    event("approval", "needs_input");
    event("finished", "working");
    event("finished", "ended");
    let original = read();
    crate::state::clear_paused_sessions().unwrap();
    let cleared = read();
    assert!(cleared["sessions"].get("paused-1").is_none());
    assert!(cleared["sessions"].get("paused-2").is_none());
    for id in ["resumed", "working", "approval", "finished"] {
        assert_eq!(cleared["sessions"][id], original["sessions"][id]);
    }
    crate::state::clear_paused_sessions().unwrap();
    assert_eq!(read(), cleared);
    event("paused-1", "resumed");
    assert_eq!(read()["sessions"]["paused-1"]["status"], "active");
    std::fs::write(state_file_path(), "invalid JSON").unwrap();
    assert!(crate::state::clear_session("paused").is_err());
    assert!(crate::state::clear_paused_sessions().is_err());
    assert_eq!(
        std::fs::read_to_string(state_file_path()).unwrap(),
        "invalid JSON"
    );
}

/// A desktop-hosted session whose stored identity predates capturing the
/// desktop app's session ID.
fn host_id_missing(t: &crate::state::TerminalInfo) -> bool {
    t.host_session_id.is_none() && std::env::var_os("CLAUDE_CODE_HOST_SESSION_ID").is_some()
}

/// `broglow hook`: read one hook event as JSON on stdin and update
/// `state.json`. Mirrors the semantics of the previous `hook.sh` exactly.
pub fn run() -> anyhow::Result<()> {
    // A nested Claude session that broglow spawns for auto-naming sets this,
    // so its own hooks must not recurse back into state tracking.
    if std::env::var_os("BROGLOW_NAMING").is_some() {
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

    // Claude's own session title (the desktop app's auto-title, or `/rename`),
    // re-read at turn boundaries so renames follow, and on every event while
    // the session is unnamed: the desktop app titles a session a few records
    // into its first turn, which can run for many minutes of tool calls. Read
    // before taking the lock: it's file IO on a transcript that can run into
    // the tens of MB.
    let read_title = !transcript_path.is_empty()
        && match hook_event.as_str() {
            "SessionStart" | "UserPromptSubmit" | "Stop" => true,
            _ => is_unnamed(&session_id),
        };
    let claude_title = if read_title {
        transcript_title(&transcript_path)
    } else {
        None
    };

    // Whether to kick off auto-naming after the write. Decided inside the RMW
    // span (it depends on the pre-existing name) and acted on afterward, off the
    // lock, so the detached spawn never blocks other writers.
    let mut spawn_naming = false;

    update_state(|state| {
        // PreToolUse fires very frequently; skip the write if already active,
        // unless it brings the session's first title.
        if hook_event == "PreToolUse" {
            if let Some(s) = state.sessions.get(&session_id) {
                if s.status == Status::Active
                    && (claude_title.is_none() || s.name.is_some())
                    && !s.terminal.as_ref().is_some_and(host_id_missing)
                {
                    return false;
                }
            }
        }

        // Claude owns its titles; otherwise preserve an existing name across
        // status updates.
        let existing = state.sessions.get(&session_id);
        let name = claude_title
            .clone()
            .or_else(|| existing.and_then(|s| s.name.clone()));

        // Where the session runs, for click-to-focus. Captured on SessionStart
        // (a resumed session may live in a new window) and backfilled for
        // sessions first seen mid-flight; otherwise carried over unchanged so
        // routine updates skip the capture's `ps` spawn. Entries recorded
        // before the desktop session ID was captured are refreshed once.
        let terminal = match existing.and_then(|s| s.terminal.clone()) {
            Some(t) if hook_event != "SessionStart" && !host_id_missing(&t) => Some(t),
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
                name: name.clone(),
                terminal,
                // Claude Code sessions carry no harness tag (absent = claude).
                harness: None,
            },
        );

        // On the first Stop with no name yet, kick off auto-naming below.
        spawn_naming = hook_event == "Stop" && name.is_none() && !transcript_path.is_empty();
        true
    })?;

    // In a detached process so the hook returns immediately instead of blocking
    // on the CLI.
    if spawn_naming {
        spawn_namer(&session_id, &transcript_path);
    }

    Ok(())
}

/// `broglow event`: read one normalized harness event as JSON on stdin and
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
    ingest_event(&v)
}

/// Apply one normalized harness event (see [`run_event`] for the shape).
/// Shared by the stdin backend and the in-process shims — Codex
/// ([`run_codex_hook`], translating Claude-dialect payloads) and Copilot
/// ([`run_copilot_hook`], translating per-event payloads) — which build the
/// same shape.
fn ingest_event(v: &Value) -> anyhow::Result<()> {
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

        // A pending permission (`NeedsHelp`) is the product's core red signal.
        // A bare `working` — e.g. a `session.status: busy` re-broadcast when a
        // client attaches to the server — must not silently clear it; only an
        // explicit `resumed` (permission.replied) does. So keep red until then.
        if event == "working" {
            if let Some(s) = existing {
                if s.status == Status::NeedsHelp {
                    return false;
                }
            }
        }

        // `working` is chatty (fires on every message/tool step), and the
        // Codex/Copilot shims emit `resumed` on every PostToolUse. Skip the write
        // when nothing would change — the same suppression as the Claude
        // PreToolUse path — so the file watchers don't thrash. A title still
        // forces a write. (The needs-help guard above already ran, so a
        // `resumed` that actually clears red always gets here with a
        // non-Active status and writes.)
        if matches!(event.as_str(), "working" | "resumed") && title.is_none() {
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

/// `broglow codex-hook`: the Codex shim. Codex (>= 0.144) fires hooks in
/// Claude Code's dialect — same stdin JSON shape — from the entries the codex
/// adapter registers in `$CODEX_HOME/hooks.json`. This reads one such event,
/// maps it onto the normalized verbs, and feeds [`ingest_event`], so Codex
/// sessions get the shared semantics (sticky red, owner-pid reaping, badges)
/// without a JS plugin.
///
/// Mapping notes:
/// - `PermissionRequest` is Codex's waiting-on-approval signal → `needs_input`
///   (Codex has no `Notification`).
/// - `PostToolUse` → `resumed`: the first tool completion after an approval is
///   what clears the red; a plain `working` deliberately would not.
/// - `UserPromptSubmit` → `resumed`: the user typing is also them dealing
///   with a pending request (e.g. rejecting the tool and redirecting).
/// - `Stop` → `idle`, except for one-shot `codex exec` rollouts → `ended`
///   (nobody resumes those; Codex has no `SessionEnd`).
/// - Titles: Codex names its own threads (`session_index.jsonl`); the thread
///   name rides along as the event title. Unnamed sessions use the submitted
///   prompt immediately, with the rollout at `Stop` as a fallback for older hooks.
pub fn run_codex_hook() -> anyhow::Result<()> {
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
    let Some(session_id) = field("session_id") else {
        return Ok(());
    };
    let cwd = field("cwd");
    let transcript_path = field("transcript_path").unwrap_or_default();

    let exec = hook_event == "Stop"
        && !transcript_path.is_empty()
        && crate::codex::rollout_is_exec(&transcript_path);
    let Some(event) = codex_event_verb(&hook_event, exec) else {
        return Ok(());
    };

    // Codex's own thread title, when it has one — resolved at turn boundaries
    // only (the index is tiny, but Pre/PostToolUse fire constantly).
    let title = matches!(
        hook_event.as_str(),
        "SessionStart" | "UserPromptSubmit" | "Stop"
    )
    .then(|| crate::codex::thread_names().remove(&session_id))
    .flatten();

    let mut ev = serde_json::json!({
        "harness": "codex",
        "event": event,
        "session_id": session_id,
    });
    if let Some(t) = &title {
        ev["title"] = Value::String(t.clone());
    }
    if let Some(c) = &cwd {
        ev["directory"] = Value::String(c.clone());
    }
    ingest_event(&ev)?;

    // Prompt hooks also work for ephemeral chats, which have no rollout.
    // Name once; later Codex titles still win through the title path above.
    if title.is_none() {
        let prompt = match hook_event.as_str() {
            "UserPromptSubmit" => field("prompt"),
            "Stop" if !transcript_path.is_empty() => {
                crate::codex::first_user_message(&transcript_path)
            }
            _ => None,
        };
        if let Some(prompt) = prompt {
            let prompt = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
            if prompt.is_empty() {
                return Ok(());
            }
            let name = crate::session::truncate(&prompt, 50);
            update_state(|state| match state.sessions.get_mut(&session_id) {
                Some(s) if s.name.is_none() => {
                    s.name = Some(name.clone());
                    true
                }
                _ => false,
            })?;
        }
    }

    Ok(())
}

/// Normalized verb for one Codex hook event; `None` for events that must not
/// touch state (subagent/compaction events, future additions).
fn codex_event_verb(hook_event: &str, exec: bool) -> Option<&'static str> {
    match hook_event {
        "SessionStart" | "PreToolUse" => Some("working"),
        "UserPromptSubmit" | "PostToolUse" => Some("resumed"),
        "PermissionRequest" => Some("needs_input"),
        "Stop" => Some(if exec { "ended" } else { "idle" }),
        _ => None,
    }
}

/// `broglow copilot-hook <event>`: the GitHub Copilot CLI shim. The copilot
/// adapter (see copilot.rs) registers this command per lifecycle event in
/// `$COPILOT_HOME/hooks/broglow.json`; Copilot pipes one JSON payload on
/// stdin. The payload does not name its event — that arrives as `event` from
/// argv — so this reads the payload, maps the pair onto the normalized verbs,
/// and feeds [`ingest_event`], giving Copilot sessions the shared semantics
/// (sticky red, owner-pid reaping, badges) without a plugin.
///
/// Mapping notes:
/// - `permissionRequest` is Copilot's waiting-on-approval signal →
///   `needs_input` (its `notification` event is unregistered: an undocumented
///   type vocabulary that could flip the icon red on informational nudges).
/// - `postToolUse` → `resumed`: the first tool completion after an approval
///   is what clears the red; a plain `working` deliberately would not.
/// - `userPromptSubmitted` → `resumed`: the user typing is also them dealing
///   with a pending request (e.g. rejecting the tool and redirecting).
/// - `agentStop` → `idle`, `sessionEnd` → `ended`: Copilot has a real
///   session-end event (unlike Codex), so one-shot `copilot -p` runs and
///   interactive quits both end cleanly with no rollout sniffing.
/// - Titles: Copilot's own AI-generated session names live in its SQLite
///   session store, which we don't read; instead the first typed prompt —
///   carried right in the payload (`prompt` / `initialPrompt`) — becomes the
///   name, once, while the session is still unnamed.
pub fn run_copilot_hook(event_name: &str) -> anyhow::Result<()> {
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let v: Value = serde_json::from_str(&input).unwrap_or(Value::Null);

    let field = |k: &str| -> Option<String> {
        v.get(k)
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    };
    // camelCase-registered events get camelCase payloads; tolerate the
    // snake_case spelling too in case the registration is ever hand-edited to
    // the PascalCase aliases (whose payloads come snake_cased).
    let Some(session_id) = field("sessionId").or_else(|| field("session_id")) else {
        return Ok(());
    };
    let cwd = field("cwd");

    let Some(event) = copilot_event_verb(event_name) else {
        return Ok(());
    };

    let mut ev = serde_json::json!({
        "harness": "copilot",
        "event": event,
        "session_id": session_id,
    });
    if let Some(c) = &cwd {
        ev["directory"] = Value::String(c.clone());
    }
    ingest_event(&ev)?;

    // Fallback naming: the prompt rides in the payload itself, so the first
    // one we see names a still-unnamed session (a later prompt never renames).
    let prompt = match event_name {
        "userPromptSubmitted" => field("prompt"),
        "sessionStart" => field("initialPrompt").or_else(|| field("initial_prompt")),
        _ => None,
    };
    if let Some(prompt) = prompt {
        let first_line = prompt.trim().lines().next().unwrap_or("").to_string();
        if !first_line.is_empty() {
            let name = crate::session::truncate(&first_line, 50);
            update_state(|state| match state.sessions.get_mut(&session_id) {
                Some(s) if s.name.is_none() => {
                    s.name = Some(name.clone());
                    true
                }
                _ => false,
            })?;
        }
    }

    Ok(())
}

/// Normalized verb for one Copilot lifecycle event (as registered in
/// copilot.rs, camelCase); `None` for events that must not touch state
/// (subagent/compaction events, future additions).
fn copilot_event_verb(event_name: &str) -> Option<&'static str> {
    match event_name {
        "sessionStart" | "preToolUse" => Some("working"),
        "userPromptSubmitted" | "postToolUse" => Some("resumed"),
        "permissionRequest" => Some("needs_input"),
        "agentStop" => Some("idle"),
        "sessionEnd" => Some("ended"),
        _ => None,
    }
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

/// `broglow name <session_id> <transcript_path>`: generate a concise session
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

    let generated = match run_claude_cli(&prompt) {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => String::new(),
    };

    // The CLI fails when it isn't logged in (e.g. only the desktop app is), so
    // fall back to the prompt itself, as the Codex and Copilot adapters do.
    let name = if generated.is_empty() {
        crate::session::truncate(&first_prompt, 50)
    } else {
        generated
    };

    update_state(|state| match state.sessions.get_mut(session_id) {
        // Claude's own title may have landed while the CLI ran; never clobber it.
        Some(s) if s.name.is_none() => {
            s.name = Some(name);
            true
        }
        _ => false,
    })
}

/// Give live Claude sessions that are still unnamed Claude's own title from
/// their transcripts. Hooks read it only while the session is working, so an
/// idle session would show `Session <id>` until its next prompt. The tray runs this once at
/// startup; best-effort, and never overwrites a name.
pub fn backfill_claude_titles() -> anyhow::Result<()> {
    let LoadedState::Ok(state) = read_state_raw() else {
        return Ok(());
    };
    let titles: Vec<(String, String)> = state
        .sessions
        .iter()
        .filter(|(_, s)| s.harness.is_none() && s.name.is_none() && s.status != Status::Done)
        .filter_map(|(id, _)| {
            let path = find_claude_transcript(id)?;
            Some((id.clone(), transcript_title(path.to_str()?)?))
        })
        .collect();
    if titles.is_empty() {
        return Ok(());
    }
    update_state(|state| {
        let mut changed = false;
        for (id, title) in &titles {
            if let Some(s) = state.sessions.get_mut(id) {
                if s.name.is_none() {
                    s.name = Some(title.clone());
                    changed = true;
                }
            }
        }
        changed
    })
}

/// `~/.claude/projects/<project dir>/<id>.jsonl`. The directory is Claude's
/// own encoding of the working directory, so look the file up rather than
/// re-derive it. IDs must be plain names; nothing else is joined into a path.
fn find_claude_transcript(session_id: &str) -> Option<std::path::PathBuf> {
    let plain = !session_id.is_empty()
        && session_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !plain {
        return None;
    }
    let file = format!("{session_id}.jsonl");
    std::fs::read_dir(dirs::home_dir()?.join(".claude").join("projects"))
        .ok()?
        .flatten()
        .map(|entry| entry.path().join(&file))
        .find(|path| path.is_file())
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
        // Without --no-session-persistence every naming call would leave a
        // "Generate a concise title..." session in the user's history.
        cmd.args(["-p", "--no-session-persistence", "--model", "haiku", prompt])
            // BROGLOW_NAMING tells the hooks fired by this nested call to no-op.
            .env("BROGLOW_NAMING", "1")
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

/// Whether the session has no name yet. An unlocked peek that only decides
/// whether a transcript read is worth it; the write itself re-checks under the
/// lock. An unreadable state file counts as named, so nothing extra happens.
fn is_unnamed(session_id: &str) -> bool {
    match read_state_raw() {
        LoadedState::Ok(state) => state
            .sessions
            .get(session_id)
            .is_none_or(|s| s.name.is_none()),
        LoadedState::Fresh(_) => true,
        LoadedState::Unreadable => false,
    }
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
        let Ok(line) = line else { continue };
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
        let text = strip_system_reminders(&text);
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

/// Drop `<system-reminder>…</system-reminder>` blocks the host injects ahead
/// of what the user typed (the desktop app's scratch-workspace notice), so
/// they don't become the session's name. An unterminated block drops the rest.
fn strip_system_reminders(text: &str) -> String {
    const OPEN: &str = "<system-reminder>";
    const CLOSE: &str = "</system-reminder>";
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find(OPEN) {
        out.push_str(&rest[..start]);
        rest = match rest[start..].find(CLOSE) {
            Some(end) => &rest[start + end + CLOSE.len()..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// How far back from the end of a transcript to look for Claude's title.
/// Claude re-appends its `custom-title` record every turn, so it sits near the
/// end; a turn with more tool output than this is missed until the next one.
const TITLE_TAIL_BYTES: u64 = 1024 * 1024;

/// Claude's own title for a session: the latest `custom-title` record in its
/// transcript, written by the desktop app's auto-titling and by `/rename`.
/// Only the tail is read — the file can be tens of MB.
fn transcript_title(transcript_path: &str) -> Option<String> {
    let mut file = File::open(transcript_path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(TITLE_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).ok()?;
    let tail = String::from_utf8_lossy(&buf);

    // Mid-file, the first line is a fragment; skip it.
    let lines = tail.lines().skip(usize::from(start > 0));
    lines
        .filter(|line| line.contains("custom-title"))
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v.get("type").and_then(|t| t.as_str()) == Some("custom-title"))
        .filter_map(|v| {
            v.get("customTitle")
                .and_then(|t| t.as_str())
                .map(|t| t.trim().to_string())
        })
        .filter(|t| !t.is_empty())
        .last()
}

/// Launch `broglow name ...` as a detached, windowless child so the hook can
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
