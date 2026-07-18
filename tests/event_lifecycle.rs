//! End-to-end tests of the normalized-event backend: drive the real
//! `clawlight` binary the way the opencode plugin does — one normalized event
//! as JSON on stdin — and observe the resulting `state.json`. Mirrors the
//! strategy of `hook_lifecycle.rs`.
//!
//! Unix-only for the same reason as `hook_lifecycle.rs`: `dirs::home_dir()`
//! follows `$HOME` on unix, which is what sandboxes each test's home.
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

/// Run `clawlight event` inside the sandbox home with the given stdin.
fn run_event(home: &TempDir, stdin: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("clawlight")
        .expect("binary built")
        .arg("event")
        .env("HOME", home.path())
        .write_stdin(stdin.to_string())
        .assert()
}

/// Run `clawlight hook` (the Claude path) inside the sandbox home.
fn run_hook(home: &TempDir, stdin: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("clawlight")
        .expect("binary built")
        .arg("hook")
        .env("HOME", home.path())
        .env_remove("CLAWLIGHT_NAMING")
        .write_stdin(stdin.to_string())
        .assert()
}

/// A normalized opencode event as the plugin sends it.
fn ev(event: &str, session_id: &str) -> String {
    json!({
        "harness": "opencode",
        "event": event,
        "session_id": session_id,
        "directory": "/tmp/oc-project",
    })
    .to_string()
}

fn ev_title(event: &str, session_id: &str, title: &str) -> String {
    json!({
        "harness": "opencode",
        "event": event,
        "session_id": session_id,
        "title": title,
    })
    .to_string()
}

fn read_state(home: &TempDir) -> Value {
    let content = std::fs::read_to_string(state_path(home)).expect("state.json exists");
    serde_json::from_str(&content).expect("state.json parses")
}

fn session_status(state: &Value, session_id: &str) -> String {
    state["sessions"][session_id]["status"]
        .as_str()
        .unwrap_or_else(|| panic!("session {session_id} has a status"))
        .to_string()
}

fn seed_state(home: &TempDir, state: &Value) {
    let path = state_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, state.to_string()).unwrap();
}

#[test]
fn full_lifecycle_maps_normalized_events_to_statuses() {
    let home = temp_home();
    let steps = [
        ("working", "active"),
        ("idle", "inactive"),
        ("needs_input", "needs_help"),
        ("resumed", "active"),
        ("ended", "done"),
    ];
    for (event, expected) in steps {
        run_event(&home, &ev(event, "ses-life")).success();
        assert_eq!(
            session_status(&read_state(&home), "ses-life"),
            expected,
            "after {event}"
        );
    }
}

#[test]
fn permission_ask_flips_red_and_reply_clears_it() {
    let home = temp_home();
    run_event(&home, &ev("working", "ses-perm")).success();
    assert_eq!(session_status(&read_state(&home), "ses-perm"), "active");

    // permission.asked → needs_help (the product's core red moment).
    run_event(&home, &ev("needs_input", "ses-perm")).success();
    assert_eq!(session_status(&read_state(&home), "ses-perm"), "needs_help");

    // permission.replied → active immediately (allow or deny, the agent continues).
    run_event(&home, &ev("resumed", "ses-perm")).success();
    assert_eq!(session_status(&read_state(&home), "ses-perm"), "active");
}

#[test]
fn a_title_sets_the_name_without_changing_status() {
    let home = temp_home();
    run_event(&home, &ev("working", "ses-title")).success();
    run_event(&home, &ev("idle", "ses-title")).success();
    assert_eq!(session_status(&read_state(&home), "ses-title"), "inactive");

    // A pure title update must not flip the idle session back to active.
    run_event(&home, &ev_title("title", "ses-title", "Fix the parser")).success();
    let state = read_state(&home);
    assert_eq!(session_status(&state, "ses-title"), "inactive");
    assert_eq!(state["sessions"]["ses-title"]["name"], "Fix the parser");
}

#[test]
fn a_title_on_the_first_event_becomes_the_name() {
    let home = temp_home();
    run_event(&home, &ev_title("working", "ses-named", "Build the CLI")).success();
    let state = read_state(&home);
    assert_eq!(session_status(&state, "ses-named"), "active");
    assert_eq!(state["sessions"]["ses-named"]["name"], "Build the CLI");

    // A later status-only event preserves the name.
    run_event(&home, &ev("idle", "ses-named")).success();
    assert_eq!(
        read_state(&home)["sessions"]["ses-named"]["name"],
        "Build the CLI"
    );
}

