//! End-to-end install/uninstall round-trip.
//!
//! Linux-only: `$HOME` sandboxes every path the command touches (settings.json,
//! the XDG autostart entry, the state dir), and the side effects stop there.
//! On macOS `install` would `launchctl bootstrap` a real LaunchAgent — on a
//! developer machine that unloads the actually-installed daemon mid-test — and
//! on Windows it writes the real HKCU Run key, so neither can run here.
#![cfg(target_os = "linux")]

mod common;

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

/// Empty PATH makes detection independent of locally installed coding agents.
fn installer(home: &TempDir) -> Command {
    let mut command = common::command(home);
    command.env("PATH", home.path().join("empty-bin"));
    command
}

fn run(home: &TempDir, subcommand: &str) {
    installer(home).arg(subcommand).assert().success();
}

fn read_settings(home: &TempDir) -> Value {
    let path = home.path().join(".claude").join("settings.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn headless_tray_does_not_register_hooks() {
    let home = TempDir::new().unwrap();
    installer(&home).arg("menubar").assert().failure();
    assert!(
        !home.path().join(".claude/settings.json").exists(),
        "a failed tray launch must not recreate hooks during uninstall"
    );
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
    let autostart = home.path().join(".config/autostart/broglow.desktop");
    assert!(autostart.exists(), "XDG autostart entry written");
    assert!(home.path().join(".broglow").exists(), "state dir created");

    run(&home, "uninstall");

    let settings = read_settings(&home);
    assert_eq!(settings["model"], "opus", "unrelated settings preserved");
    assert!(
        settings.get("hooks").is_none(),
        "empty hooks object removed entirely"
    );
    assert!(!autostart.exists(), "autostart entry removed");
    assert!(!home.path().join(".broglow").exists(), "state dir removed");
}

#[test]
fn install_and_uninstall_write_through_a_settings_symlink() {
    // Dotfiles setups link ~/.claude/settings.json to a file they manage; the
    // link must survive and its target must change, never the link itself.
    let home = TempDir::new().unwrap();
    let claude_dir = home.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    let real = home.path().join("dotfiles-settings.json");
    std::fs::write(&real, json!({"model": "opus"}).to_string()).unwrap();
    let link = claude_dir.join("settings.json");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    run(&home, "install");
    assert!(std::fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    let settings: Value = serde_json::from_str(&std::fs::read_to_string(&real).unwrap()).unwrap();
    assert_eq!(settings["model"], "opus");
    assert!(settings["hooks"]["SessionStart"].is_array());

    run(&home, "uninstall");
    assert!(std::fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    let settings: Value = serde_json::from_str(&std::fs::read_to_string(&real).unwrap()).unwrap();
    assert_eq!(settings, json!({"model": "opus"}));
}

#[test]
fn claude_foreign_hooks_survive_install_reinstall_and_uninstall() {
    let home = TempDir::new().unwrap();
    let dir = home.path().join(".claude");
    std::fs::create_dir_all(&dir).unwrap();
    let foreign = json!({"matcher":"*", "hooks":[{"type":"command","command":"guard.sh"}]});
    let mixed = json!({"hooks":[
        {"type":"command","command":"broglow hook"},
        {"type":"command","command":"other.sh"}
    ]});
    std::fs::write(
        dir.join("settings.json"),
        json!({
            "model":"opus", "hooks":{"SessionStart":[foreign.clone(), mixed.clone()]}
        })
        .to_string(),
    )
    .unwrap();
    run(&home, "install");
    run(&home, "install");
    let settings = read_settings(&home);
    assert_eq!(
        settings["hooks"]["SessionStart"].as_array().unwrap().len(),
        3
    );
    assert_eq!(settings["hooks"]["SessionStart"][0], foreign);
    assert_eq!(settings["hooks"]["SessionStart"][1], mixed);
    run(&home, "uninstall");
    let settings = read_settings(&home);
    assert_eq!(settings["model"], "opus");
    assert_eq!(settings["hooks"]["SessionStart"], json!([foreign, mixed]));
}

#[test]
fn uninstall_leaves_no_placeholder_in_front_of_a_foreign_claude_hook() {
    // Codex trusts hooks by position, so there ours becomes an inert
    // placeholder; Claude does not, so ours must simply disappear.
    let home = TempDir::new().unwrap();
    run(&home, "install");
    let dir = home.path().join(".claude");
    let mut settings = read_settings(&home);
    let foreign = json!({"matcher":"*", "hooks":[{"type":"command","command":"guard.sh"}]});
    settings["hooks"]["Stop"]
        .as_array_mut()
        .unwrap()
        .push(foreign.clone());
    std::fs::write(dir.join("settings.json"), settings.to_string()).unwrap();
    run(&home, "uninstall");
    let settings = read_settings(&home);
    assert_eq!(settings["hooks"]["Stop"], json!([foreign]));
}

#[test]
fn install_writes_the_opencode_plugin_when_detected_and_uninstall_removes_it() {
    let home = TempDir::new().unwrap();
    // opencode "present": its global config dir exists (one of the two detection
    // signals — the other is an `opencode` binary on PATH).
    std::fs::create_dir_all(home.path().join(".config/opencode")).unwrap();

    run(&home, "install");

    let plugin = home.path().join(".config/opencode/plugins/broglow.js");
    assert!(plugin.exists(), "plugin written when opencode is detected");

    let contents = std::fs::read_to_string(&plugin).unwrap();
    assert!(
        contents.contains("managed by broglow"),
        "carries the managed-by header"
    );
    // The version and absolute binary path are baked in — no placeholders left.
    assert!(!contents.contains("{{BIN}}"), "binary path substituted");
    assert!(!contents.contains("{{VERSION}}"), "version substituted");
    assert!(
        contents.contains("\"event\""),
        "plugin drives the `broglow event` backend"
    );

    run(&home, "uninstall");
    assert!(!plugin.exists(), "uninstall removes our plugin");
}

#[test]
fn install_skips_the_plugin_when_opencode_is_absent() {
    let home = TempDir::new().unwrap();
    // No config directory and an isolated empty PATH → detection fails.
    run(&home, "install");
    let plugin = home.path().join(".config/opencode/plugins/broglow.js");
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
    let plugin = plugin_dir.join("broglow.js");
    // A plugin from an older build: our marker (so it's ours to replace), an
    // un-substituted binary placeholder, and a sentinel that must not survive.
    std::fs::write(
        &plugin,
        "// managed by broglow v0.0.1\nconst BROGLOW_BIN = \"{{BIN}}\";\n// STALE_SENTINEL\n",
    )
    .unwrap();

    run(&home, "install");

    let contents = std::fs::read_to_string(&plugin).unwrap();
    assert!(
        !contents.contains("STALE_SENTINEL"),
        "reinstall must overwrite the stale plugin (version-skew fix)"
    );
    assert!(!contents.contains("{{BIN}}"), "binary path re-substituted");
    assert!(contents.contains("managed by broglow"), "still managed");
}

#[test]
fn foreign_opencode_plugins_survive_install_and_uninstall() {
    let home = TempDir::new().unwrap();
    let plugin_dir = home.path().join(".config/opencode/plugins");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    let plugin = plugin_dir.join("broglow.js");
    // A hand-rolled file at our path (no managed-by header) is the user's own
    // opencode↔broglow integration; install must warn and skip, not clobber
    // it — the same guard uninstall applies before deleting.
    let hand_rolled = "// my own hand-rolled plugin\n";
    std::fs::write(&plugin, hand_rolled).unwrap();

    run(&home, "install");

    assert_eq!(
        std::fs::read_to_string(&plugin).unwrap(),
        hand_rolled,
        "install must never overwrite a file without our header"
    );
    run(&home, "uninstall");
    assert_eq!(std::fs::read_to_string(&plugin).unwrap(), hand_rolled);
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
        installer(&home)
            .arg(subcommand)
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
        "Interrupt",
        "SessionEnd",
    ] {
        let groups = hooks["hooks"][event].as_array().unwrap_or_else(|| {
            panic!("{event} registered in codex hooks.json");
        });
        let ours = groups
            .iter()
            .filter_map(|g| g["hooks"][0]["command"].as_str())
            .find(|c| c.ends_with("\" codex-hook"));
        assert!(ours.is_some(), "{event} has a broglow codex-hook group");
        let group = groups
            .iter()
            .find(|g| g["hooks"][0]["command"].as_str() == ours)
            .unwrap();
        assert_eq!(
            group["hooks"][0]["statusMessage"],
            "BroGlow: update session status"
        );
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
        "broglow-only events removed"
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
    let hooks_file = home.path().join(".copilot/hooks/broglow.json");
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
    let ours_path = hooks_dir.join("broglow.json");
    // A hand-rolled file at our path (nothing in it invokes `copilot-hook`)
    // is the user's own configuration; both operations must skip it.
    let hand_rolled =
        r#"{"version":1,"hooks":{"preToolUse":[{"type":"command","command":"./guard.sh"}]}}"#;
    std::fs::write(&ours_path, hand_rolled).unwrap();
    // A sibling hooks file is never broglow's business at all.
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
