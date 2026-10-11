//! GitHub Copilot CLI specifics, behind the `copilot` harness adapter (see
//! harness.rs).
//!
//! Copilot CLI supports user-level lifecycle hooks: every `*.json` file in
//! `$COPILOT_HOME/hooks/` (default `~/.copilot/hooks/`) may register commands
//! per event, and each command receives one JSON payload on stdin. Unlike
//! Claude's dialect the payload does **not** carry the event name — hooks are
//! registered per event — so the adapter bakes the event into the registered
//! command line (`broglow copilot-hook <event>`) and the shim in hook.rs
//! maps it onto the normalized harness verbs.
//!
//! Two Copilot facts shape the adapter:
//! - **broglow owns a whole file.** The hooks directory takes any number of
//!   `*.json` files, so broglow writes its own `broglow.json` and never
//!   merges into (or reorders) anyone else's configuration — simpler and
//!   safer than the Codex hooks.json merge. Ownership is proven by the
//!   `copilot-hook` command string inside the file, not a comment marker:
//!   JSON has no comments, and inventing an extra top-level key risks a
//!   strict parser rejecting the whole file.
//! - **Sessions are payload-only for us.** Copilot names its own sessions,
//!   but that name lives in `~/.copilot/session-store.db` (SQLite) and
//!   undocumented session-state files — not worth a database dependency. The
//!   hook payloads themselves carry the user's prompt (`prompt` /
//!   `initialPrompt`), so first-prompt fallback naming needs no file reads at
//!   all. And Copilot fires a real `sessionEnd`, so unlike Codex there is no
//!   exec-vs-interactive sniffing: session end is an event, not an inference.

use std::path::PathBuf;

use anyhow::Context;

/// Copilot CLI's home directory: `$COPILOT_HOME` when set (tests, relocated
/// installs), else `~/.copilot`.
pub fn copilot_home() -> Option<PathBuf> {
    if let Some(h) = std::env::var_os("COPILOT_HOME") {
        if !h.is_empty() {
            return Some(PathBuf::from(h));
        }
    }
    dirs::home_dir().map(|home| home.join(".copilot"))
}

/// The hook file broglow owns: `$COPILOT_HOME/hooks/broglow.json`.
fn hooks_file_path() -> Option<PathBuf> {
    copilot_home().map(|h| h.join("hooks").join("broglow.json"))
}

/// Lifecycle events the adapter registers, in Copilot's camelCase dialect.
/// `notification` is deliberately absent: Copilot's waiting-on-approval
/// signal is `permissionRequest`, and its other notification types are an
/// undocumented vocabulary — mapping them blind could flip the icon red on
/// informational nudges.
pub const HOOK_EVENTS: [&str; 7] = [
    "sessionStart",
    "userPromptSubmitted",
    "preToolUse",
    "postToolUse",
    "permissionRequest",
    "agentStop",
    "sessionEnd",
];

/// Whether a hooks-file body is broglow's to overwrite or delete: every
/// registration we write invokes `copilot-hook`, so its absence means the
/// file at our path is someone's hand-rolled configuration.
fn file_is_ours(contents: &str) -> bool {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(contents) else {
        return false;
    };
    let Some(hooks) = value.get("hooks").and_then(|v| v.as_object()) else {
        return false;
    };
    !hooks.is_empty()
        && hooks.iter().all(|(event, entries)| {
            HOOK_EVENTS.contains(&event.as_str())
                && entries.as_array().is_some_and(|entries| {
                    !entries.is_empty()
                        && entries.iter().all(|entry| {
                            entry
                                .get("command")
                                .and_then(|v| v.as_str())
                                .is_some_and(|cmd| {
                                    crate::codex::broglow_command_args(cmd)
                                        == Some(format!("copilot-hook {event}").as_str())
                                })
                        })
                })
        })
}

