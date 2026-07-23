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
fn install_leaves_a_foreign_opencode_plugin_alone() {
    let home = TempDir::new().unwrap();
    let plugin_dir = home.path().join(".config/opencode/plugins");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    let plugin = plugin_dir.join("clawlight.js");
    // A hand-rolled file at our path (no managed-by header) is the user's own
    // opencode↔clawlight integration; install must warn and skip, not clobber
    // it — the same guard uninstall applies before deleting.
    let hand_rolled = "// my own hand-rolled plugin\n";
    std::fs::write(&plugin, hand_rolled).unwrap();

    run(&home, "install");

    assert_eq!(
        std::fs::read_to_string(&plugin).unwrap(),
        hand_rolled,
        "install must never overwrite a file without our header"
    );
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

#[test]
fn install_registers_codex_hooks_and_uninstall_reverts_them() {
    let home = TempDir::new().unwrap();
    let codex_home = home.path().join("codex-home");
    std::fs::create_dir_all(&codex_home).unwrap();

    // A foreign hook (the user's own) that must survive both operations at
    // its original position — Codex keys hook trust by position in the file.
    let foreign = json!({
        "hooks": [{ "type": "command",
                    "command": "~/scripts/permission-hook.sh",
                    "timeout": 600 }]
    });
    std::fs::write(
        codex_home.join("hooks.json"),
        json!({ "hooks": { "PermissionRequest": [foreign.clone()] } }).to_string(),
    )
    .unwrap();

    let run = |subcommand: &str| {
        Command::cargo_bin("clawlight")
            .expect("binary built")
            .arg(subcommand)
            .env("HOME", home.path())
            .env("CODEX_HOME", &codex_home)
            .assert()
            .success();
    };
    let read_hooks = || -> Value {
        serde_json::from_str(&std::fs::read_to_string(codex_home.join("hooks.json")).unwrap())
            .unwrap()
    };

    run("install");

    let hooks = read_hooks();
    for event in [
        "SessionStart",
        "UserPromptSubmit",
        "Stop",
        "PreToolUse",
        "PostToolUse",
        "PermissionRequest",
    ] {
        let groups = hooks["hooks"][event].as_array().unwrap_or_else(|| {
            panic!("{event} registered in codex hooks.json");
        });
        let ours = groups
            .iter()
            .filter_map(|g| g["hooks"][0]["command"].as_str())
            .find(|c| c.ends_with("\" codex-hook"));
        assert!(ours.is_some(), "{event} has a clawlight codex-hook group");
    }
    // The user's hook kept its position at index 0.
    assert_eq!(
        hooks["hooks"]["PermissionRequest"][0], foreign,
        "foreign PermissionRequest group preserved in place"
    );

    // Idempotent: a second install must not duplicate our groups.
    run("install");
    assert_eq!(
        read_hooks()["hooks"]["Stop"].as_array().unwrap().len(),
        1,
        "no duplicate group after reinstall"
    );

    run("uninstall");
    let hooks = read_hooks();
    assert!(
        hooks["hooks"].get("Stop").is_none(),
        "clawlight-only events removed"
    );
    assert_eq!(
        hooks["hooks"]["PermissionRequest"],
        json!([foreign]),
        "foreign hook survives uninstall"
    );
}

#[test]
fn install_writes_copilot_hooks_and_uninstall_reverts_them() {
    let home = TempDir::new().unwrap();
    // The `.copilot` dir is the detection signal (no `copilot` binary on CI).
    let hooks_file = home.path().join(".copilot/hooks/clawlight.json");
    std::fs::create_dir_all(home.path().join(".copilot")).unwrap();

    run(&home, "install");

    let hooks: Value =
        serde_json::from_str(&std::fs::read_to_string(&hooks_file).unwrap()).unwrap();
    assert_eq!(hooks["version"], 1);
    for event in [
        "sessionStart",
        "userPromptSubmitted",
        "preToolUse",
        "postToolUse",
        "permissionRequest",
        "agentStop",
        "sessionEnd",
    ] {
        let command = hooks["hooks"][event][0]["command"]
            .as_str()
            .unwrap_or_else(|| panic!("{event} registered in copilot hooks"));
        assert!(
            command.ends_with(&format!("\" copilot-hook {event}")),
            "{event} invokes the shim with its name on argv: {command}"
        );
        assert_eq!(hooks["hooks"][event][0]["type"], "command");
    }

    // Idempotent: a second install just refreshes the same file.
    run(&home, "install");
    let again: Value =
        serde_json::from_str(&std::fs::read_to_string(&hooks_file).unwrap()).unwrap();
    assert_eq!(again["hooks"]["agentStop"].as_array().unwrap().len(), 1);

    run(&home, "uninstall");
    assert!(!hooks_file.exists(), "our hooks file is removed");
    assert!(
        home.path().join(".copilot").exists(),
        "copilot's own directory is left alone"
    );
}

#[test]
fn copilot_hook_files_that_are_not_ours_are_left_alone() {
    let home = TempDir::new().unwrap();
    let hooks_dir = home.path().join(".copilot/hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    let ours_path = hooks_dir.join("clawlight.json");
    // A hand-rolled file at our path (nothing in it invokes `copilot-hook`)
    // is the user's own configuration; both operations must skip it.
    let hand_rolled =
        r#"{"version":1,"hooks":{"preToolUse":[{"type":"command","command":"./guard.sh"}]}}"#;
    std::fs::write(&ours_path, hand_rolled).unwrap();
    // A sibling hooks file is never clawlight's business at all.
    let sibling = hooks_dir.join("policy.json");
    std::fs::write(&sibling, r#"{"version":1,"hooks":{}}"#).unwrap();

    run(&home, "install");
    assert_eq!(
        std::fs::read_to_string(&ours_path).unwrap(),
        hand_rolled,
        "install must never overwrite a file that isn't ours"
    );

    run(&home, "uninstall");
    assert!(
        ours_path.exists(),
        "a file that isn't ours is never deleted"
    );
    assert!(sibling.exists(), "sibling hook files are untouched");
}