#[test]
fn a_title_for_an_unknown_session_is_a_noop() {
    let home = temp_home();
    run_event(&home, &ev_title("title", "ses-ghost", "No such session")).success();
    // Nothing to update → nothing written.
    assert!(!state_path(&home).exists());
}

#[test]
fn a_working_event_is_suppressed_when_already_active() {
    let home = temp_home();
    let old_timestamp = "2020-01-01T00:00:00Z";
    seed_state(
        &home,
        &json!({
            "sessions": {
                "ses-hot": {
                    "status": "active",
                    "last_updated": old_timestamp,
                    "project_path": null,
                    "notification_type": null,
                    "name": null,
                    "harness": "opencode",
                }
            }
        }),
    );

    run_event(&home, &ev("working", "ses-hot")).success();

    // The seeded timestamp surviving proves the chatty write was suppressed.
    assert_eq!(
        read_state(&home)["sessions"]["ses-hot"]["last_updated"],
        old_timestamp
    );
}

#[test]
fn a_title_event_is_suppressed_when_the_name_is_unchanged() {
    let home = temp_home();
    let old_timestamp = "2020-01-01T00:00:00Z";
    seed_state(
        &home,
        &json!({
            "sessions": {
                "ses-t": {
                    "status": "active",
                    "last_updated": old_timestamp,
                    "project_path": null,
                    "notification_type": null,
                    "name": "Same Title",
                    "harness": "opencode",
                }
            }
        }),
    );

    // opencode re-emits session.updated repeatedly with the same title.
    run_event(&home, &ev_title("title", "ses-t", "Same Title")).success();

    // Unchanged name → no write (seeded timestamp survives).
    assert_eq!(
        read_state(&home)["sessions"]["ses-t"]["last_updated"],
        old_timestamp
    );
}

#[test]
fn unknown_events_and_missing_session_ids_write_nothing() {
    let home = temp_home();
    run_event(&home, &ev("some_future_verb", "ses-x")).success();
    run_event(
        &home,
        &json!({"harness": "opencode", "event": "working"}).to_string(),
    )
    .success();
    run_event(&home, "not json at all {{{").success();

    assert!(!state_path(&home).exists());
}

#[test]
fn an_opencode_session_records_its_harness() {
    let home = temp_home();
    run_event(&home, &ev("working", "ses-oc")).success();

    let state = read_state(&home);
    assert_eq!(state["sessions"]["ses-oc"]["harness"], "opencode");
    assert_eq!(
        state["sessions"]["ses-oc"]["project_path"],
        "/tmp/oc-project"
    );
}

#[test]
fn claude_and_opencode_sessions_coexist_in_one_state_file() {
    let home = temp_home();
    // A Claude Code session via the hook path…
    run_hook(
        &home,
        &json!({
            "hook_event_name": "SessionStart",
            "session_id": "cc-1",
            "cwd": "/tmp/cc-project",
        })
        .to_string(),
    )
    .success();
    // …and an opencode session via the event path, both live at once.
    run_event(&home, &ev("working", "ses-oc")).success();

    let state = read_state(&home);
    assert_eq!(session_status(&state, "cc-1"), "active");
    assert_eq!(session_status(&state, "ses-oc"), "active");
    // Claude sessions carry no harness tag; opencode ones do.
    assert!(
        state["sessions"]["cc-1"].get("harness").is_none(),
        "claude session must not carry a harness field"
    );
    assert_eq!(state["sessions"]["ses-oc"]["harness"], "opencode");
}

#[test]
fn reconnected_sweeps_stale_opencode_sessions_but_spares_other_harnesses() {
    let home = temp_home();
    seed_state(
        &home,
        &json!({
            "sessions": {
                // opencode session with no captured owner PID → treated as
                // stale on reconnect and swept to done.
                "ses-stale": {
                    "status": "active",
                    "last_updated": "2020-01-01T00:00:00Z",
                    "project_path": null,
                    "notification_type": null,
                    "name": null,
                    "harness": "opencode",
                },
                // A Claude session must never be touched by an opencode sweep.
                "cc-1": {
                    "status": "active",
                    "last_updated": "2020-01-01T00:00:00Z",
                    "project_path": null,
                    "notification_type": null,
                    "name": null,
                }
            }
        }),
    );

    run_event(
        &home,
        &json!({"harness": "opencode", "event": "reconnected"}).to_string(),
    )
    .success();

    let state = read_state(&home);
    assert_eq!(session_status(&state, "ses-stale"), "done");
    assert_eq!(session_status(&state, "cc-1"), "active");
}