/// The full contents of `broglow.json`: one command group per event, with
/// the event name baked into the argv (Copilot payloads don't identify their
/// event). The binary path is quoted for spaces (`C:\Program Files\...`).
fn hooks_file_value(exe: &std::path::Path) -> serde_json::Value {
    let mut hooks = serde_json::Map::new();
    for event in HOOK_EVENTS {
        hooks.insert(
            event.to_string(),
            serde_json::json!([{
                "type": "command",
                "command": format!("\"{}\" copilot-hook {event}", exe.display()),
            }]),
        );
    }
    serde_json::json!({ "version": 1, "hooks": hooks })
}

/// Write (or refresh) `broglow.json` with this binary's absolute path baked
/// in. Idempotent: the unconditional overwrite of a file we own is also the
/// version-skew fix — every `broglow install` re-syncs it to the current
/// binary. A file at the path that isn't ours is never touched, matching
/// `uninstall`.
pub fn install_hooks() -> anyhow::Result<()> {
    let path = hooks_file_path().context("No home directory")?;

    match std::fs::read_to_string(&path) {
        Ok(existing) if !file_is_ours(&existing) => {
            eprintln!(
                "broglow: {} exists but isn't broglow-managed; leaving it alone",
                path.display()
            );
            return Ok(());
        }
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(e).with_context(|| format!("Reading {}", path.display()));
        }
        _ => {}
    }

    let exe = std::env::current_exe().context("Resolving current executable path")?;
    // Temp file + rename, like the Codex and Claude settings writers: a crash
    // mid-write must never leave Copilot a half-written hooks file.
    crate::codex::write_json_atomic(&path, &hooks_file_value(&exe))
        .context("Writing Copilot hooks file")?;
    println!("Wrote Copilot hooks to {}", path.display());
    println!("  (restart any running copilot sessions to pick them up)");
    Ok(())
}

/// Remove `broglow.json` iff it is still ours, so a hand-rolled file at the
/// path is left alone.
pub fn uninstall_hooks() {
    let Some(path) = hooks_file_path() else {
        return;
    };
    let Ok(contents) = std::fs::read_to_string(&path) else {
        return;
    };
    if !file_is_ours(&contents) {
        return;
    }
    if std::fs::remove_file(&path).is_ok() {
        println!("Removed {}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_file_registers_every_event_with_its_name_in_argv() {
        let v = hooks_file_value(std::path::Path::new("/opt/bro glow/broglow"));
        assert_eq!(v["version"], 1);
        for event in HOOK_EVENTS {
            let cmd = v["hooks"][event][0]["command"]
                .as_str()
                .unwrap_or_else(|| panic!("{event} registered"));
            // Quoted path (spaces survive) + the event baked into argv, since
            // Copilot payloads don't say which event fired.
            assert_eq!(
                cmd,
                format!("\"/opt/bro glow/broglow\" copilot-hook {event}")
            );
            assert_eq!(v["hooks"][event][0]["type"], "command");
        }
        // The serialized file must satisfy our own ownership predicate, or
        // install could never refresh what it just wrote.
        assert!(file_is_ours(&v.to_string()));
    }

    #[test]
    fn foreign_hook_files_are_recognized() {
        for command in [
            "other copilot-hook agentStop",
            "my-broglow copilot-hook agentStop",
            "echo /bin/broglow copilot-hook agentStop",
        ] {
            assert!(!file_is_ours(
                &serde_json::json!({"hooks":{"agentStop":[{"command":command}]}}).to_string()
            ));
        }
        assert!(!file_is_ours("not JSON: copilot-hook"));
        assert!(!file_is_ours(&serde_json::json!({"hooks":{"agentStop":[{"command":"broglow copilot-hook agentStop"},{"command":"other"}]}}).to_string()));
        assert!(!file_is_ours(
            r#"{"version":1,"hooks":{"preToolUse":[{"type":"command","command":"./guard.sh"}]}}"#
        ));
        assert!(file_is_ours(
            r#"{"hooks":{"agentStop":[{"command":"\"/usr/bin/broglow\" copilot-hook agentStop"}]}}"#
        ));
    }
}
