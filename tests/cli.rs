//! Cross-platform CLI smoke tests. These must stay safe to run on any machine
//! (including Windows, where the home directory can't be sandboxed via env),
//! so nothing here may read or write real state.

use assert_cmd::Command;

fn clawlight() -> Command {
    Command::cargo_bin("clawlight").expect("binary built")
}

#[test]
fn version_identifies_the_release() {
    clawlight()
        .arg("--version")
        .assert()
        .success()
        .stdout(concat!("clawlight ", env!("CARGO_PKG_VERSION"), "\n"));
}

#[test]
fn help_lists_the_public_subcommands() {
    let assert = clawlight().arg("--help").assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();

    for subcommand in [
        "install",
        "uninstall",
        "menubar",
        "dashboard-button",
        "led",
        "ble",
    ] {
        assert!(stdout.contains(subcommand), "--help mentions {subcommand}");
    }
    assert!(!stdout.contains("\n  update"));
    // Internal backends stay hidden from users.
    for hidden in ["hook", "name"] {
        assert!(
            !stdout.contains(&format!("\n  {hidden}")),
            "{hidden} is hidden"
        );
    }
}

#[cfg(unix)]
#[test]
fn dashboard_visibility_persists_without_touching_other_settings_or_installing() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join(".claude/clawlight/config.json");
    let run = |value: &str| {
        clawlight()
            .env("HOME", home.path())
            .args(["dashboard-button", value])
            .assert()
    };
    run("hide").success();
    let cfg: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(cfg["show_dashboard_button"], false);
    assert_eq!(cfg["led_enabled"], false);
    assert!(!home.path().join(".claude/settings.json").exists());
    assert!(!home.path().join("Library/LaunchAgents").exists());

    let original = serde_json::json!({
        "led_enabled": true, "led_brightness": 37,
        "ble_peripherals": ["home", "office"], "custom": {"keep": true}
    });
    std::fs::write(&path, original.to_string()).unwrap();
    for (value, enabled) in [("show", true), ("hide", false)] {
        run(value).success();
        let cfg: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(cfg["show_dashboard_button"], enabled);
        for (key, value) in original.as_object().unwrap() {
            assert_eq!(&cfg[key], value);
        }
    }
    let saved = std::fs::read(&path).unwrap();
    run("invalid").failure();
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    std::fs::write(&path, "{corrupt").unwrap();
    run("show").failure();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{corrupt");
}

#[cfg(unix)]
#[test]
fn concurrent_settings_commands_preserve_disjoint_edits() {
    use fs4::fs_std::FileExt;
    use std::process::Stdio;

    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(".claude/clawlight");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    let first = "11111111-2222-3333-4444-555555555555";
    let second = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    let original = serde_json::json!({
        "ble_peripherals": [first, second], "led_enabled": false,
        "led_brightness": 64, "custom": {"keep": true}
    });
    std::fs::write(&path, original.to_string()).unwrap();
    let lock = std::fs::File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join(".config.lock"))
        .unwrap();
    lock.lock_exclusive().unwrap();
    let edits = [
        vec!["ble", "rename", first, "Home"],
        vec!["ble", "rename", second, "Office"],
        vec!["ble", "brightness", "23", "--id", first],
        vec!["ble", "brightness", "87", "--id", second],
        vec!["dashboard-button", "show"],
    ];
    let mut children: Vec<_> = edits
        .iter()
        .map(|args| {
            std::process::Command::new(clawlight().get_program())
                .env("HOME", home.path())
                .args(args)
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    std::thread::sleep(std::time::Duration::from_millis(150));
    let all_waited_for_lock = children
        .iter_mut()
        .all(|child| child.try_wait().unwrap().is_none());
    drop(lock);
    for child in children {
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success(), "{:?}", output);
    }
    assert!(
        all_waited_for_lock,
        "every settings command must respect the shared lock"
    );
    let cfg: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        cfg["lamp_names"],
        serde_json::json!({first: "Home", second: "Office"})
    );
    assert_eq!(
        cfg["lamp_brightness"],
        serde_json::json!({first: 23, second: 87})
    );
    assert_eq!(cfg["show_dashboard_button"], true);
    for (key, value) in original.as_object().unwrap() {
        assert_eq!(&cfg[key], value, "preserve unrelated {key}");
    }
}

#[test]
fn ble_help_documents_selection_without_accessing_the_radio() {
    let assert = clawlight().args(["ble", "--help"]).assert().success();
    let help = String::from_utf8_lossy(&assert.get_output().stdout);
    for command in [
        "scan", "pair", "add", "select", "remove", "list", "usb", "off", "status",
    ] {
        assert!(help.contains(command));
    }
    clawlight().args(["ble", "select"]).assert().failure();
    clawlight()
        .args(["ble", "pair", "--help"])
        .assert()
        .success();
}

