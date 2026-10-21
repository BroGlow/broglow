//! End-to-end tests of the Codex shim: drive the real `broglow codex-hook`
//! the way Codex's hooks do — one Claude-dialect hook payload as JSON on
//! stdin — and observe the resulting `state.json`. Mirrors the strategy of
//! `event_lifecycle.rs` / `hook_lifecycle.rs`.
//!
//! Unix-only for the same reason as those suites: `dirs::home_dir()` follows
//! `$HOME` on unix, which is what sandboxes each test's home. `$CODEX_HOME`
//! sandboxes the Codex side (session_index.jsonl, rollouts).
#![cfg(unix)]

mod common;

use common::{codex_home, command, read_state, session, state_path, temp_home};
use serde_json::{json, Value};
use tempfile::TempDir;

/// Run `broglow codex-hook` inside the sandbox with the given stdin.
fn run_codex_hook(home: &TempDir, stdin: &str) -> assert_cmd::assert::Assert {
    command(home)
        .arg("codex-hook")
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

fn assert_not_live(home: &TempDir, id: &str, event: &str) {
    let row = session(&read_state(home), id);
    assert!(
        row.is_null() || row["status"] == "done",
        "background run must not contribute live status after {event}: {row}"
    );
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
fn automatic_approval_review_does_not_ask_the_user_for_help() {
    let home = temp_home();
    let id = "auto-review";
    let rollout = home.path().join("rollout.jsonl");
    let request = |turn_id: &str| {
        run_codex_hook(
            &home,
            &json!({
                "hook_event_name": "PermissionRequest", "session_id": id,
                "transcript_path": rollout, "turn_id": turn_id,
                "tool_name": "Bash", "tool_input": {"command": "true"},
            })
            .to_string(),
        )
        .success();
    };
    run_codex_hook(&home, &ev("SessionStart", id)).success();
    std::fs::write(&rollout, concat!(
        "{\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"old\",\"approvals_reviewer\":\"user\"}}\n",
        "{\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"current\",\"approvals_reviewer\":\"auto_review\"}}\n",
    )).unwrap();
    request("current");
    assert_eq!(session(&read_state(&home), id)["status"], "active");

    // A stale or absent reviewer must never hide a possible human approval.
    request("next");
    assert_eq!(session(&read_state(&home), id)["status"], "needs_help");
    request("current");
    assert_eq!(session(&read_state(&home), id)["status"], "needs_help");
    run_codex_hook(&home, &ev("PostToolUse", id)).success();
    std::fs::write(&rollout, "{\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"current\",\"approvals_reviewer\":\"user\"}}\n").unwrap();
    request("current");
    assert_eq!(session(&read_state(&home), id)["status"], "needs_help");
}

#[test]
fn codex_questions_interrupts_and_session_end_follow_the_lifecycle() {
    let home = temp_home();
    let id = "question";
    run_codex_hook(&home, &ev("SessionStart", id)).success();
    run_codex_hook(
        &home,
        &json!({
            "hook_event_name": "PreToolUse", "session_id": id,
            "tool_name": "Bash", "tool_use_id": "shell",
        })
        .to_string(),
    )
    .success();
    for event in ["PreToolUse", "PostToolUse"] {
        run_codex_hook(
            &home,
            &json!({
                "hook_event_name": event, "session_id": id,
                "tool_name": "request_user_input", "tool_use_id": "question",
                "tool_input": {"questions": [{"id": "choice", "question": "Which option?"}]},
            })
            .to_string(),
        )
        .success();
        assert_eq!(
            session(&read_state(&home), id)["status"],
            if event == "PreToolUse" {
                "needs_help"
            } else {
                "active"
            }
        );
        if event == "PreToolUse" {
            for (tool, call) in [("Bash", "shell"), ("request_user_input", "older-question")] {
                run_codex_hook(
                    &home,
                    &json!({
                        "hook_event_name": "PostToolUse", "session_id": id,
                        "tool_name": tool, "tool_use_id": call,
                    })
                    .to_string(),
                )
                .success();
                assert_eq!(session(&read_state(&home), id)["status"], "needs_help");
            }
        }
    }
    run_codex_hook(&home, &ev("PermissionRequest", id)).success();
    run_codex_hook(&home, &ev("Interrupt", id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "inactive");
    run_codex_hook(&home, &ev("SessionEnd", id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "done");
}

#[test]
fn codex_async_question_without_terminal_evidence_stays_red_until_the_user_replies() {
    let home = temp_home();
    let id = "async-question";
    run_codex_hook(&home, &ev("SessionStart", id)).success();
    for event in ["PreToolUse", "PostToolUse"] {
        run_codex_hook(
            &home,
            &json!({
                "hook_event_name": event, "session_id": id,
                "tool_name": "request_user_input_async", "tool_use_id": "quiz",
                "tool_input": {"questions": [{"title": "iPhone or Samsung?", "options": ["iPhone", "Samsung"]}]},
                "tool_response": {"accepted": true},
            })
            .to_string(),
        )
        .success();
        assert_eq!(
            session(&read_state(&home), id)["status"],
            "needs_help",
            "{event}"
        );
    }
    // Displaying the quiz completes the tool immediately; neither concurrent
    // work nor an unconfirmed Stop means the question has expired.
    for event in ["PreToolUse", "PostToolUse", "Stop"] {
        run_codex_hook(&home, &ev(event, id)).success();
        assert_eq!(
            session(&read_state(&home), id)["status"],
            "needs_help",
            "{event}"
        );
    }
    run_codex_hook(&home, &ev("UserPromptSubmit", id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "active");
    run_codex_hook(&home, &ev("Stop", id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "inactive");
    for (event, status) in [("Interrupt", "inactive"), ("SessionEnd", "done")] {
        run_codex_hook(
            &home,
            &json!({
                "hook_event_name": "PreToolUse", "session_id": id,
                "tool_name": "request_user_input_async", "tool_use_id": "next-quiz",
            })
            .to_string(),
        )
        .success();
        run_codex_hook(&home, &ev(event, id)).success();
        let s = session(&read_state(&home), id);
        assert_eq!(s["status"], status, "{event}");
        assert!(s["notification_type"].is_null());
    }
}

#[test]
fn async_question_creation_failure_and_confirmed_terminal_are_distinct() {
    let home = temp_home();
    let rollout = home.path().join("question.jsonl");
    std::fs::write(&rollout, "").unwrap();
    let mut event = json!({
        "hook_event_name": "PreToolUse", "session_id": "question",
        "turn_id": "turn", "tool_name": "request_user_input_async",
        "tool_use_id": "call", "transcript_path": rollout,
    });
    run_codex_hook(&home, &event.to_string()).success();
    event["hook_event_name"] = json!("PostToolUse");
    event["tool_response"] = json!({"accepted": false});
    run_codex_hook(&home, &event.to_string()).success();
    assert_eq!(session(&read_state(&home), "question")["status"], "active");
    assert!(read_state(&home)["question_expiry"].is_null());

    event["hook_event_name"] = json!("PreToolUse");
    event["tool_use_id"] = json!("retry-call");
    run_codex_hook(&home, &event.to_string()).success();
    event["hook_event_name"] = json!("PostToolUse");
    event["tool_response"] = json!({"accepted": true});
    run_codex_hook(&home, &event.to_string()).success();
    event["hook_event_name"] = json!("Stop");
    run_codex_hook(&home, &event.to_string()).success();
    assert_eq!(
        session(&read_state(&home), "question")["status"],
        "needs_help"
    );
    std::fs::write(&rollout, "{\"timestamp\":\"2026-10-21T10:00:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"turn\"}}\n").unwrap();
    run_codex_hook(&home, &event.to_string()).success();
    let state = read_state(&home);
    assert_eq!(session(&state, "question")["status"], "inactive");
    assert_eq!(
        state["question_expiry"]["occurred_at"],
        "2026-10-21T10:00:00Z"
    );
    run_codex_hook(&home, &event.to_string()).success();
    assert_eq!(
        read_state(&home)["question_expiry"],
        state["question_expiry"]
    );
}

#[test]
fn answered_question_is_quiet_and_old_turn_hooks_cannot_clear_a_new_question() {
    let home = temp_home();
    let rollout = home.path().join("question.jsonl");
    std::fs::write(&rollout, "").unwrap();
    let mut event = json!({
        "hook_event_name": "PreToolUse", "session_id": "question", "turn_id": "old",
        "tool_name": "request_user_input_async", "tool_use_id": "call", "transcript_path": rollout,
    });
    run_codex_hook(&home, &event.to_string()).success();
    event["hook_event_name"] = json!("PostToolUse");
    event["tool_response"] = json!({"accepted": true});
    run_codex_hook(&home, &event.to_string()).success();
    run_codex_hook(&home, &ev("UserPromptSubmit", "question")).success();
    assert_eq!(session(&read_state(&home), "question")["status"], "active");
    assert_eq!(
        read_state(&home)["codex_questions"]["question"]["resolved"],
        true
    );
    assert!(read_state(&home)["question_expiry"].is_null());
    event["hook_event_name"] = json!("PreToolUse");
    event["turn_id"] = json!("new");
    event["tool_use_id"] = json!("new-call");
    std::fs::write(
        &rollout,
        "{\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"new\"}}\n",
    )
    .unwrap();
    run_codex_hook(&home, &event.to_string()).success();
    std::fs::write(&rollout, "{\"timestamp\":\"2026-10-21T10:00:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"old\"}}\n").unwrap();
    for hook in ["PostToolUse", "Stop", "Interrupt", "SessionEnd"] {
        event["hook_event_name"] = json!(hook);
        event["turn_id"] = json!("old");
        run_codex_hook(&home, &event.to_string()).success();
        assert_eq!(
            session(&read_state(&home), "question")["status"],
            "needs_help",
            "{hook}"
        );
        assert!(read_state(&home)["question_expiry"].is_null());
    }
}

#[test]
fn structured_answers_resolve_all_question_indices_without_a_prompt_hook() {
    let home = temp_home();
    let rollout = home.path().join("question.jsonl");
    std::fs::write(&rollout, "").unwrap();
    let event = json!({
        "hook_event_name": "PreToolUse", "session_id": "question", "turn_id": "turn",
        "tool_name": "request_user_input_async", "tool_use_id": "call", "transcript_path": rollout,
        "tool_input": {"questions": [{"title":"One?"}, {"title":"Two?"}]},
    });
    run_codex_hook(&home, &event.to_string()).success();
    let mut contents = json!({"type":"event_msg", "payload":{
        "type":"item_completed", "turn_id":"turn", "item": {
            "id":"call", "type":"AgentMessage", "questions":[{"title":"One?"},{"title":"Two?"}],
            "phase":"final_answer"
        }
    }})
    .to_string()
        + "\n";
    let stop = json!({"hook_event_name":"Stop", "session_id":"question", "turn_id":"turn"});
    for (call, index, expected) in [
        ("older-call", 0, "needs_help"),
        ("call", 3, "needs_help"),
        ("call", 0, "needs_help"),
        ("call", 0, "needs_help"),
        ("call", 1, "active"),
    ] {
        let answer = json!([{"questionItemId": json!(["request_user_input_async", call, index]).to_string(),
            "question":"Question", "answer":"Selected option"}]);
        contents += &json!({"type":"response_item", "payload":{
            "type":"message", "role":"user", "content":[{"type":"input_text", "text":format!(
                "<send_user_message_question_reply>{answer}</send_user_message_question_reply>"
            )}]
        }})
        .to_string();
        contents.push('\n');
        std::fs::write(&rollout, &contents).unwrap();
        run_codex_hook(&home, &stop.to_string()).success();
        assert_eq!(session(&read_state(&home), "question")["status"], expected);
        assert!(read_state(&home)["question_expiry"].is_null());
    }
}

#[test]
fn terminal_tombstone_blocks_replayed_hooks_but_allows_a_proven_new_turn() {
    let home = temp_home();
    let rollout = home.path().join("question.jsonl");
    let mut contents = String::new();
    std::fs::write(&rollout, &contents).unwrap();
    let mut event = json!({"hook_event_name":"PreToolUse", "session_id":"q", "turn_id":"old",
        "tool_name":"request_user_input_async", "tool_use_id":"old-call", "transcript_path":rollout,
        "tool_input":{"questions":[{"title":"Choice?"}]}, "tool_response":{"accepted":true}});
    for hook in ["PreToolUse", "PostToolUse"] {
        event["hook_event_name"] = json!(hook);
        run_codex_hook(&home, &event.to_string()).success();
    }
    contents += "{\"type\":\"event_msg\",\"timestamp\":\"2026-10-21T10:00:00Z\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"old\"}}\n";
    std::fs::write(&rollout, &contents).unwrap();
    event["hook_event_name"] = json!("Stop");
    run_codex_hook(&home, &event.to_string()).success();
    let expiry = read_state(&home)["question_expiry"].clone();
    for hook in ["PreToolUse", "PostToolUse", "Stop"] {
        event["hook_event_name"] = json!(hook);
        run_codex_hook(&home, &event.to_string()).success();
        assert_eq!(session(&read_state(&home), "q")["status"], "inactive");
        assert_eq!(read_state(&home)["question_expiry"], expiry);
    }
    event["hook_event_name"] = json!("SessionEnd");
    run_codex_hook(&home, &event.to_string()).success();
    assert_eq!(session(&read_state(&home), "q")["status"], "done");
    let mut ordinary = json!({"session_id":"q", "turn_id":"ordinary", "tool_name":"Bash"});
    for hook in ["UserPromptSubmit", "PreToolUse", "PostToolUse", "Stop"] {
        ordinary["hook_event_name"] = json!(hook);
        run_codex_hook(&home, &ordinary.to_string()).success();
    }
    assert_eq!(session(&read_state(&home), "q")["status"], "inactive");
    contents += "{\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"new\"}}\n";
    std::fs::write(&rollout, &contents).unwrap();
    event["turn_id"] = json!("new");
    event["tool_use_id"] = json!("new-call");
    event["hook_event_name"] = json!("PreToolUse");
    run_codex_hook(&home, &event.to_string()).success();
    assert_eq!(
        session(&read_state(&home), "q")["notification_type"],
        "codex_question_async:new-call"
    );
    event["turn_id"] = json!("old");
    event["tool_use_id"] = json!("old-call");
    run_codex_hook(&home, &event.to_string()).success();
    assert_eq!(
        session(&read_state(&home), "q")["notification_type"],
        "codex_question_async:new-call"
    );
    assert_eq!(
        read_state(&home)["codex_questions"]
            .as_object()
            .unwrap()
            .len(),
        1
    );
    ordinary["turn_id"] = json!("prompt-proven");
    ordinary["hook_event_name"] = json!("UserPromptSubmit");
    run_codex_hook(&home, &ordinary.to_string()).success();
    event["turn_id"] = json!("prompt-proven");
    event["tool_use_id"] = json!("prompt-call");
    run_codex_hook(&home, &event.to_string()).success();
    event["turn_id"] = json!("new");
    event["tool_use_id"] = json!("new-call");
    run_codex_hook(&home, &event.to_string()).success();
    assert_eq!(
        session(&read_state(&home), "q")["notification_type"],
        "codex_question_async:prompt-call"
    );
    ordinary["turn_id"] = json!("long-turn");
    ordinary["hook_event_name"] = json!("UserPromptSubmit");
    run_codex_hook(&home, &ordinary.to_string()).success();
    contents += &"x".repeat(300 * 1024);
    contents.push('\n');
    std::fs::write(&rollout, &contents).unwrap();
    event["turn_id"] = json!("long-turn");
    event["tool_use_id"] = json!("long-call");
    run_codex_hook(&home, &event.to_string()).success();
    assert_eq!(
        session(&read_state(&home), "q")["notification_type"],
        "codex_question_async:long-call"
    );
}

#[test]
fn a_late_new_call_from_a_superseded_turn_cannot_restore_red() {
    let home = temp_home();
    let rollout = home.path().join("question.jsonl");
    std::fs::write(&rollout, "").unwrap();
    let mut event = json!({"session_id":"q", "turn_id":"old", "transcript_path":rollout,
        "tool_name":"request_user_input_async", "tool_use_id":"first",
        "tool_input":{"questions":[{"title":"Choice?"}]}, "tool_response":{"accepted":true}});
    for hook in ["PreToolUse", "PostToolUse"] {
        event["hook_event_name"] = json!(hook);
        run_codex_hook(&home, &event.to_string()).success();
    }
    let mut next = json!({"session_id":"q", "turn_id":"new", "hook_event_name":"UserPromptSubmit"});
    run_codex_hook(&home, &next.to_string()).success();
    event["tool_use_id"] = json!("delayed-call");
    for hook in ["PreToolUse", "PostToolUse"] {
        event["hook_event_name"] = json!(hook);
        run_codex_hook(&home, &event.to_string()).success();
        assert_eq!(session(&read_state(&home), "q")["status"], "active");
    }
    std::fs::write(&rollout, "{\"type\":\"event_msg\",\"timestamp\":\"2026-10-21T10:00:00Z\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"old\"}}\n").unwrap();
    next["hook_event_name"] = json!("Stop");
    run_codex_hook(&home, &next.to_string()).success();
    let state = read_state(&home);
    assert_eq!(session(&state, "q")["status"], "inactive");
    assert!(state["question_expiry"].is_null());
}

#[test]
fn answering_the_latest_call_keeps_an_older_call_pending() {
    let home = temp_home();
    let rollout = home.path().join("question.jsonl");
    std::fs::write(&rollout, "").unwrap();
    let mut event = json!({"session_id":"q", "turn_id":"turn", "transcript_path":rollout,
        "tool_name":"request_user_input_async", "tool_input":{"questions":[{"title":"Choice?"}]},
        "tool_response":{"accepted":true}});
    for call in ["first", "second"] {
        event["tool_use_id"] = json!(call);
        for hook in ["PreToolUse", "PostToolUse"] {
            event["hook_event_name"] = json!(hook);
            run_codex_hook(&home, &event.to_string()).success();
        }
    }
    let answer = json!([{"questionItemId":json!(["request_user_input_async", "second", 0]).to_string(), "answer":"yes"}]);
    let mut contents = json!({"type":"response_item", "payload":{"type":"message", "role":"user", "content":[{
        "type":"input_text", "text":format!("<send_user_message_question_reply>{answer}</send_user_message_question_reply>")
    }]}}).to_string() + "\n";
    std::fs::write(&rollout, &contents).unwrap();
    event["hook_event_name"] = json!("Stop");
    run_codex_hook(&home, &event.to_string()).success();
    assert_eq!(session(&read_state(&home), "q")["status"], "needs_help");
    contents += "{\"type\":\"event_msg\",\"timestamp\":\"2026-10-21T10:00:00Z\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"turn\"}}\n";
    std::fs::write(&rollout, &contents).unwrap();
    run_codex_hook(&home, &event.to_string()).success();
    assert_eq!(session(&read_state(&home), "q")["status"], "inactive");
    assert!(!read_state(&home)["question_expiry"].is_null());
}

#[cfg(target_os = "macos")]
#[test]
fn desktop_rollout_repairs_stale_terminal_identity_before_focus() {
    let home = temp_home();
    let id = "00000000-0000-4000-8000-000000000001";
    let rollout = home.path().join("desktop.jsonl");
    std::fs::write(&rollout, "{\"type\":\"session_meta\",\"payload\":{\"source\":\"vscode\",\"originator\":\"Codex Desktop\"}}\n").unwrap();
    run_codex_hook(&home, &ev("SessionStart", id)).success();
    let mut state = read_state(&home);
    state["sessions"][id]["terminal"] = json!({
        "bundle_id": "com.microsoft.VSCode", "term_program": "Apple_Terminal",
        "tty": "/dev/ttys007", "owner_pid": 99999999,
    });
    std::fs::write(state_path(&home), state.to_string()).unwrap();
    // The VS Code extension shares source=vscode, but must keep its terminal.
    std::fs::write(&rollout, "{\"type\":\"session_meta\",\"payload\":{\"source\":\"vscode\",\"originator\":\"codex_vscode\"}}\n").unwrap();
    run_codex_hook(
        &home,
        &ev_with_transcript("PreToolUse", id, rollout.to_str().unwrap()),
    )
    .success();
    assert_eq!(
        session(&read_state(&home), id)["terminal"],
        state["sessions"][id]["terminal"]
    );
    std::fs::write(&rollout, "{\"type\":\"session_meta\",\"payload\":{\"source\":\"vscode\",\"originator\":\"Codex Desktop\"}}\n").unwrap();
    // Already-active event suppression must still let the host repair through.
    run_codex_hook(
        &home,
        &ev_with_transcript("PreToolUse", id, rollout.to_str().unwrap()),
    )
    .success();
    assert_eq!(
        session(&read_state(&home), id)["terminal"],
        json!({"bundle_id": "com.openai.codex"})
    );
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
fn codex_transient_background_runs_do_not_become_paused_chats() {
    let home = temp_home();
    run_codex_hook(&home, &ev("SessionStart", "real-chat")).success();
    let original = session(&read_state(&home), "real-chat");
    for (id, transcript) in [("null-path", Value::Null), ("empty-path", json!(""))] {
        for event in [
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PostToolUse",
            "Stop",
        ] {
            run_codex_hook(
                &home,
                &json!({
                    "hook_event_name": event, "session_id": id,
                    "transcript_path": transcript,
                })
                .to_string(),
            )
            .success();
            assert_not_live(&home, id, event);
        }
        // Each hook runs in a separate process; later payloads may omit the
        // evidence identifying the unsaved run.
        for event in ["PreToolUse", "PostToolUse", "Stop"] {
            run_codex_hook(&home, &ev(event, id)).success();
            assert_not_live(&home, id, event);
        }
        assert_eq!(session(&read_state(&home), "real-chat"), original);
    }
    // A paused row left by an older BroGlow stops contributing live status.
    command(&home)
        .arg("event")
        .write_stdin(
            json!({"harness": "codex", "event": "idle", "session_id": "legacy-background"})
                .to_string(),
        )
        .assert()
        .success();
    run_codex_hook(
        &home,
        &json!({"hook_event_name": "Stop", "session_id": "legacy-background", "transcript_path": null})
            .to_string(),
    )
    .success();
    assert_not_live(&home, "legacy-background", "Stop");
    assert_eq!(session(&read_state(&home), "real-chat"), original);
    // Subagent lifecycle events refer to the parent id and must not remove it.
    let before = read_state(&home);
    run_codex_hook(&home, &json!({"hook_event_name": "SubagentStop", "session_id": "real-chat", "transcript_path": null}).to_string()).success();
    assert_eq!(read_state(&home), before);
}

#[test]
fn codex_saved_ambient_runs_stay_finished_after_metadata_arrives() {
    for source in ["ambient_suggestions", "ambient_suggestion_safety"] {
        let home = temp_home();
        let id = "saved-ambient";
        let rollout = home.path().join("saved-ambient.jsonl");
        // SessionStart may arrive before the saved rollout is readable.
        run_codex_hook(
            &home,
            &ev_with_transcript("SessionStart", id, rollout.to_str().unwrap()),
        )
        .success();
        assert_eq!(session(&read_state(&home), id)["status"], "active");
        std::fs::write(
            &rollout,
            json!({"type": "session_meta", "payload": {
                "id": id, "source": "vscode", "thread_source": source,
            }})
            .to_string(),
        )
        .unwrap();
        run_codex_hook(
            &home,
            &ev_with_transcript("PreToolUse", id, rollout.to_str().unwrap()),
        )
        .success();
        assert_not_live(&home, id, "PreToolUse with metadata");
        // Later hook processes do not need to repeat the transcript path.
        for event in [
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PostToolUse",
            "PermissionRequest",
            "Stop",
            "Interrupt",
            "SessionEnd",
        ] {
            run_codex_hook(&home, &ev(event, id)).success();
            assert_not_live(&home, id, event);
        }
    }
}

#[test]
fn codex_human_prompts_mentioning_app_requests_keep_normal_lifecycle() {
    let home = temp_home();
    for (id, prompt) in [
        ("normal", "Fix the session light"),
        (
            "exact",
            "This is an app-generated request for a suggested next task.",
        ),
        (
            "quoted",
            "\"This is an app-generated request for a suggested next task.\"",
        ),
        (
            "embedded",
            "Why does Codex say This is an app-generated request for a suggested next task.?",
        ),
    ] {
        run_codex_hook(
            &home,
            &json!({
                "hook_event_name": "UserPromptSubmit",
                "session_id": id,
                "prompt": prompt,
            })
            .to_string(),
        )
        .success();
        assert_eq!(session(&read_state(&home), id)["status"], "active", "{id}");
        run_codex_hook(&home, &ev("Stop", id)).success();
        assert_eq!(
            session(&read_state(&home), id)["status"],
            "inactive",
            "{id}"
        );
    }
}

#[test]
fn codex_accepted_and_scheduled_tasks_keep_app_generated_prompt_and_title() {
    let prompt = "This is an app-generated request for a suggested next task.";
    for source in [
        "ambient_suggestion_task",
        "automation",
        "agent_created_thread",
        "agent_forked_thread",
    ] {
        let home = temp_home();
        let id = "visible-task";
        let rollout = home.path().join("visible-task.jsonl");
        std::fs::write(
            &rollout,
            format!(
                "{}\n{}\n",
                json!({"type": "session_meta", "payload": {
                    "id": id, "source": "vscode", "thread_source": source,
                }}),
                json!({"type": "event_msg", "payload": {"type": "user_message", "message": prompt}})
            ),
        )
        .unwrap();
        std::fs::create_dir_all(codex_home(&home)).unwrap();
        std::fs::write(
            codex_home(&home).join("session_index.jsonl"),
            json!({"id": id, "thread_name": prompt}).to_string(),
        )
        .unwrap();
        run_codex_hook(
            &home,
            &json!({
                "hook_event_name": "UserPromptSubmit",
                "session_id": id,
                "transcript_path": rollout,
                "prompt": prompt,
            })
            .to_string(),
        )
        .success();
        assert_eq!(
            session(&read_state(&home), id)["status"],
            "active",
            "{source}"
        );
        run_codex_hook(
            &home,
            &ev_with_transcript("Stop", id, rollout.to_str().unwrap()),
        )
        .success();
        let row = session(&read_state(&home), id);
        assert_eq!(row["status"], "inactive", "{source}");
        assert_eq!(row["name"], prompt, "{source}");
    }
}

#[test]
fn codex_ambient_run_does_not_replace_another_harness_with_the_same_id() {
    let home = temp_home();
    let id = "shared-id";
    command(&home)
        .arg("event")
        .write_stdin(json!({"harness": "opencode", "event": "idle", "session_id": id}).to_string())
        .assert()
        .success();
    let original = read_state(&home);
    let rollout = home.path().join("ambient.jsonl");
    std::fs::write(
        &rollout,
        json!({"type": "session_meta", "payload": {
            "id": id, "source": "vscode", "thread_source": "ambient_suggestions",
        }})
        .to_string(),
    )
    .unwrap();
    run_codex_hook(
        &home,
        &ev_with_transcript("UserPromptSubmit", id, rollout.to_str().unwrap()),
    )
    .success();
    assert_eq!(read_state(&home), original);
}

#[test]
fn codex_ambient_child_hooks_cannot_hide_a_parent_question() {
    let home = temp_home();
    let id = "parent-chat";
    run_codex_hook(
        &home,
        &json!({
            "hook_event_name": "PreToolUse", "session_id": id,
            "tool_name": "request_user_input_async", "tool_use_id": "question",
        })
        .to_string(),
    )
    .success();
    let pending = read_state(&home);
    assert_eq!(session(&pending, id)["status"], "needs_help");
    let rollout = home.path().join("app-child.jsonl");
    for source in ["ambient_suggestions", "ambient_suggestion_safety"] {
        std::fs::write(
            &rollout,
            json!({"type": "session_meta", "payload": {
                "id": "child", "source": "vscode", "thread_source": source,
            }})
            .to_string(),
        )
        .unwrap();
        for event in ["UserPromptSubmit", "PreToolUse", "PostToolUse", "Stop"] {
            run_codex_hook(
                &home,
                &ev_with_transcript(event, id, rollout.to_str().unwrap()),
            )
            .success();
            assert_eq!(read_state(&home), pending, "child {event}");
        }
    }
}

#[test]
fn codex_internal_rollouts_do_not_become_paused_chats() {
    let home = temp_home();
    run_codex_hook(&home, &ev("SessionStart", "real-chat")).success();
    let original = session(&read_state(&home), "real-chat");
    let rollout = home.path().join("internal.jsonl");
    for (index, metadata) in [
        json!({"source": {"subagent": {"other": "guardian"}}, "thread_source": "guardian_review"}),
        json!({"source": {"subagent": {"thread_spawn": {"parent_thread_id": "real-chat", "depth": 1}}}}),
        json!({"source": "vscode", "thread_source": "subagent"}),
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("internal-{index}");
        std::fs::write(
            &rollout,
            json!({"type": "session_meta", "payload": metadata}).to_string(),
        )
        .unwrap();
        for event in [
            "SessionStart",
            "UserPromptSubmit",
            "PreToolUse",
            "PostToolUse",
            "Stop",
        ] {
            run_codex_hook(
                &home,
                &ev_with_transcript(event, &id, rollout.to_str().unwrap()),
            )
            .success();
            assert_not_live(&home, &id, event);
            assert_eq!(
                session(&read_state(&home), "real-chat"),
                original,
                "{event}: {metadata}"
            );
        }
    }
}

#[test]
fn codex_subagent_hooks_cannot_clear_the_parent_question() {
    let home = temp_home();
    let id = "parent-chat";
    let rollout = home.path().join("child.jsonl");
    run_codex_hook(
        &home,
        &json!({
            "hook_event_name": "PreToolUse", "session_id": id,
            "tool_name": "request_user_input_async", "tool_use_id": "quiz",
        })
        .to_string(),
    )
    .success();
    let pending = read_state(&home);
    assert_eq!(session(&pending, id)["status"], "needs_help");
    // Codex routes child hooks with the parent's session_id. The rollout's
    // id is the child's identity; even its legacy session_id names the parent.
    for (thread_id, legacy_id) in [
        (json!("child-agent"), id),
        (Value::Null, "child-agent"),
        (json!(""), "child-agent"),
    ] {
        std::fs::write(
            &rollout,
            json!({
                "type": "session_meta",
                "payload": {
                    "id": thread_id, "session_id": legacy_id,
                    "source": {"subagent": {"thread_spawn": {"parent_thread_id": id, "depth": 1}}},
                    "thread_source": "subagent",
                },
            })
            .to_string(),
        )
        .unwrap();
        for event in [
            "PreToolUse",
            "PostToolUse",
            "PermissionRequest",
            "SessionStart",
            "UserPromptSubmit",
            "Stop",
            "Interrupt",
            "SessionEnd",
        ] {
            run_codex_hook(
                &home,
                &ev_with_transcript(event, id, rollout.to_str().unwrap()),
            )
            .success();
            assert_eq!(read_state(&home), pending, "child {event}");
        }
    }
    run_codex_hook(&home, &ev("UserPromptSubmit", id)).success();
    assert_eq!(session(&read_state(&home), id)["status"], "active");
}

#[test]
fn codex_saved_chats_keep_working_when_metadata_is_incomplete_or_unfamiliar() {
    let home = temp_home();
    let rollout = home.path().join("saved.jsonl");
    for metadata in [
        json!({"source": "cli"}),
        json!({"source": "vscode", "thread_source": "user"}),
        json!({"source": "vscode", "thread_source": "agent_created_thread"}),
        json!({"source": "vscode", "thread_source": "future_source"}),
        Value::Null,
    ] {
        std::fs::write(
            &rollout,
            json!({"type": "session_meta", "payload": metadata}).to_string(),
        )
        .unwrap();
        run_codex_hook(
            &home,
            &ev_with_transcript("SessionStart", "saved", rollout.to_str().unwrap()),
        )
        .success();
        assert_eq!(session(&read_state(&home), "saved")["status"], "active");
        run_codex_hook(
            &home,
            &ev_with_transcript("Stop", "saved", rollout.to_str().unwrap()),
        )
        .success();
        assert_eq!(session(&read_state(&home), "saved")["status"], "inactive");
    }
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
fn prompt_names_sessions_before_stop_when_the_rollout_is_not_ready() {
    let home = temp_home();
    let prompt = format!("  \n Explain\t{}\nplease  ", "🦀日本語 ".repeat(12));
    let expected =
        "Explain 🦀日本語 🦀日本語 🦀日本語 🦀日本語 🦀日本語 🦀日本語 🦀日本語 🦀日本語...";

    // A not-yet-readable path does not make a saved session a background job.
    for (id, transcript) in [
        ("legacy", None),
        ("normal", Some(json!("/missing/rollout.jsonl"))),
    ] {
        let submit = |prompt: Value| {
            let mut payload = json!({
                "hook_event_name": "UserPromptSubmit",
                "session_id": id,
                "prompt": prompt,
            });
            if let Some(transcript) = &transcript {
                payload["transcript_path"] = transcript.clone();
            }
            run_codex_hook(&home, &payload.to_string()).success();
        };
        run_codex_hook(&home, &ev("SessionStart", id)).success();
        for blank in [Value::Null, json!(42), json!(" \n\t ")] {
            submit(blank);
            assert!(session(&read_state(&home), id)["name"].is_null());
        }
        submit(json!(prompt));
        let named = session(&read_state(&home), id);
        assert_eq!(named["name"], expected);
        assert_eq!(named["name"].as_str().unwrap().chars().count(), 50);
        assert_eq!(named["status"], "active");
        submit(json!("A later prompt must not rename this session"));
        run_codex_hook(&home, &ev("Stop", id)).success();
        assert_eq!(session(&read_state(&home), id)["name"], expected);
        assert_eq!(session(&read_state(&home), id)["status"], "inactive");

        // A saved Codex title wins even when the next observed event is a prompt.
        std::fs::create_dir_all(codex_home(&home)).unwrap();
        std::fs::write(
            codex_home(&home).join("session_index.jsonl"),
            json!({"id": id, "thread_name": "Saved Codex title"}).to_string(),
        )
        .unwrap();
        submit(json!("Keep the saved title"));
        assert_eq!(session(&read_state(&home), id)["name"], "Saved Codex title");
    }
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
