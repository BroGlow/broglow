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
fn prompt_names_sessions_before_stop_without_a_transcript() {
    let home = temp_home();
    let prompt = format!("  \n Explain\t{}\nplease  ", "🦀日本語 ".repeat(12));
    let expected =
        "Explain 🦀日本語 🦀日本語 🦀日本語 🦀日本語 🦀日本語 🦀日本語 🦀日本語 🦀日本語...";

    // Both ephemeral sessions and sessions whose rollout is not available yet.
    for (id, transcript) in [
        ("ephemeral", Value::Null),
        ("normal", json!("/missing/rollout.jsonl")),
    ] {
        let submit = |prompt: Value| {
            run_codex_hook(
                &home,
                &json!({
                    "hook_event_name": "UserPromptSubmit",
                    "session_id": id,
                    "transcript_path": transcript,
                    "prompt": prompt,
                })
                .to_string(),
            )
            .success();
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
