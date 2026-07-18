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

#[test]
fn install_writes_the_opencode_plugin_when_detected_and_uninstall_removes_it() {
    let home = TempDir::new().unwrap();
    // opencode "present": its global config dir exists (one of the two detection
    // signals — the other is an `opencode` binary on PATH).
    std::fs::create_dir_all(home.path().join(".config/opencode")).unwrap();

    run(&home, "install");

    let plugin = home.path().join(".config/opencode/plugins/clawlight.js");
    assert!(plugin.exists(), "plugin written when opencode is detected");

    let contents = std::fs::read_to_string(&plugin).unwrap();
    assert!(
        contents.contains("managed by clawlight"),
        "carries the managed-by header"
    );
    // The version and absolute binary path are baked in — no placeholders left.
    assert!(!contents.contains("{{BIN}}"), "binary path substituted");
    assert!(!contents.contains("{{VERSION}}"), "version substituted");
    assert!(
        contents.contains("\"event\""),
        "plugin drives the `clawlight event` backend"
    );

    run(&home, "uninstall");
    assert!(!plugin.exists(), "uninstall removes our plugin");
}

#[test]
fn install_skips_the_plugin_when_opencode_is_absent() {
    let home = TempDir::new().unwrap();
    // No ~/.config/opencode and (assumed) no opencode on PATH → detection fails.
    run(&home, "install");
    let plugin = home.path().join(".config/opencode/plugins/clawlight.js");
    assert!(
        !plugin.exists(),
        "no plugin written on a machine without opencode"
    );
}

#[test]
fn reinstall_overwrites_a_stale_managed_plugin() {
    let home = TempDir::new().unwrap();
    let plugin_dir = home.path().join(".config/opencode/plugins");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    let plugin = plugin_dir.join("clawlight.js");
    // A plugin from an older build: our marker (so it's ours to replace), an
    // un-substituted binary placeholder, and a sentinel that must not survive.
    std::fs::write(
        &plugin,
        "// managed by clawlight v0.0.1\nconst CLAWLIGHT_BIN = \"{{BIN}}\";\n// STALE_SENTINEL\n",
    )
    .unwrap();

    run(&home, "install");

    let contents = std::fs::read_to_string(&plugin).unwrap();
    assert!(
        !contents.contains("STALE_SENTINEL"),
        "reinstall must overwrite the stale plugin (version-skew fix)"
    );
    assert!(!contents.contains("{{BIN}}"), "binary path re-substituted");
    assert!(contents.contains("managed by clawlight"), "still managed");
}

#[test]
fn uninstall_leaves_a_foreign_opencode_plugin_alone() {
    let home = TempDir::new().unwrap();
    let plugin_dir = home.path().join(".config/opencode/plugins");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    let plugin = plugin_dir.join("clawlight.js");
    // A file at our path that isn't ours (no managed-by header).
    std::fs::write(&plugin, "// my own hand-rolled plugin\n").unwrap();

    run(&home, "uninstall");

    assert!(
        plugin.exists(),
        "a file without our header must never be deleted"
    );
}
