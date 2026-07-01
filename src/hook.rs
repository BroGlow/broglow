//! Cross-platform hook backend, invoked by Claude Code as `clawlight hook`.
//!
//! This replaces the old bash + jq hook script: the same binary now handles
//! session-state updates on macOS, Linux, and Windows with no shell or `jq`
//! dependency. The on-disk `state.json` schema is unchanged, so the TUI and
//! the menu-bar daemon are unaffected.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};

use chrono::Utc;
use fs4::fs_std::FileExt;
use serde_json::Value;

use crate::state::{state_file_path, write_state_atomic, HookState, SessionStatus, Status};

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

    // Hold the lock across the whole read-modify-write span so a concurrent
    // hook from another session can't read a stale snapshot between our read
    // and our rename. Best-effort: if locking fails, proceed unlocked rather
    // than break the hook.
    let _lock = acquire_state_lock();

    let mut state = match read_state_raw() {
        LoadedState::Fresh(s) | LoadedState::Ok(s) => s,
        LoadedState::Unreadable => return Ok(()),
    };

    // PreToolUse fires very frequently; skip the write if already active.
    if hook_event == "PreToolUse" {
        if let Some(s) = state.sessions.get(&session_id) {
            if s.status == Status::Active {
                return Ok(());
            }
        }
    }

    // Preserve an existing name across status updates.
    let existing_name = state.sessions.get(&session_id).and_then(|s| s.name.clone());

    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    state.sessions.insert(
        session_id.clone(),
        SessionStatus {
            status,
            last_updated: timestamp,
            project_path: cwd,
            notification_type,
            name: existing_name.clone(),
        },
    );

    write_state_atomic(&state)?;
    drop(_lock);

    // On the first Stop with no name yet, kick off auto-naming in a detached
    // process so the hook returns immediately instead of blocking on the CLI.
    if hook_event == "Stop" && existing_name.is_none() && !transcript_path.is_empty() {
        spawn_namer(&session_id, &transcript_path);
    }

    Ok(())
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

    let _lock = acquire_state_lock();

    let mut state = match read_state_raw() {
        LoadedState::Fresh(s) | LoadedState::Ok(s) => s,
        LoadedState::Unreadable => return Ok(()),
    };
    if let Some(s) = state.sessions.get_mut(session_id) {
        s.name = Some(name);
        write_state_atomic(&state)?;
    }
    Ok(())
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

/// Take a blocking exclusive lock on `.state.lock` beside `state.json`,
/// guarding the read-modify-write span against concurrent hooks from other
/// sessions. The lock releases automatically when the returned `File` drops.
/// Returns `None` if the lock can't be acquired (e.g. create_dir_all fails);
/// callers proceed unlocked rather than break the hook over this.
fn acquire_state_lock() -> Option<File> {
    let path = state_file_path();
    let dir = path.parent()?;
    std::fs::create_dir_all(dir).ok()?;
    let lock_path = dir.join(".state.lock");
    let file = File::options()
        .create(true)
        .write(true)
        .open(&lock_path)
        .ok()?;
    file.lock_exclusive().ok()?;
    Some(file)
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
