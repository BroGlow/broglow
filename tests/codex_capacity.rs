//! Capacity errors are terminal rollout records, rather than lifecycle hooks.
#![cfg(unix)]

mod common;

use common::{command, read_state, session, temp_home};
use serde_json::{json, Value};
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

fn hook(home: &TempDir, event: &str, id: &str, turn: &str, rollout: &Path) {
    command(home)
        .arg("codex-hook")
        .write_stdin(
            json!({"hook_event_name":event, "session_id":id, "turn_id":turn,
                "transcript_path":rollout})
            .to_string(),
        )
        .assert()
        .success();
}

fn append(path: &Path, value: Value) {
    writeln!(
        std::fs::OpenOptions::new().append(true).open(path).unwrap(),
        "{value}"
    )
    .unwrap();
}

fn terminal(turn: &str, error: Value) -> Value {
    json!({"type":"event_msg", "timestamp":chrono::Utc::now().to_rfc3339(),
        "payload":{"type":"task_complete", "turn_id":turn, "root_turn_id":turn, "error":error}})
}

fn capacity(turn: &str) -> Value {
    terminal(
        turn,
        json!({"codex_error_info":"server_overloaded",
        "message":"Selected model is at capacity. Please try a different model."}),
    )
}

#[test]
fn capacity_before_or_after_stop_stays_red_until_a_new_attempt() {
    for late in [false, true] {
        let home = temp_home();
        let rollout = home.path().join("rollout.jsonl");
        std::fs::write(&rollout, "").unwrap();
        hook(&home, "UserPromptSubmit", "chat", "first", &rollout);
        if late {
            hook(&home, "Stop", "chat", "first", &rollout);
            assert_eq!(session(&read_state(&home), "chat")["status"], "inactive");
        }
        append(&rollout, capacity("first"));
        hook(&home, "Stop", "chat", "first", &rollout);
        let failed = read_state(&home);
        assert_eq!(session(&failed, "chat")["status"], "needs_help");
        assert_eq!(
            session(&failed, "chat")["notification_type"],
            "codex_capacity"
        );
        assert_eq!(failed["capacity_alert"]["id"], "chat:first");
        assert_eq!(failed["capacity_alert"]["session_id"], "chat");
        assert!(failed["question_expiry"].is_null());
        for event in [
            "Stop",
            "PostToolUse",
            "PreToolUse",
            "Interrupt",
            "PermissionRequest",
            "SessionStart",
        ] {
            hook(&home, event, "chat", "first", &rollout);
            assert_eq!(
                session(&read_state(&home), "chat")["status"],
                "needs_help",
                "{event}"
            );
            assert_eq!(
                read_state(&home)["capacity_alert"],
                failed["capacity_alert"]
            );
        }
        // A different chat's lifecycle cannot dismiss the failed one.
        let other = home.path().join("other.jsonl");
        std::fs::write(&other, "").unwrap();
        hook(&home, "UserPromptSubmit", "other", "other", &other);
        hook(&home, "Stop", "other", "other", &other);
        assert_eq!(session(&read_state(&home), "chat")["status"], "needs_help");

        // The prompt hook can precede task_started and omit its transcript.
        command(&home)
            .arg("codex-hook")
            .write_stdin(
                json!({"hook_event_name":"UserPromptSubmit",
            "session_id":"chat", "turn_id":"retry"})
                .to_string(),
            )
            .assert()
            .success();
        assert_eq!(session(&read_state(&home), "chat")["status"], "active");
        hook(&home, "Stop", "chat", "first", &rollout);
        assert_eq!(session(&read_state(&home), "chat")["status"], "active");
        append(
            &rollout,
            json!({"type":"event_msg", "payload":{"type":"task_started", "turn_id":"retry"}}),
        );
        append(&rollout, capacity("retry"));
        hook(&home, "Stop", "chat", "retry", &rollout);
        assert_eq!(read_state(&home)["capacity_alert"]["id"], "chat:retry");
        hook(&home, "SessionEnd", "chat", "retry", &rollout);
        assert_eq!(session(&read_state(&home), "chat")["status"], "done");
        assert!(read_state(&home)["codex_turns"]["chat"].is_null());
    }
}

#[test]
fn startup_does_not_replay_saved_errors_but_live_stop_can_recover_tracking() {
    let home = temp_home();
    let rollout = home.path().join("rollout.jsonl");
    std::fs::write(&rollout, "").unwrap();
    append(
        &rollout,
        json!({"type":"event_msg", "payload":{"type":"task_started", "turn_id":"old"}}),
    );
    append(&rollout, capacity("old"));
    hook(&home, "SessionStart", "saved", "old", &rollout);
    hook(&home, "Stop", "saved", "old", &rollout);
    assert!(read_state(&home)["capacity_alert"].is_null());
    assert_eq!(session(&read_state(&home), "saved")["status"], "inactive");

    // An older installation may have no transcript tracking until this Stop.
    command(&home)
        .arg("codex-hook")
        .write_stdin(
            json!({"hook_event_name":"SessionStart",
        "session_id":"live"})
            .to_string(),
        )
        .assert()
        .success();
    hook(&home, "Stop", "live", "old", &rollout);
    assert_eq!(read_state(&home)["capacity_alert"]["id"], "live:old");

    command(&home)
        .arg("codex-hook")
        .write_stdin(
            json!({"hook_event_name":"SessionStart",
        "session_id":"different-turn"})
            .to_string(),
        )
        .assert()
        .success();
    hook(&home, "Stop", "different-turn", "new", &rollout);
    assert_eq!(
        session(&read_state(&home), "different-turn")["status"],
        "inactive"
    );
    assert_eq!(read_state(&home)["capacity_alert"]["id"], "live:old");
    append(&rollout, capacity("new"));
    hook(&home, "Stop", "different-turn", "new", &rollout);
    assert_eq!(
        read_state(&home)["capacity_alert"]["id"],
        "different-turn:new"
    );
}

