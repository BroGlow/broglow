//! End-to-end tests of the hook backend: drive the real `broglow` binary the
//! way Claude Code does — one lifecycle event as JSON on stdin — and observe
//! the resulting `~/.claude/broglow/state.json`.
//!
//! Each test gets its own throwaway home directory. `dirs::home_dir()` follows
//! `$HOME` on unix, which is what sandboxes these; Windows resolves the profile
//! via the known-folder API (no env override), hence the cfg gate.
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
        .join("broglow")
        .join("state.json")
}

/// Run `broglow hook` inside the sandbox home with the given stdin.
fn run_hook(home: &TempDir, stdin: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("broglow")
        .expect("binary built")
        .arg("hook")
        .env("HOME", home.path())
        .env_remove("BROGLOW_NAMING")
        .write_stdin(stdin.to_string())
        .assert()
}

/// A hook event as Claude Code sends it. `transcript_path` is deliberately
/// omitted: a `Stop` event carrying one would spawn the detached auto-namer,
/// which shells out to the `claude` CLI — the namer is tested separately
/// against a fake `claude` below.
fn event(name: &str, session_id: &str) -> String {
    json!({
        "hook_event_name": name,
        "session_id": session_id,
        "cwd": "/tmp/some-project",
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

/// Seed a pre-existing state.json, bypassing the binary.
fn seed_state(home: &TempDir, state: &Value) {
    let path = state_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, state.to_string()).unwrap();
}

#[test]
fn session_start_records_an_active_session() {
    let home = temp_home();
    run_hook(&home, &event("SessionStart", "sess-1")).success();

    let state = read_state(&home);
    assert_eq!(session_status(&state, "sess-1"), "active");
    assert_eq!(
        state["sessions"]["sess-1"]["project_path"],
        "/tmp/some-project"
    );
}

#[test]
fn full_lifecycle_maps_events_to_statuses() {
    let home = temp_home();
    let steps = [
        ("SessionStart", "active"),
        ("UserPromptSubmit", "active"),
        ("Stop", "inactive"),
        ("Notification", "needs_help"),
        ("SessionEnd", "done"),
    ];
    for (hook_event, expected) in steps {
        run_hook(&home, &event(hook_event, "sess-life")).success();
        let state = read_state(&home);
        assert_eq!(
            session_status(&state, "sess-life"),
            expected,
            "after {hook_event}"
        );
    }
}

#[test]
fn idle_prompt_notification_is_ignored() {
    let home = temp_home();
    run_hook(&home, &event("SessionStart", "sess-idle")).success();

    let idle = json!({
        "hook_event_name": "Notification",
        "session_id": "sess-idle",
        "notification_type": "idle_prompt",
    });
    run_hook(&home, &idle.to_string()).success();

    // Still active — an idle nudge must not flip the session to needs-help.
    assert_eq!(session_status(&read_state(&home), "sess-idle"), "active");
}

#[test]
fn pre_tool_use_skips_the_write_when_already_active() {
    let home = temp_home();
    let old_timestamp = "2020-01-01T00:00:00Z";
    seed_state(
        &home,
        &json!({
            "sessions": {
                "sess-hot": {
                    "status": "active",
                    "last_updated": old_timestamp,
                    "project_path": null,
                    "notification_type": null,
                    "name": null,
                }
            }
        }),
    );

    run_hook(&home, &event("PreToolUse", "sess-hot")).success();

    // The seeded timestamp surviving proves no write happened.
    let state = read_state(&home);
    assert_eq!(state["sessions"]["sess-hot"]["last_updated"], old_timestamp);
}

#[test]
fn pre_tool_use_reactivates_an_inactive_session() {
    let home = temp_home();
    seed_state(
        &home,
        &json!({
            "sessions": {
                "sess-cold": {
                    "status": "inactive",
                    "last_updated": "2020-01-01T00:00:00Z",
                    "project_path": null,
                    "notification_type": null,
                    "name": null,
                }
            }
        }),
    );

    run_hook(&home, &event("PreToolUse", "sess-cold")).success();

    let state = read_state(&home);
    assert_eq!(session_status(&state, "sess-cold"), "active");
    assert_ne!(
        state["sessions"]["sess-cold"]["last_updated"],
        "2020-01-01T00:00:00Z"
    );
}

/// A `PreToolUse` event for `tool_name` with the given `tool_input`.
fn tool_event(session_id: &str, tool_name: &str, tool_input: Value) -> String {
    json!({
        "hook_event_name": "PreToolUse",
        "session_id": session_id,
        "cwd": "/tmp/some-project",
        "tool_name": tool_name,
        "tool_input": tool_input,
    })
    .to_string()
}

#[test]
fn a_wrap_up_question_stays_needs_help_through_the_stop_that_follows() {
    let home = temp_home();
    run_hook(&home, &event("UserPromptSubmit", "sess-q")).success();

    // The desktop app's next-step box: the model asks, the box ends the turn.
    let wrap_up = json!({
        "recap": "Install fix is one commit.",
        "ask": "Split it into 3 commits?",
        "blocked": true,
    });
    run_hook(
        &home,
        &tool_event("sess-q", "mcp__ccd_turn__wrap_up", wrap_up),
    )
    .success();
    let state = read_state(&home);
    assert_eq!(session_status(&state, "sess-q"), "needs_help");
    assert_eq!(
        state["sessions"]["sess-q"]["notification_type"],
        "question_box"
    );

    run_hook(&home, &event("Stop", "sess-q")).success();
    assert_eq!(session_status(&read_state(&home), "sess-q"), "needs_help");

    // The user's answer (a button click is a prompt) clears it.
    run_hook(&home, &event("UserPromptSubmit", "sess-q")).success();
    let state = read_state(&home);
    assert_eq!(session_status(&state, "sess-q"), "active");
    assert_eq!(
        state["sessions"]["sess-q"]["notification_type"],
        Value::Null
    );
}

#[test]
fn an_unblocked_ask_counts_as_a_question_too() {
    let home = temp_home();
    let wrap_up = json!({ "recap": "Done.", "ask": "Commit it?" });
    run_hook(
        &home,
        &tool_event("sess-ask", "mcp__ccd_turn__wrap_up", wrap_up),
    )
    .success();
    assert_eq!(session_status(&read_state(&home), "sess-ask"), "needs_help");
}

#[test]
fn a_recap_only_wrap_up_is_a_normal_turn_end() {
    let home = temp_home();
    run_hook(&home, &event("UserPromptSubmit", "sess-recap")).success();

    let wrap_up = json!({ "recap": "All done, nothing pending." });
    run_hook(
        &home,
        &tool_event("sess-recap", "mcp__ccd_turn__wrap_up", wrap_up),
    )
    .success();
    assert_eq!(session_status(&read_state(&home), "sess-recap"), "active");

    run_hook(&home, &event("Stop", "sess-recap")).success();
    assert_eq!(session_status(&read_state(&home), "sess-recap"), "inactive");
}

#[test]
fn ask_user_question_is_needs_help_until_the_turn_moves_on() {
    let home = temp_home();
    run_hook(&home, &event("UserPromptSubmit", "sess-auq")).success();

    let input = json!({ "questions": [{ "question": "Which fix first?" }] });
    run_hook(&home, &tool_event("sess-auq", "AskUserQuestion", input)).success();
    let state = read_state(&home);
    assert_eq!(session_status(&state, "sess-auq"), "needs_help");
    assert_eq!(
        state["sessions"]["sess-auq"]["notification_type"],
        "ask_user_question"
    );

    // The tool blocks the turn; once answered the model keeps working …
    run_hook(&home, &tool_event("sess-auq", "Read", json!({}))).success();
    assert_eq!(session_status(&read_state(&home), "sess-auq"), "active");
    // … and a plain Stop ends the turn as usual.
    run_hook(&home, &event("Stop", "sess-auq")).success();
    assert_eq!(session_status(&read_state(&home), "sess-auq"), "inactive");
}

#[test]
fn unknown_events_and_missing_session_ids_write_nothing() {
    let home = temp_home();
    run_hook(&home, &event("SomeFutureEvent", "sess-x")).success();
    run_hook(
        &home,
        &json!({"hook_event_name": "SessionStart"}).to_string(),
    )
    .success();

    assert!(!state_path(&home).exists());
}

#[test]
fn malformed_input_is_a_safe_noop() {
    let home = temp_home();
    run_hook(&home, "this is not json {{{").success();
    run_hook(&home, "").success();

    assert!(!state_path(&home).exists());
}

#[test]
fn an_empty_state_file_is_treated_as_fresh() {
    // A crash during the write's rename, or a hand truncation, leaves a
    // zero-length file; hooks must recover from it instead of freezing.
    let home = temp_home();
    let path = state_path(&home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "").unwrap();

    run_hook(&home, &event("SessionStart", "sess-1")).success();

    assert_eq!(session_status(&read_state(&home), "sess-1"), "active");
}

#[test]
fn a_corrupt_state_file_is_never_clobbered() {
    let home = temp_home();
    // Simulate a mid-write snapshot / unknown future schema on disk.
    let garbage = r#"{"sessions": {"other-session": "#;
    let path = state_path(&home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, garbage).unwrap();

    run_hook(&home, &event("SessionStart", "sess-new")).success();

    // The hook must refuse to write rather than wipe other sessions' status.
    assert_eq!(std::fs::read_to_string(&path).unwrap(), garbage);
}

#[test]
fn an_existing_name_survives_status_updates() {
    let home = temp_home();
    seed_state(
        &home,
        &json!({
            "sessions": {
                "sess-named": {
                    "status": "active",
                    "last_updated": "2020-01-01T00:00:00Z",
                    "project_path": null,
                    "notification_type": null,
                    "name": "Fix Auth Bug",
                }
            }
        }),
    );

    run_hook(&home, &event("Stop", "sess-named")).success();

    let state = read_state(&home);
    assert_eq!(session_status(&state, "sess-named"), "inactive");
    assert_eq!(state["sessions"]["sess-named"]["name"], "Fix Auth Bug");
}

#[test]
fn sessions_are_tracked_independently() {
    let home = temp_home();
    run_hook(&home, &event("SessionStart", "sess-a")).success();
    run_hook(&home, &event("SessionStart", "sess-b")).success();
    run_hook(&home, &event("Stop", "sess-b")).success();

    let state = read_state(&home);
    assert_eq!(session_status(&state, "sess-a"), "active");
    assert_eq!(session_status(&state, "sess-b"), "inactive");
}

#[test]
fn the_naming_guard_env_disables_tracking() {
    let home = temp_home();
    Command::cargo_bin("broglow")
        .expect("binary built")
        .arg("hook")
        .env("HOME", home.path())
        .env("BROGLOW_NAMING", "1")
        .write_stdin(event("SessionStart", "sess-nested"))
        .assert()
        .success();

    assert!(!state_path(&home).exists());
}

// ---------------------------------------------------------------------------
// Auto-namer (`broglow name <id> <transcript>`), run against a fake `claude`
// CLI placed first on PATH so the test never touches the real one.
// ---------------------------------------------------------------------------

/// Install an executable `claude` stub in the sandbox and return the PATH to
/// run with. The stub must come first so a real `claude` is never invoked.
fn fake_claude(home: &TempDir, script_body: &str) -> String {
    use std::os::unix::fs::PermissionsExt;

    let bin_dir = home.path().join("fake-bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let script = bin_dir.join("claude");
    std::fs::write(&script, format!("#!/bin/sh\n{script_body}\n")).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    format!(
        "{}:{}",
        bin_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    )
}

fn write_transcript(home: &TempDir, first_prompt: &str) -> PathBuf {
    let path = home.path().join("transcript.jsonl");
    let lines = [
        json!({"type": "summary", "summary": "not a message"}).to_string(),
        json!({"message": {"role": "assistant", "content": "ignored"}}).to_string(),
        json!({"message": {"role": "user", "content": first_prompt}}).to_string(),
    ];
    std::fs::write(&path, lines.join("\n")).unwrap();
    path
}

fn run_namer(home: &TempDir, path_env: &str, transcript: &PathBuf) {
    Command::cargo_bin("broglow")
        .expect("binary built")
        .arg("name")
        .arg("sess-unnamed")
        .arg(transcript)
        .env("HOME", home.path())
        .env("PATH", path_env)
        .assert()
        .success();
}

fn seed_unnamed_session(home: &TempDir) {
    seed_state(
        home,
        &json!({
            "sessions": {
                "sess-unnamed": {
                    "status": "inactive",
                    "last_updated": "2020-01-01T00:00:00Z",
                    "project_path": null,
                    "notification_type": null,
                    "name": null,
                }
            }
        }),
    );
}

#[test]
fn the_namer_titles_a_session_via_the_claude_cli() {
    let home = temp_home();
    seed_unnamed_session(&home);
    // Answers only when the naming call opts out of session persistence, so a
    // regression that would litter the user's history with naming sessions
    // fails here.
    let path_env = fake_claude(
        &home,
        r#"case "$*" in *--no-session-persistence*) echo "Refactor Login Flow" ;; *) exit 1 ;; esac"#,
    );
    let transcript = write_transcript(&home, "please refactor the login flow");

    run_namer(&home, &path_env, &transcript);

    let state = read_state(&home);
    assert_eq!(
        state["sessions"]["sess-unnamed"]["name"],
        "Refactor Login Flow"
    );
}

#[test]
fn the_namer_falls_back_to_the_first_prompt_when_the_cli_fails() {
    let home = temp_home();
    seed_unnamed_session(&home);
    // What a logged-out CLI does: the error on stdout, a non-zero exit.
    let path_env = fake_claude(&home, r#"echo "Not logged in"; exit 1"#);
    let transcript = write_transcript(&home, "some prompt");

    run_namer(&home, &path_env, &transcript);

    let state = read_state(&home);
    assert_eq!(state["sessions"]["sess-unnamed"]["name"], "some prompt");
}

#[test]
fn the_namer_fallback_skips_injected_system_reminders() {
    let home = temp_home();
    seed_unnamed_session(&home);
    let path_env = fake_claude(&home, "exit 1");
    let transcript = write_transcript(
        &home,
        "<system-reminder>\nscratch workspace notice\n</system-reminder>\n\nwhat can you do?",
    );

    run_namer(&home, &path_env, &transcript);

    let state = read_state(&home);
    assert_eq!(
        state["sessions"]["sess-unnamed"]["name"],
        "what can you do?"
    );
}

#[test]
fn the_namer_never_clobbers_a_name_that_landed_meanwhile() {
    let home = temp_home();
    seed_unnamed_session(&home);
    let mut state = read_state(&home);
    state["sessions"]["sess-unnamed"]["name"] = json!("Title From Claude");
    seed_state(&home, &state);
    let path_env = fake_claude(&home, r#"echo "Generated Title""#);
    let transcript = write_transcript(&home, "some prompt");

    run_namer(&home, &path_env, &transcript);

    let state = read_state(&home);
    assert_eq!(
        state["sessions"]["sess-unnamed"]["name"],
        "Title From Claude"
    );
}

// ---------------------------------------------------------------------------
// Claude's own titles (`custom-title` transcript records), read by the hook.
// ---------------------------------------------------------------------------

/// A transcript as Claude writes it once titled: messages interleaved with
/// `custom-title` records, re-appended each turn; the last one is current.
fn write_titled_transcript(home: &TempDir, titles: &[&str]) -> PathBuf {
    let path = home.path().join("titled.jsonl");
    let mut lines =
        vec![json!({"message": {"role": "user", "content": "tell me the status"}}).to_string()];
    for title in titles {
        lines.push(
            json!({"type": "custom-title", "customTitle": title, "sessionId": "sess-titled"})
                .to_string(),
        );
        lines.push(json!({"message": {"role": "assistant", "content": "ok"}}).to_string());
    }
    std::fs::write(&path, lines.join("\n")).unwrap();
    path
}

/// A hook event carrying a transcript, run with a failing fake `claude` first
/// on PATH so a wrongly spawned namer can never reach the real CLI.
fn run_hook_with_transcript(home: &TempDir, name: &str, transcript: &PathBuf) {
    let path_env = fake_claude(home, "exit 1");
    Command::cargo_bin("broglow")
        .expect("binary built")
        .arg("hook")
        .env("HOME", home.path())
        .env("PATH", path_env)
        .env_remove("BROGLOW_NAMING")
        .write_stdin(
            json!({
                "hook_event_name": name,
                "session_id": "sess-titled",
                "cwd": "/tmp/some-project",
                "transcript_path": transcript,
            })
            .to_string(),
        )
        .assert()
        .success();
}

#[test]
fn the_hook_names_a_session_from_claudes_own_title() {
    let home = temp_home();
    let transcript = write_titled_transcript(&home, &["Project status and next steps"]);

    run_hook_with_transcript(&home, "Stop", &transcript);

    let state = read_state(&home);
    assert_eq!(session_status(&state, "sess-titled"), "inactive");
    assert_eq!(
        state["sessions"]["sess-titled"]["name"],
        "Project status and next steps"
    );
}

#[test]
fn a_claude_rename_replaces_an_earlier_name() {
    let home = temp_home();
    seed_state(
        &home,
        &json!({
            "sessions": {
                "sess-titled": {
                    "status": "inactive",
                    "last_updated": "2020-01-01T00:00:00Z",
                    "project_path": null,
                    "notification_type": null,
                    "name": "Generated Title",
                }
            }
        }),
    );
    let transcript = write_titled_transcript(&home, &["First title", "Renamed by user"]);

    run_hook_with_transcript(&home, "UserPromptSubmit", &transcript);

    let state = read_state(&home);
    assert_eq!(session_status(&state, "sess-titled"), "active");
    assert_eq!(state["sessions"]["sess-titled"]["name"], "Renamed by user");
}

#[test]
fn claudes_title_is_found_at_the_end_of_a_large_transcript() {
    let home = temp_home();
    let path = home.path().join("large.jsonl");
    // Well past the 1 MiB tail window, so the read starts mid-line.
    let filler = json!({"message": {"role": "assistant", "content": "x".repeat(1000)}}).to_string();
    let mut lines = vec![filler; 1500];
    lines.push(json!({"type": "custom-title", "customTitle": "Long Session"}).to_string());
    lines.push(json!({"message": {"role": "assistant", "content": "done"}}).to_string());
    std::fs::write(&path, lines.join("\n")).unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() > 1024 * 1024);

    run_hook_with_transcript(&home, "Stop", &path);

    let state = read_state(&home);
    assert_eq!(state["sessions"]["sess-titled"]["name"], "Long Session");
}

#[test]
fn an_unnamed_session_takes_claudes_title_mid_turn() {
    // The desktop app titles a session a few records into its first turn,
    // which can run for many minutes of tool calls before the next Stop.
    let home = temp_home();
    seed_state(
        &home,
        &json!({
            "sessions": {
                "sess-titled": {
                    "status": "active",
                    "last_updated": "2020-01-01T00:00:00Z",
                    "project_path": null,
                    "notification_type": null,
                    "name": null,
                }
            }
        }),
    );
    let transcript = write_titled_transcript(&home, &["App readiness for launch"]);

    run_hook_with_transcript(&home, "PreToolUse", &transcript);

    let state = read_state(&home);
    assert_eq!(session_status(&state, "sess-titled"), "active");
    assert_eq!(
        state["sessions"]["sess-titled"]["name"],
        "App readiness for launch"
    );
}

#[test]
fn a_named_active_session_keeps_skipping_pre_tool_use_writes() {
    // Renames follow at turn boundaries; mid-turn tool calls stay write-free.
    let home = temp_home();
    let old_timestamp = "2020-01-01T00:00:00Z";
    seed_state(
        &home,
        &json!({
            "sessions": {
                "sess-titled": {
                    "status": "active",
                    "last_updated": old_timestamp,
                    "project_path": null,
                    "notification_type": null,
                    "name": "First title",
                }
            }
        }),
    );
    let transcript = write_titled_transcript(&home, &["First title", "Renamed by user"]);

    run_hook_with_transcript(&home, "PreToolUse", &transcript);

    let state = read_state(&home);
    assert_eq!(
        state["sessions"]["sess-titled"]["last_updated"],
        old_timestamp
    );
    assert_eq!(state["sessions"]["sess-titled"]["name"], "First title");
}
