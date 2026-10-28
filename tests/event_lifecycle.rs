//! End-to-end tests of the normalized-event backend: drive the real
//! `broglow` binary the way the opencode plugin does — one normalized event
//! as JSON on stdin — and observe the resulting `state.json`. Mirrors the
//! strategy of `hook_lifecycle.rs`.
//!
//! Unix-only for the same reason as `hook_lifecycle.rs`: `dirs::home_dir()`
//! follows `$HOME` on unix, which is what sandboxes each test's home.
#![cfg(unix)]

mod common;

use common::{command, read_state, run_hook, seed_state, session_status, state_path, temp_home};
use serde_json::json;
use tempfile::TempDir;

/// A `last_updated` the done-session retention will not prune.
fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Run `broglow event` inside the sandbox home with the given stdin.
fn run_event(home: &TempDir, stdin: &str) -> assert_cmd::assert::Assert {
    command(home)
        .arg("event")
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

fn usage_event(session: &str, message: &str) -> serde_json::Value {
    json!({
        "harness": "opencode", "event": "usage", "session_id": session,
        "message_id": message, "role": "assistant",
        "completed": chrono::Local::now().timestamp_millis(),
        "tokens": {"input": 100, "output": 30, "reasoning": 20,
            "cache": {"read": 50, "write": 10}},
    })
}

fn usage_path(home: &TempDir) -> std::path::PathBuf {
    state_path(home).with_file_name(format!(
        "opencode-usage-{}.json",
        chrono::Local::now().date_naive()
    ))
}

fn enable_usage(home: &TempDir) {
    let path = state_path(home).with_file_name("config.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, r#"{"usage_enabled":true}"#).unwrap();
}

#[test]
fn usage_is_opt_in_and_never_creates_session_state() {
    let home = temp_home();
    let event = usage_event("ses-api", "msg-api").to_string();
    run_event(&home, &event).success();
    assert!(!state_path(&home).parent().unwrap().exists());

    enable_usage(&home);
    run_event(&home, &event).success();
    assert!(usage_path(&home).exists());
    assert!(!state_path(&home).exists());
}

#[test]
fn usage_reconciles_messages_across_sessions_processes_and_out_of_order_updates() {
    let home = temp_home();
    enable_usage(&home);
    run_event(&home, &ev("needs_input", "ses-api")).success();
    let state_before = std::fs::read(state_path(&home)).unwrap();
    let mut event = usage_event("ses-api", "msg-shared");
    run_event(&home, &event.to_string()).success();
    run_event(&home, &event.to_string()).success();
    event["tokens"]["total"] = json!(200);
    run_event(&home, &event.to_string()).success();
    event["tokens"]["total"] = json!(150);
    run_event(&home, &event.to_string()).success();
    run_event(&home, &usage_event("ses-other", "msg-shared").to_string()).success();

    let bytes = std::fs::read(usage_path(&home)).unwrap();
    let ledger: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        ledger,
        json!({"ses-api": {"msg-shared": 200},
        "ses-other": {"msg-shared": 190}})
    );
    assert_eq!(std::fs::read(state_path(&home)).unwrap(), state_before);
}

#[test]
fn usage_ignores_incomplete_or_invalid_messages() {
    let home = temp_home();
    enable_usage(&home);
    let valid = usage_event("ses-api", "msg-api");
    for (field, value) in [
        ("role", json!("user")),
        ("completed", json!(null)),
        ("completed", json!(-1)),
        ("completed", json!(1.5)),
        ("message_id", json!("")),
        ("session_id", json!(null)),
        ("tokens", json!({"input": 5, "output": -1})),
    ] {
        let mut invalid = valid.clone();
        invalid[field] = value;
        run_event(&home, &invalid.to_string()).success();
        assert!(!usage_path(&home).exists(), "invalid {field}");
    }
    assert!(!state_path(&home).exists());
}

#[test]
fn usage_never_overwrites_unreadable_ledger_or_writes_without_the_shared_lock() {
    let home = temp_home();
    enable_usage(&home);
    let path = usage_path(&home);
    for damaged in ["{incomplete", r#"{"ses":{"msg":"bad count"}}"#] {
        std::fs::write(&path, damaged).unwrap();
        run_event(&home, &usage_event("ses-api", "msg-api").to_string()).failure();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), damaged);
    }
    std::fs::remove_file(&path).unwrap();
    let lock = path.with_file_name(".state.lock");
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(lock).unwrap();
    run_event(&home, &usage_event("ses-api", "msg-api").to_string()).success();
    assert!(!path.exists());
}

#[test]
fn usage_uses_completion_local_day_rather_than_arrival_or_utc_day() {
    let home = temp_home();
    enable_usage(&home);
    let mut event = usage_event("ses-api", "msg-api");
    event["completed"] = json!(1_704_070_800_000i64); // 2024-01-01 01:00 UTC.
    command(&home)
        .env("TZ", "America/Los_Angeles")
        .arg("event")
        .write_stdin(event.to_string())
        .assert()
        .success();
    let path = state_path(&home).with_file_name("opencode-usage-2023-12-31.json");
    assert!(path.exists());
    assert!(!state_path(&home)
        .with_file_name("opencode-usage-2024-01-01.json")
        .exists());
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
        let state = read_state(&home);
        assert_eq!(state["sessions"]["ses-life"]["harness"], "opencode");
        assert_eq!(
            state["sessions"]["ses-life"]["project_path"],
            "/tmp/oc-project"
        );
    }
}

#[test]
fn failed_state_lock_never_writes_unlocked() {
    let home = temp_home();
    run_event(&home, &ev("working", "existing")).success();
    let path = state_path(&home);
    let original = std::fs::read(&path).unwrap();
    let lock = path.with_file_name(".state.lock");
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();

    run_event(&home, &ev("idle", "existing")).success();
    run_hook(
        &home,
        &json!({"hook_event_name": "SessionStart", "session_id": "new"}).to_string(),
    )
    .success();
    assert_eq!(std::fs::read(&path).unwrap(), original);
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
                    "last_updated": now(),
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
                "cx-stale": {"status": "active", "last_updated": now(),
                    "project_path": null, "notification_type": null, "name": null, "harness": "codex"},
                "oc-stale": {"status": "active", "last_updated": now(),
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

#[test]
fn a_write_prunes_done_sessions_by_the_configured_retention() {
    let home = temp_home();
    let seed = || {
        seed_state(
            &home,
            &json!({
                "sessions": {
                    "ancient-done": {"status": "done", "last_updated": "2020-01-01T00:00:00Z",
                        "project_path": null, "notification_type": null, "name": null},
                    "ancient-live": {"status": "active", "last_updated": "2020-01-01T00:00:00Z",
                        "project_path": null, "notification_type": null, "name": null},
                }
            }),
        );
    };
    let event =
        json!({"harness": "opencode", "event": "working", "session_id": "oc-1"}).to_string();

    // Default retention (14 days) drops the done session; live ones stay.
    seed();
    run_event(&home, &event).success();
    let state = read_state(&home);
    assert!(state["sessions"].get("ancient-done").is_none());
    assert_eq!(session_status(&state, "ancient-live"), "active");

    // Zero disables both limits.
    let config = home.path().join(".broglow").join("config.json");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(
        &config,
        json!({"done_sessions": {"keep_days": 0, "keep_max": 0}}).to_string(),
    )
    .unwrap();
    seed();
    run_event(&home, &event).success();
    assert_eq!(session_status(&read_state(&home), "ancient-done"), "done");
}
