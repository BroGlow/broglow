//! End-to-end tests of the Copilot shim: drive the real `clawlight
//! copilot-hook <event>` the way Copilot CLI's hooks do — the event name on
//! argv, one camelCase JSON payload on stdin — and observe the resulting
//! `state.json`. Mirrors the strategy of `event_lifecycle.rs` /
//! `hook_lifecycle.rs`.
//!
//! Unix-only for the same reason as those suites: `dirs::home_dir()` follows
//! `$HOME` on unix, which is what sandboxes each test's home.
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

/// Run `clawlight copilot-hook <event>` inside the sandbox with the given
/// stdin payload.
fn run_copilot_hook(home: &TempDir, event: &str, stdin: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("clawlight")
        .expect("binary built")
        .args(["copilot-hook", event])
        .env("HOME", home.path())
        .write_stdin(stdin.to_string())
        .assert()
}

/// A Copilot hook payload: camelCase fields, no event name (that's argv's job).
fn payload(session_id: &str) -> String {
    json!({
        "sessionId": session_id,
        "timestamp": 1753113600000u64,
        "cwd": "/tmp/copilot-project",
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
fn copilot_lifecycle_maps_onto_normalized_statuses() {
    let home = temp_home();
    let id = "0198a7c2-4f31-7c11-b7ce-copilot000001";

    run_copilot_hook(&home, "sessionStart", &payload(id)).success();
    let s = session(&read_state(&home), id);
    assert_eq!(s["status"], "active");
    assert_eq!(s["harness"], "copilot");
    assert_eq!(s["project_path"], "/tmp/copilot-project");

    // permissionRequest is the red signal…
    run_copilot_hook(&home, "permissionRequest", &payload(id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "needs_help");

    // …and a bare preToolUse (working) must not clear it…
    run_copilot_hook(&home, "preToolUse", &payload(id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "needs_help");

    // …but the approved tool finishing (postToolUse → resumed) does.
    run_copilot_hook(&home, "postToolUse", &payload(id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "active");

    run_copilot_hook(&home, "agentStop", &payload(id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "inactive");

    // Typing a new prompt re-greens an idle (or red) session.
    run_copilot_hook(&home, "userPromptSubmitted", &payload(id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "active");

    // Copilot has a real session end (interactive quit or one-shot `-p` run).
    run_copilot_hook(&home, "sessionEnd", &payload(id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "done");
}

#[test]
fn unmapped_copilot_events_touch_nothing() {
    let home = temp_home();
    let id = "0198-sub";

    // Events the adapter doesn't register (and future additions) are dropped.
    run_copilot_hook(&home, "subagentStop", &payload(id)).success();
    run_copilot_hook(&home, "preCompact", &payload(id)).success();
    run_copilot_hook(&home, "notification", &payload(id)).success();
    assert!(
        !state_path(&home).exists(),
        "no state written for unmapped events"
    );

    // Malformed payloads (no session id / not JSON) are dropped quietly too.
    run_copilot_hook(&home, "agentStop", r#"{"timestamp":1753113600000}"#).success();
    run_copilot_hook(&home, "sessionStart", "not json").success();
    assert!(!state_path(&home).exists());

    // A sessionEnd for a session we never saw must not create a ghost row.
    run_copilot_hook(&home, "sessionEnd", &payload("never-seen")).success();
    assert!(!state_path(&home).exists());
}

#[test]
fn first_prompt_names_the_session_once() {
    let home = temp_home();
    let id = "0198-name";

    run_copilot_hook(&home, "sessionStart", &payload(id)).success();

    // The first typed prompt names the session — from the payload itself, no
    // transcript reads. Multi-line prompts contribute their first line only.
    let first = json!({
        "sessionId": id,
        "cwd": "/tmp/copilot-project",
        "prompt": "compare copilot with claude code please\nand be thorough",
    })
    .to_string();
    run_copilot_hook(&home, "userPromptSubmitted", &first).success();
    let s = session(&read_state(&home), id);
    assert_eq!(s["name"], "compare copilot with claude code please");
    assert_eq!(s["status"], "active");

    // A later prompt never renames.
    let second = json!({
        "sessionId": id,
        "cwd": "/tmp/copilot-project",
        "prompt": "now fix the flaky test",
    })
    .to_string();
    run_copilot_hook(&home, "userPromptSubmitted", &second).success();
    assert_eq!(
        session(&read_state(&home), id)["name"],
        "compare copilot with claude code please"
    );
}

#[test]
fn one_shot_runs_are_named_by_their_initial_prompt_and_end() {
    let home = temp_home();
    let id = "0198-exec";

    // `copilot -p "..."` delivers the prompt on sessionStart.
    let start = json!({
        "sessionId": id,
        "cwd": "/tmp/copilot-project",
        "initialPrompt": "summarize the failing CI runs",
    })
    .to_string();
    run_copilot_hook(&home, "sessionStart", &start).success();
    let s = session(&read_state(&home), id);
    assert_eq!(s["status"], "active");
    assert_eq!(s["name"], "summarize the failing CI runs");

    run_copilot_hook(&home, "sessionEnd", &payload(id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "done");
}

#[test]
fn snake_case_payload_fields_are_tolerated() {
    let home = temp_home();
    let id = "0198-snake";

    // PascalCase-registered events would deliver snake_case payloads; a
    // hand-edited registration must not strand its sessions.
    let start = json!({
        "session_id": id,
        "cwd": "/tmp/copilot-project",
        "initial_prompt": "port the adapter",
    })
    .to_string();
    run_copilot_hook(&home, "sessionStart", &start).success();
    let s = session(&read_state(&home), id);
    assert_eq!(s["status"], "active");
    assert_eq!(s["name"], "port the adapter");
}
