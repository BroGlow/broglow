//! Cross-platform hook backend, invoked by Claude Code as `clawlight hook`.
//!
//! This replaces the old bash + jq hook script: the same binary now handles
//! session-state updates on macOS, Linux, and Windows with no shell or `jq`
//! dependency. The on-disk `state.json` schema is unchanged, so the TUI and
//! the menu-bar daemon are unaffected.

use std::collections::HashMap;
use std::io::Read;

use anyhow::Context;
use chrono::Utc;
use serde_json::Value;

use crate::state::{state_file_path, HookState, SessionStatus, Status};

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

    let field = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string()
    };
    let hook_event = field("hook_event_name");
    let session_id = field("session_id");
    let cwd = field("cwd");
    let notification_type = field("notification_type");
    let transcript_path = field("transcript_path");

    if session_id.is_empty() {
        return Ok(());
    }

    let status = match hook_event.as_str() {
        "SessionStart" | "UserPromptSubmit" | "PreToolUse" => Status::Active,
        "Stop" => Status::Inactive,
        "Notification" => {
            // idle_prompt is an informational nudge, not a help request — skip
            // it so the icon doesn't flip red on idle warnings.
            if notification_type == "idle_prompt" {
                return Ok(());
            }
            Status::NeedsHelp
        }
        "SessionEnd" => Status::Done,
        _ => return Ok(()),
    };

    let mut state = read_state_raw();

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
            project_path: non_empty(cwd),
            notification_type: non_empty(notification_type),
            name: existing_name.clone(),
        },
    );

    write_state_atomic(&state)?;

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

    // CLAWLIGHT_NAMING tells the hooks fired by this nested call to no-op.
    let mut cmd = std::process::Command::new("claude");
    cmd.args(["-p", "--model", "haiku", &prompt])
        .env("CLAWLIGHT_NAMING", "1");

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // The namer runs detached with no console of its own, so without
        // CREATE_NO_WINDOW Windows allocates a fresh console window for the
        // claude CLI — a blank "claude" terminal that flashes on screen.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd.output();

    let name = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => return Ok(()),
    };

    if name.is_empty() {
        return Ok(());
    }

    let mut state = read_state_raw();
    if let Some(s) = state.sessions.get_mut(session_id) {
        s.name = Some(name);
        write_state_atomic(&state)?;
    }
    Ok(())
}

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Read `state.json` without the staleness downgrade that `read_hook_state`
/// applies — the hook must persist exactly what it observes.
fn read_state_raw() -> HookState {
    let path = state_file_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_else(|| HookState {
            sessions: HashMap::new(),
        })
}

/// Atomic write: serialize to a sibling temp file, then rename onto the target.
fn write_state_atomic(state: &HookState) -> anyhow::Result<()> {
    let path = state_file_path();
    let dir = path.parent().context("state path must have a parent")?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".state.{}.tmp", std::process::id()));
    std::fs::write(&tmp, serde_json::to_string(state)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Extract the first user prompt from a JSONL transcript. `content` may be a
/// plain string or an array of content blocks; only `text` blocks are kept.
fn first_user_prompt(transcript_path: &str) -> Option<String> {
    let content = std::fs::read_to_string(transcript_path).ok()?;
    for line in content.lines() {
        let v: Value = match serde_json::from_str(line) {
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

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW | DETACHED_PROCESS — no console flash, survives the
        // parent hook exiting.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }

    let _ = cmd.spawn();
}