#[test]
fn unknown_subcommands_fail() {
    clawlight().arg("no-such-command").assert().failure();
}

#[test]
fn the_hook_exits_cleanly_under_the_naming_guard() {
    // CLAWLIGHT_NAMING short-circuits before any state I/O, so this is safe to
    // run against the real home directory on every platform.
    clawlight()
        .arg("hook")
        .env("CLAWLIGHT_NAMING", "1")
        .write_stdin("")
        .assert()
        .success();
}

#[test]
fn the_event_backend_exits_cleanly_on_junk_input() {
    // Malformed stdin must be a safe no-op, like the hook backend. Uses a
    // nonexistent HOME override on unix so nothing real is written; on Windows
    // the parse fails before any state path is touched.
    let mut cmd = clawlight();
    cmd.arg("event").write_stdin("not an event {{{");
    #[cfg(unix)]
    cmd.env("HOME", "/nonexistent-clawlight-test-home");
    cmd.assert().success();
}

#[test]
fn the_copilot_shim_exits_cleanly_on_junk_input() {
    // Same contract as the other stdin backends: malformed payloads are a
    // safe no-op. The event name is argv, so an unknown one must no-op too.
    for (event, stdin) in [("agentStop", "not json {{{"), ("someFutureEvent", "{}")] {
        let mut cmd = clawlight();
        cmd.args(["copilot-hook", event]).write_stdin(stdin);
        #[cfg(unix)]
        cmd.env("HOME", "/nonexistent-clawlight-test-home");
        cmd.assert().success();
    }
}

