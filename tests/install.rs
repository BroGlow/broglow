//! End-to-end install/uninstall round-trip.
//!
//! Linux-only: `$HOME` sandboxes every path the command touches (settings.json,
//! the XDG autostart entry, the state dir), and the side effects stop there.
//! On macOS `install` would `launchctl bootstrap` a real LaunchAgent — on a
//! developer machine that unloads the actually-installed daemon mid-test — and
//! on Windows it writes the real HKCU Run key, so neither can run here.
#![cfg(target_os = "linux")]

use assert_cmd::Command;
use serde_json::{json, Value};
use tempfile::TempDir;

const HOOK_EVENTS: [&str; 6] = [
    "SessionStart",
    "UserPromptSubmit",
    "Stop",
    "Notification",
    "SessionEnd",
    "PreToolUse",
];

fn run(home: &TempDir, subcommand: &str) {
    Command::cargo_bin("clawlight")
        .expect("binary built")
        .arg(subcommand)
        .env("HOME", home.path())
        .assert()
        .success();
}

fn read_settings(home: &TempDir) -> Value {
    let path = home.path().join(".claude").join("settings.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn install_registers_hooks_and_uninstall_reverts_them() {
    let home = TempDir::new().unwrap();

    // Pre-existing user settings must survive both operations untouched.
    let claude_dir = home.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        json!({"model": "opus"}).to_string(),
    )
    .unwrap();

    run(&home, "install");

    let settings = read_settings(&home);
    assert_eq!(settings["model"], "opus");
    for hook_event in HOOK_EVENTS {
        let command = settings["hooks"][hook_event][0]["hooks"][0]["command"]
            .as_str()
            .unwrap_or_else(|| panic!("{hook_event} hook registered"));
        assert!(
            command.ends_with("\" hook"),
            "hook command invokes the binary: {command}"
        );
    }
    let autostart = home.path().join(".config/autostart/clawlight.desktop");
    assert!(autostart.exists(), "XDG autostart entry written");
    assert!(claude_dir.join("clawlight").exists(), "state dir created");

    run(&home, "uninstall");

    let settings = read_settings(&home);
    assert_eq!(settings["model"], "opus", "unrelated settings preserved");
    assert!(
        settings.get("hooks").is_none(),
        "empty hooks object removed entirely"
    );
    assert!(!autostart.exists(), "autostart entry removed");
    assert!(!claude_dir.join("clawlight").exists(), "state dir removed");
}