#[test]
fn only_exact_terminal_capacity_errors_raise_attention() {
    for error in [
        Value::Null,
        json!({"codex_error_info":"other_error"}),
        json!({"message":"Selected model is at capacity. Please try a different model."}),
    ] {
        let home = temp_home();
        let rollout = home.path().join("rollout.jsonl");
        std::fs::write(&rollout, "").unwrap();
        hook(&home, "UserPromptSubmit", "chat", "current", &rollout);
        append(&rollout, capacity("unrelated"));
        append(
            &rollout,
            json!({"type":"event_msg", "payload":{"type":"error",
            "turn_id":"current", "error":{"codex_error_info":"server_overloaded"}}}),
        );
        append(&rollout, terminal("current", error));
        hook(&home, "Stop", "chat", "current", &rollout);
        assert_eq!(session(&read_state(&home), "chat")["status"], "inactive");
        assert!(read_state(&home)["capacity_alert"].is_null());
    }
}

#[test]
fn prompt_without_a_turn_id_waits_for_new_turn_evidence_instead_of_replaying_old_capacity() {
    for already_tracked in [false, true] {
        let home = temp_home();
        let rollout = home.path().join("rollout.jsonl");
        std::fs::write(&rollout, "").unwrap();
        append(
            &rollout,
            json!({"type":"event_msg", "payload":{"type":"task_started", "turn_id":"old"}}),
        );
        if already_tracked {
            hook(&home, "UserPromptSubmit", "chat", "old", &rollout);
        }
        append(&rollout, capacity("old"));
        command(&home).arg("codex-hook").write_stdin(json!({
            "hook_event_name":"UserPromptSubmit", "session_id":"chat", "transcript_path":rollout
        }).to_string()).assert().success();
        assert_eq!(session(&read_state(&home), "chat")["status"], "active");
        let alert = read_state(&home)["capacity_alert"].clone();
        // Even a terminal from the old generation flushed after the prompt is stale.
        append(&rollout, capacity("old"));
        hook(&home, "Stop", "chat", "old", &rollout);
        assert_eq!(session(&read_state(&home), "chat")["status"], "active");
        assert_eq!(read_state(&home)["capacity_alert"], alert);
        append(
            &rollout,
            json!({"type":"event_msg", "payload":{"type":"task_started", "turn_id":"new"}}),
        );
        append(&rollout, capacity("new"));
        hook(&home, "Stop", "chat", "new", &rollout);
        assert_eq!(
            session(&read_state(&home), "chat")["notification_type"],
            "codex_capacity"
        );
        assert_eq!(read_state(&home)["capacity_alert"]["id"], "chat:new");
    }
}

#[test]
fn child_and_background_capacity_errors_do_not_change_parent_attention() {
    let home = temp_home();
    let parent = home.path().join("parent.jsonl");
    std::fs::write(&parent, "").unwrap();
    hook(&home, "UserPromptSubmit", "parent", "turn", &parent);
    for (hook_id, thread, source) in [
        ("parent", "child", json!("cli")),
        (
            "background",
            "background",
            json!({"subagent":{"parent_thread_id":"parent"}}),
        ),
    ] {
        let child = home.path().join(format!("{thread}-child.jsonl"));
        std::fs::write(&child, "").unwrap();
        append(
            &child,
            json!({"type":"session_meta", "payload":{"id":thread,"source":source}}),
        );
        append(
            &child,
            json!({"type":"event_msg", "payload":{"type":"task_started", "turn_id":"child-turn"}}),
        );
        append(&child, capacity("child-turn"));
        hook(&home, "Stop", hook_id, "child-turn", &child);
        assert_eq!(session(&read_state(&home), "parent")["status"], "active");
        assert!(read_state(&home)["capacity_alert"].is_null());
    }
}

#[test]
fn capacity_wins_over_async_question_terminal_reconciliation_and_retry_clears_it() {
    let home = temp_home();
    let rollout = home.path().join("rollout.jsonl");
    std::fs::write(&rollout, "").unwrap();
    command(&home).arg("codex-hook").write_stdin(json!({"hook_event_name":"PreToolUse", "session_id":"chat",
        "turn_id":"turn", "transcript_path":rollout, "tool_name":"request_user_input_async", "tool_use_id":"call"}).to_string())
        .assert().success();
    append(&rollout, capacity("turn"));
    hook(&home, "Stop", "chat", "turn", &rollout);
    assert_eq!(
        session(&read_state(&home), "chat")["notification_type"],
        "codex_capacity"
    );
    assert!(read_state(&home)["question_expiry"].is_null());
    append(
        &rollout,
        json!({"type":"event_msg", "payload":{"type":"task_started", "turn_id":"retry"}}),
    );
    // A late Stop for the old turn drives reconciliation but cannot idle the retry.
    hook(&home, "Stop", "chat", "turn", &rollout);
    assert_eq!(session(&read_state(&home), "chat")["status"], "active");
    assert!(read_state(&home)["question_expiry"].is_null());
}