#[test]
fn the_opencode_plugin_parses_as_a_js_module() {
    // Optional parse check: runs only where `node` is on PATH (the plugin's
    // real coverage is the manual matrix). Skips cleanly otherwise — including
    // on Windows, where `on_path` doesn't probe the `.exe` suffix. `node
    // --check` is the portable syntax-only check; `bun` has no equivalent flag,
    // so it's not probed.
    if !on_path("node") {
        eprintln!("skipping plugin parse check: no node on PATH");
        return;
    }
    let node = "node";

    let asset = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/opencode-plugin.js");
    let contents = std::fs::read_to_string(asset).expect("read plugin asset");
    // ES-module syntax parses only with an .mjs extension, so copy it out.
    let dir = tempfile::TempDir::new().unwrap();
    let mjs = dir.path().join("clawlight.mjs");
    std::fs::write(&mjs, contents).unwrap();

    let output = std::process::Command::new(node)
        .arg("--check")
        .arg(&mjs)
        .output()
        .expect("run the JS engine");
    assert!(
        output.status.success(),
        "the embedded plugin must be valid JS:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Whether `program` resolves to a file on PATH. Deliberately simple — it only
/// gates the optional parse check above, so a false negative just skips it.
fn on_path(program: &str) -> bool {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .any(|d| d.join(program).is_file())
}

#[cfg(target_os = "macos")]
#[test]
fn usb_pair_failure_restores_configuration_and_forwards_the_explicit_port() {
    let home = tempfile::tempdir().unwrap();
    let bundled = home
        .path()
        .join("Applications/ClawLight.app/Contents/MacOS/clawlight");
    std::fs::create_dir_all(bundled.parent().unwrap()).unwrap();
    std::fs::copy(clawlight().get_program(), &bundled).unwrap();
    let dir = home.path().join(".claude/clawlight");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    let original = r#"{"led_enabled":true,"ble_peripherals":["keep"],"unrelated":{"retain":true}}"#;
    std::fs::write(&path, original).unwrap();
    let missing_port = home.path().join("nonexistent-serial-port");
    let invoke = || {
        clawlight()
            .env("HOME", home.path())
            .args(["ble", "pair", "--port"])
            .arg(&missing_port)
            .assert()
            .failure()
    };
    let failed = invoke();
    assert!(String::from_utf8_lossy(&failed.get_output().stderr).contains("Opening USB lamp"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    {
        use fs4::fs_std::FileExt;
        let lock = std::fs::File::open(dir.join(".pair.lock")).unwrap();
        assert!(lock.try_lock_exclusive().unwrap());
        let failed = invoke();
        assert!(String::from_utf8_lossy(&failed.get_output().stderr)
            .contains("Lamp setup is already running"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }
    {
        use fs4::fs_std::FileExt;
        use std::process::Stdio;
        use std::time::{Duration, Instant};

        // Keep setup waiting for its worker, then edit another setting through
        // the real CLI. Setup must not hold the config lock across that wait.
        let worker = std::fs::File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(dir.join(".lamp.lock"))
            .unwrap();
        worker.lock_exclusive().unwrap();
        let child = std::process::Command::new(clawlight().get_program())
            .env("HOME", home.path())
            .args(["ble", "pair", "--port"])
            .arg(&missing_port)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let cfg: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            if cfg["led_enabled"] == false {
                break;
            }
            assert!(Instant::now() < deadline, "setup did not pause its worker");
            std::thread::sleep(Duration::from_millis(10));
        }
        clawlight()
            .env("HOME", home.path())
            .timeout(Duration::from_secs(3))
            .args(["dashboard-button", "show"])
            .assert()
            .success();
        drop(worker);
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Opening USB lamp"));
        let cfg: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            cfg["led_enabled"], true,
            "rollback restores previous enablement"
        );
        assert_eq!(
            cfg["show_dashboard_button"], true,
            "rollback preserves the concurrent edit"
        );
        assert_eq!(cfg["ble_peripherals"], serde_json::json!(["keep"]));
        assert_eq!(cfg["unrelated"], serde_json::json!({"retain": true}));
    }
    std::fs::remove_file(&path).unwrap();
    invoke();
    assert!(
        !path.exists(),
        "failed setup preserves a previously absent config"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn remembered_lamps_migrate_add_remove_and_list_without_radio() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(".claude/clawlight");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    let first = "11111111-2222-3333-4444-555555555555";
    let second = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    std::fs::write(
        &path,
        serde_json::json!({"ble_peripheral":first,"led_brightness":37,"custom":42}).to_string(),
    )
    .unwrap();
    let run = |args: &[&str]| {
        clawlight()
            .env("HOME", home.path())
            .args(args)
            .assert()
            .success();
    };
    run(&["ble", "add", &second.to_uppercase()]);
    run(&["ble", "select", first]);
    run(&["ble", "rename", first, "  Дом 💡  "]);
    run(&["ble", "rename", second, "Office"]);
    run(&["ble", "brightness", "0", "--id", first]);
    run(&["ble", "brightness", "100", "--id", second]);
    let bytes = std::fs::read(&path).unwrap();
    let cfg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(cfg["ble_peripherals"], serde_json::json!([first, second]));
    assert!(cfg.get("ble_peripheral").is_none());
    assert_eq!(cfg["custom"], 42);
    assert_eq!(cfg["lamp_names"][first], "Дом 💡");
    assert_eq!(cfg["lamp_names"][second], "Office");
    for name in ["a".repeat(41), "a\nb".into()] {
        clawlight()
            .env("HOME", home.path())
            .args(["ble", "rename", first, &name])
            .assert()
            .failure();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    run(&["ble", "rename", first, "   "]);
    let cleared: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert!(cleared["lamp_names"].get(first).is_none());
    assert_eq!(cleared["lamp_names"][second], "Office");
    run(&["ble", "rename", first, "Дом 💡"]);
    let listed = clawlight()
        .env("HOME", home.path())
        .args(["ble", "list"])
        .assert()
        .success();
    let output = String::from_utf8_lossy(&listed.get_output().stdout);
    assert!(output.contains(first) && output.contains(second));
    assert_eq!(std::fs::read(&path).unwrap(), bytes, "list is read-only");
    run(&["ble", "remove", &second.to_uppercase()]);
    let remaining = std::fs::read(&path).unwrap();
    let cfg: serde_json::Value = serde_json::from_slice(&remaining).unwrap();
    assert_eq!(cfg["ble_peripherals"], serde_json::json!([first]));
    assert_eq!(cfg["lamp_names"], serde_json::json!({first:"Дом 💡"}));
    assert_eq!(cfg["lamp_brightness"], serde_json::json!({first:0}));
    assert_eq!(cfg["led_enabled"], true);
    assert_eq!(cfg["led_brightness"], 37);
    assert_eq!(cfg["custom"], 42);
    for id in [second, "invalid"] {
        clawlight()
            .env("HOME", home.path())
            .args(["ble", "remove", id])
            .assert()
            .failure();
        assert_eq!(std::fs::read(&path).unwrap(), remaining);
    }
    run(&["ble", "remove", first]);
    let cfg: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(cfg["ble_peripherals"], serde_json::json!([]));
    assert!(
        cfg.get("lamp_names").is_none(),
        "forget removes the saved name"
    );
    assert!(
        cfg.get("lamp_brightness").is_none(),
        "forget removes brightness overrides"
    );
    assert_eq!(cfg["led_enabled"], false);
    assert_eq!(cfg["led_brightness"], 37);
    assert_eq!(cfg["custom"], 42);
    std::fs::write(&path, "{corrupt").unwrap();
    clawlight()
        .env("HOME", home.path())
        .args(["ble", "remove", first])
        .assert()
        .failure();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "{corrupt");
}

#[cfg(target_os = "macos")]
#[test]
fn brightness_settings_are_independent_and_save_offline_without_enabling() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(".claude/clawlight");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    let first = "11111111-2222-3333-4444-555555555555";
    let second = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    std::fs::write(
        &path,
        serde_json::json!({
            "ble_peripheral": first, "ble_peripherals": [second],
            "led_brightness": 37, "led_enabled": false, "led_transport": "usb",
            "led_port": "keep", "custom": 42
        })
        .to_string(),
    )
    .unwrap();
    let run = |args: &[&str]| {
        clawlight()
            .env("HOME", home.path())
            .args(args)
            .assert()
            .success();
    };
    run(&["ble", "brightness", "0", "--id", first]);
    run(&["ble", "brightness", "100", "--id", &second.to_uppercase()]);
    let bytes = std::fs::read(&path).unwrap();
    let cfg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        cfg["lamp_brightness"],
        serde_json::json!({first:0, second:100})
    );
    assert_eq!(cfg["led_brightness"], 37);
    assert_eq!(cfg["led_enabled"], false);
    assert_eq!(cfg["led_transport"], "usb");
    assert_eq!(cfg["led_port"], "keep");
    assert_eq!(cfg["custom"], 42);
    assert!(!dir.join("lamp-status.json").exists());
    for args in [
        vec!["ble", "brightness", "101", "--id", first],
        vec!["ble", "brightness", "101"],
        vec!["ble", "brightness", "40", "--id", "invalid"],
        vec![
            "ble",
            "brightness",
            "40",
            "--id",
            "00000000-0000-0000-0000-000000000000",
        ],
    ] {
        clawlight()
            .env("HOME", home.path())
            .args(args)
            .assert()
            .failure();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    run(&["ble", "brightness", "40"]);
    let cfg: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(cfg["led_brightness"], 40);
    assert!(
        cfg.get("lamp_brightness").is_none(),
        "All lamps clears overrides"
    );
    assert_eq!(cfg["ble_peripheral"], first);
    assert_eq!(cfg["ble_peripherals"], serde_json::json!([second]));
    assert_eq!(cfg["led_enabled"], false);
    assert_eq!(cfg["custom"], 42);
    std::fs::write(&path, "{corrupt").unwrap();
    for args in [
        vec!["ble", "brightness", "20"],
        vec!["ble", "brightness", "20", "--id", first],
    ] {
        clawlight()
            .env("HOME", home.path())
            .args(args)
            .assert()
            .failure();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "{corrupt");
    }
}

#[cfg(target_os = "macos")]
#[test]
fn ble_selection_preserves_settings_without_radio() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(".claude/clawlight");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    std::fs::write(
        &path,
        r#"{"custom":42,"led_port":"keep","yellow_mode":"active_wins"}"#,
    )
    .unwrap();
    let id = "11111111-2222-3333-4444-555555555555";
    clawlight()
        .env("HOME", home.path())
        .args(["ble", "select", id])
        .assert()
        .success();
    let selected = std::fs::read(&path).unwrap();
    let cfg: serde_json::Value = serde_json::from_slice(&selected).unwrap();
    assert_eq!(cfg["ble_peripherals"], serde_json::json!([id]));
    assert_eq!(cfg["led_transport"], "ble");
    assert_eq!(cfg["led_enabled"], true);
    assert_eq!(cfg["custom"], 42);
    assert_eq!(cfg["led_port"], "keep");
    clawlight()
        .env("HOME", home.path())
        .args(["ble", "select", "invalid"])
        .assert()
        .failure();
    assert_eq!(std::fs::read(&path).unwrap(), selected);
    clawlight()
        .env("HOME", home.path())
        .args(["ble", "off"])
        .assert()
        .success();
    let cfg: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(cfg["ble_peripherals"], serde_json::json!([id]));
    assert_eq!(cfg["led_enabled"], false);
    std::fs::write(&path, "{corrupt").unwrap();
    clawlight()
        .env("HOME", home.path())
        .args(["ble", "usb"])
        .assert()
        .failure();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "{corrupt");
}
