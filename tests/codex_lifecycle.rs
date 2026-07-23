//! End-to-end tests of the Codex shim: drive the real `clawlight codex-hook`
//! the way Codex's hooks do — one Claude-dialect hook payload as JSON on
//! stdin — and observe the resulting `state.json`. Mirrors the strategy of
//! `event_lifecycle.rs` / `hook_lifecycle.rs`.
//!
//! Unix-only for the same reason as those suites: `dirs::home_dir()` follows
//! `$HOME` on unix, which is what sandboxes each test's home. `$CODEX_HOME`
//! sandboxes the Codex side (session_index.jsonl, rollouts).
#![cfg(unix)]

use std::path::PathBuf;

use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;

fn temp_home() -> TempDir {
    TempDir::new().expect("create temp home")
}

fn state_path(home: &TempDir) -> PathBuf {
    home.path()
        .join(".claude")
        .join("clawlight")
        .join("state.json")
}

fn codex_home(home: &TempDir) -> PathBuf {
    home.path().join("codex-home")
}

/// Run `clawlight codex-hook` inside the sandbox with the given stdin.
fn run_codex_hook(home: &TempDir, stdin: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("clawlight")
        .expect("binary built")
        .arg("codex-hook")
        .env("HOME", home.path())
        .env("CODEX_HOME", codex_home(home))
        .write_stdin(stdin.to_string())
        .assert()
}

/// A Codex hook payload (the Claude dialect Codex speaks).
fn ev(hook_event: &str, session_id: &str) -> String {
    json!({
        "hook_event_name": hook_event,
        "session_id": session_id,
        "cwd": "/tmp/codex-project",
    })
    .to_string()
}

fn ev_with_transcript(hook_event: &str, session_id: &str, transcript: &str) -> String {
    json!({
        "hook_event_name": hook_event,
        "session_id": session_id,
        "cwd": "/tmp/codex-project",
        "transcript_path": transcript,
    })
    .to_string()
}

fn read_state(home: &TempDir) -> Value {
    let content = std::fs::read_to_string(state_path(home)).expect("state.json exists");
    serde_json::from_str(&content).expect("state.json parses")
}

fn session(state: &Value, session_id: &str) -> Value {
    state["sessions"][session_id].clone()
}

#[test]
fn codex_lifecycle_maps_onto_normalized_statuses() {
    let home = temp_home();
    let id = "019f7fa3-cff8-7962-956f-917245c8d037";

    run_codex_hook(&home, &ev("SessionStart", id)).success();
    let s = session(&read_state(&home), id);
    assert_eq!(s["status"], "active");
    assert_eq!(s["harness"], "codex");
    assert_eq!(s["project_path"], "/tmp/codex-project");

    // PermissionRequest is the red signal…
    run_codex_hook(&home, &ev("PermissionRequest", id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "needs_help");

    // …and a bare PreToolUse (working) must not clear it…
    run_codex_hook(&home, &ev("PreToolUse", id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "needs_help");

    // …but the approved tool finishing (PostToolUse → resumed) does.
    run_codex_hook(&home, &ev("PostToolUse", id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "active");

    run_codex_hook(&home, &ev("Stop", id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "inactive");

    // Typing a new prompt re-greens an idle (or red) session.
    run_codex_hook(&home, &ev("UserPromptSubmit", id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "active");
}

#[test]
fn unmapped_codex_events_touch_nothing() {
    let home = temp_home();
    let id = "019f-sub";

    run_codex_hook(&home, &ev("SubagentStop", id)).success();
    run_codex_hook(&home, &ev("PreCompact", id)).success();
    assert!(
        !state_path(&home).exists(),
        "no state written for unmapped events"
    );

    // Malformed payloads (no session id / not JSON) are dropped quietly too.
    run_codex_hook(&home, r#"{"hook_event_name":"Stop"}"#).success();
    run_codex_hook(&home, "not json").success();
    assert!(!state_path(&home).exists());
}

#[test]
fn exec_rollouts_end_on_stop_interactive_ones_idle() {
    let home = temp_home();
    let sessions_dir = codex_home(&home).join("sessions").join("2026/07/20");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    // One-shot `codex exec` rollout: Stop is the end of the session.
    let exec_rollout = sessions_dir.join("rollout-exec.jsonl");
    std::fs::write(
        &exec_rollout,
        r#"{"timestamp":"2026-07-20T13:40:17.039Z","type":"session_meta","payload":{"session_id":"e1","cwd":"/tmp","originator":"codex_exec","source":"exec"}}
"#,
    )
    .unwrap();
    let exec_transcript = exec_rollout.display().to_string();
    run_codex_hook(&home, &ev("SessionStart", "e1")).success();
    run_codex_hook(&home, &ev_with_transcript("Stop", "e1", &exec_transcript)).success();
    assert_eq!(session(&read_state(&home), "e1")["status"], "done");

    // Interactive rollout (tui/vscode source): Stop just pauses.
    let tui_rollout = sessions_dir.join("rollout-tui.jsonl");
    std::fs::write(
        &tui_rollout,
        r#"{"timestamp":"2026-07-20T13:08:52.675Z","type":"session_meta","payload":{"session_id":"i1","cwd":"/tmp","originator":"Codex Desktop","source":"vscode"}}
"#,
    )
    .unwrap();
    let tui_transcript = tui_rollout.display().to_string();
    run_codex_hook(&home, &ev("SessionStart", "i1")).success();
    run_codex_hook(&home, &ev_with_transcript("Stop", "i1", &tui_transcript)).success();
    assert_eq!(session(&read_state(&home), "i1")["status"], "inactive");
}

#[test]
fn naming_prefers_codex_thread_titles_over_first_prompt() {
    let home = temp_home();
    let id = "019f-name";
    let sessions_dir = codex_home(&home).join("sessions").join("2026/07/20");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    // Rollout with a typed user message but (as yet) no thread name.
    let rollout = sessions_dir.join("rollout-name.jsonl");
    std::fs::write(
        &rollout,
        concat!(
            r#"{"timestamp":"2026-07-20T13:08:52.675Z","type":"session_meta","payload":{"session_id":"019f-name","cwd":"/tmp","originator":"Codex Desktop","source":"vscode"}}"#,
            "\n",
            r#"{"timestamp":"2026-07-20T13:08:52.681Z","type":"event_msg","payload":{"type":"user_message","message":"compare codex with claude code please"}}"#,
            "\n",
        ),
    )
    .unwrap();
    let transcript = rollout.display().to_string();

    run_codex_hook(&home, &ev("SessionStart", id)).success();
    run_codex_hook(&home, &ev_with_transcript("Stop", id, &transcript)).success();
    assert_eq!(
        session(&read_state(&home), id)["name"],
        "compare codex with claude code please"
    );

    // Codex later titles the thread: the thread name wins on the next turn
    // boundary.
    std::fs::write(
        codex_home(&home).join("session_index.jsonl"),
        r#"{"id":"019f-name","thread_name":"Compare Codex with Claude Code","updated_at":"2026-07-20T13:09:00Z"}
"#,
    )
    .unwrap();
    run_codex_hook(&home, &ev_with_transcript("Stop", id, &transcript)).success();
    assert_eq!(
        session(&read_state(&home), id)["name"],
        "Compare Codex with Claude Code"
    );
}
