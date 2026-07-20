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
fn a_working_event_does_not_clear_a_pending_permission() {
    let home = temp_home();
    run_event(&home, &ev("working", "ses-p")).success();
    run_event(&home, &ev("needs_input", "ses-p")).success();
    assert_eq!(session_status(&read_state(&home), "ses-p"), "needs_help");

    // A bare `working` (e.g. a `session.status: busy` re-broadcast when another
    // client attaches to the server) must NOT silently clear the red — the
    // product's core signal. Only `resumed` (permission.replied) clears it.
    run_event(&home, &ev("working", "ses-p")).success();
    assert_eq!(
        session_status(&read_state(&home), "ses-p"),
        "needs_help",
        "working must not clear a pending permission"
    );

    // `resumed` still clears it.
    run_event(&home, &ev("resumed", "ses-p")).success();
    assert_eq!(session_status(&read_state(&home), "ses-p"), "active");
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

#[test]
fn an_event_without_a_harness_is_dropped() {
    let home = temp_home();
    // A generic ingestion path must not attribute a harness-less event to
    // anyone (that's how a buggy future adapter would masquerade as opencode).
    run_event(
        &home,
        &json!({"event": "working", "session_id": "nh-1"}).to_string(),
    )
    .success();
    assert!(!state_path(&home).exists());
}

#[test]
fn ended_for_an_unknown_session_writes_nothing() {
    let home = temp_home();
    // No ghost `Done` row for a session we never saw.
    run_event(&home, &ev("ended", "ses-ghost")).success();
    assert!(!state_path(&home).exists());
}

#[test]
fn a_status_event_without_a_directory_preserves_the_project_path() {
    let home = temp_home();
    // First event carries the directory…
    run_event(&home, &ev("working", "ses-dir")).success();
    assert_eq!(
        read_state(&home)["sessions"]["ses-dir"]["project_path"],
        "/tmp/oc-project"
    );
    // …a later status-only event omits it and must not blank the label.
    run_event(
        &home,
        &json!({"harness": "opencode", "event": "idle", "session_id": "ses-dir"}).to_string(),
    )
    .success();
    let state = read_state(&home);
    assert_eq!(session_status(&state, "ses-dir"), "inactive");
    assert_eq!(
        state["sessions"]["ses-dir"]["project_path"],
        "/tmp/oc-project"
    );
}

#[test]
fn a_working_event_with_a_title_writes_even_when_already_active() {
    let home = temp_home();
    let old_timestamp = "2020-01-01T00:00:00Z";
    seed_state(
        &home,
        &json!({
            "sessions": {
                "ses-w": {
                    "status": "active",
                    "last_updated": old_timestamp,
                    "project_path": null,
                    "notification_type": null,
                    "name": "old name",
                    "harness": "opencode",
                }
            }
        }),
    );

    // Already active, but a title rides along → the write is NOT suppressed.
    run_event(&home, &ev_title("working", "ses-w", "fresh title")).success();

    let state = read_state(&home);
    assert_eq!(state["sessions"]["ses-w"]["name"], "fresh title");
    assert_ne!(state["sessions"]["ses-w"]["last_updated"], old_timestamp);
}

#[test]
fn a_foreign_harness_flows_through_and_sweeps_stay_scoped() {
    let home = temp_home();
    // A future adapter (here "codex") uses the exact same verbs, no new Rust.
    run_event(
        &home,
        &json!({"harness": "codex", "event": "working", "session_id": "cx-1"}).to_string(),
    )
    .success();
    let state = read_state(&home);
    assert_eq!(session_status(&state, "cx-1"), "active");
    assert_eq!(state["sessions"]["cx-1"]["harness"], "codex");

    // A codex reconnect sweep must leave a stale opencode session untouched, and
    // vice-versa. Seed one stale session of each (no owner PID → stale).
    seed_state(
        &home,
        &json!({
            "sessions": {
                "cx-stale": {"status": "active", "last_updated": "2020-01-01T00:00:00Z",
                    "project_path": null, "notification_type": null, "name": null, "harness": "codex"},
                "oc-stale": {"status": "active", "last_updated": "2020-01-01T00:00:00Z",
                    "project_path": null, "notification_type": null, "name": null, "harness": "opencode"},
            }
        }),
    );
    run_event(
        &home,
        &json!({"harness": "codex", "event": "reconnected"}).to_string(),
    )
    .success();
    let state = read_state(&home);
    assert_eq!(
        session_status(&state, "cx-stale"),
        "done",
        "codex sweep clears codex"
    );
    assert_eq!(
        session_status(&state, "oc-stale"),
        "active",
        "codex sweep must not touch opencode"
    );
}

#[test]
fn reconnected_spares_a_session_whose_owner_is_alive() {
    let home = temp_home();
    // owner_pid = this test process, which is alive for the duration → the
    // session belongs to a still-running server and must survive the sweep.
    let live_pid = std::process::id();
    seed_state(
        &home,
        &json!({
            "sessions": {
                "ses-live": {
                    "status": "active",
                    "last_updated": "2020-01-01T00:00:00Z",
                    "project_path": null,
                    "notification_type": null,
                    "name": null,
                    "harness": "opencode",
                    "terminal": { "owner_pid": live_pid },
                }
            }
        }),
    );

    run_event(
        &home,
        &json!({"harness": "opencode", "event": "reconnected"}).to_string(),
    )
    .success();

    assert_eq!(session_status(&read_state(&home), "ses-live"), "active");
}
