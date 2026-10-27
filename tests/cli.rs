//! CLI behavior through the compiled binary. Settings tests are Unix-only:
//! dirs::home_dir honors HOME there, so each test can use a temporary home.
//! Cross-platform smoke checks must never read or write real state.

mod common;

use common::broglow;

#[test]
fn version_identifies_the_release() {
    broglow()
        .arg("--version")
        .assert()
        .success()
        .stdout(concat!("broglow ", env!("CARGO_PKG_VERSION"), "\n"));
}

#[cfg(unix)]
#[test]
fn dashboard_visibility_persists_without_touching_other_settings_or_installing() {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join(".claude/broglow/config.json");
    let run = |value: &str| {
        broglow()
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
    use std::process::Stdio;

    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(".claude/broglow");
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
    lock.lock().unwrap();
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
            std::process::Command::new(broglow().get_program())
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

#[cfg(target_os = "macos")]
#[test]
fn usb_pair_failure_restores_configuration_and_forwards_the_explicit_port() {
    let home = tempfile::tempdir().unwrap();
    let bundled = home
        .path()
        .join("Applications/BroGlow.app/Contents/MacOS/broglow");
    std::fs::create_dir_all(bundled.parent().unwrap()).unwrap();
    std::fs::copy(broglow().get_program(), &bundled).unwrap();
    let dir = home.path().join(".claude/broglow");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    let original = r#"{"led_enabled":true,"ble_peripherals":["keep"],"unrelated":{"retain":true}}"#;
    std::fs::write(&path, original).unwrap();
    let missing_port = home.path().join("nonexistent-serial-port");
    let invoke = |automatic| {
        let mut command = broglow();
        if automatic {
            command.arg("ble").arg("pair").arg("--automatic");
        } else {
            command.args(["ble", "pair"]);
        }
        command
            .env("HOME", home.path())
            .arg("--port")
            .arg(&missing_port)
            .assert()
            .failure()
    };
    let failed = invoke(false);
    assert!(String::from_utf8_lossy(&failed.get_output().stderr).contains("Opening USB lamp"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    let failed = invoke(true);
    assert!(String::from_utf8_lossy(&failed.get_output().stderr).contains("Opening USB lamp"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    {
        let lock = std::fs::File::open(dir.join(".pair.lock")).unwrap();
        lock.try_lock().unwrap();
        let failed = invoke(true);
        assert!(String::from_utf8_lossy(&failed.get_output().stderr)
            .contains("Lamp setup is already running"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
    }
    {
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
        worker.lock().unwrap();
        let child = std::process::Command::new(broglow().get_program())
            .env("HOME", home.path())
            .args(["ble", "pair", "--automatic", "--port"])
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
        broglow()
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
    invoke(true);
    assert!(
        !path.exists(),
        "failed setup preserves a previously absent config"
    );
}

#[cfg(target_os = "macos")]
#[test]
fn remembered_lamps_add_remove_and_list_without_radio() {
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join(".claude/broglow");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    let first = "11111111-2222-3333-4444-555555555555";
    let second = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    std::fs::write(
        &path,
        serde_json::json!({"ble_peripherals":[first],"led_brightness":37,"custom":42}).to_string(),
    )
    .unwrap();
    let run = |args: &[&str]| {
        broglow()
            .env("HOME", home.path())
            .args(args)
            .assert()
            .success();
    };
    run(&["ble", "add", &second.to_uppercase()]);
    run(&["ble", "add", first]);
    run(&["ble", "rename", first, "  Дом 💡  "]);
    run(&["ble", "rename", second, "Office"]);
    run(&["ble", "brightness", "0", "--id", first]);
    run(&["ble", "brightness", "100", "--id", second]);
    let bytes = std::fs::read(&path).unwrap();
    let cfg: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(cfg["ble_peripherals"], serde_json::json!([first, second]));
    assert_eq!(cfg["custom"], 42);
    assert_eq!(cfg["lamp_names"][first], "Дом 💡");
    assert_eq!(cfg["lamp_names"][second], "Office");
    for name in ["a".repeat(41), "a\nb".into()] {
        broglow()
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
    let listed = broglow()
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
        broglow()
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
    broglow()
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
    let dir = home.path().join(".claude/broglow");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    let first = "11111111-2222-3333-4444-555555555555";
    let second = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
    std::fs::write(
        &path,
        serde_json::json!({
            "ble_peripherals": [first, second],
            "led_brightness": 37, "led_enabled": false, "led_transport": "usb",
            "led_port": "keep", "custom": 42
        })
        .to_string(),
    )
    .unwrap();
    let run = |args: &[&str]| {
        broglow()
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
        broglow()
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
    assert_eq!(cfg["ble_peripherals"], serde_json::json!([first, second]));
    assert_eq!(cfg["led_enabled"], false);
    assert_eq!(cfg["custom"], 42);
    std::fs::write(&path, "{corrupt").unwrap();
    for args in [
        vec!["ble", "brightness", "20"],
        vec!["ble", "brightness", "20", "--id", first],
    ] {
        broglow()
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
    let dir = home.path().join(".claude/broglow");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    std::fs::write(
        &path,
        r#"{"custom":42,"led_port":"keep","yellow_mode":"active_wins"}"#,
    )
    .unwrap();
    let id = "11111111-2222-3333-4444-555555555555";
    broglow()
        .env("HOME", home.path())
        .args(["ble", "add", id])
        .assert()
        .success();
    let selected = std::fs::read(&path).unwrap();
    let cfg: serde_json::Value = serde_json::from_slice(&selected).unwrap();
    assert_eq!(cfg["ble_peripherals"], serde_json::json!([id]));
    assert_eq!(cfg["led_transport"], "ble");
    assert_eq!(cfg["led_enabled"], true);
    assert_eq!(cfg["custom"], 42);
    assert_eq!(cfg["led_port"], "keep");
    broglow()
        .env("HOME", home.path())
        .args(["ble", "add", "invalid"])
        .assert()
        .failure();
    assert_eq!(std::fs::read(&path).unwrap(), selected);
    broglow()
        .env("HOME", home.path())
        .args(["ble", "off"])
        .assert()
        .success();
    let cfg: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(cfg["ble_peripherals"], serde_json::json!([id]));
    assert_eq!(cfg["led_enabled"], false);
    broglow()
        .env("HOME", home.path())
        .args(["ble", "on"])
        .assert()
        .success();
    let cfg: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(cfg["ble_peripherals"], serde_json::json!([id]));
    assert_eq!(cfg["led_transport"], "ble");
    assert_eq!(cfg["led_enabled"], true);
    std::fs::write(&path, "{corrupt").unwrap();
    broglow()
        .env("HOME", home.path())
        .args(["ble", "usb"])
        .assert()
        .failure();
    assert_eq!(std::fs::read_to_string(path).unwrap(), "{corrupt");
}

#[cfg(unix)]
#[test]
fn flash_needs_release_images_and_a_reachable_lamp_without_touching_settings() {
    let home = tempfile::tempdir().unwrap();
    let config = home.path().join(".claude/broglow/config.json");
    let missing_port = home.path().join("nonexistent-serial-port");

    // The test binary has no release layout around it.
    let failed = broglow()
        .env("HOME", home.path())
        .args(["flash", "--yes", "--port"])
        .arg(&missing_port)
        .assert()
        .failure();
    assert!(String::from_utf8_lossy(&failed.get_output().stderr).contains("--firmware"));

    let images = home.path().join("images");
    std::fs::create_dir(&images).unwrap();
    for name in ["bootloader.bin", "partitions.bin", "boot_app0.bin"] {
        std::fs::write(images.join(name), [0xAA; 16]).unwrap();
    }
    std::fs::write(images.join("firmware.bin"), b"\0lamp=%s\nversion=9.9.9\n").unwrap();
    let failed = broglow()
        .env("HOME", home.path())
        .args(["flash", "--yes", "--firmware"])
        .arg(&images)
        .arg("--port")
        .arg(&missing_port)
        .assert()
        .failure();
    assert!(String::from_utf8_lossy(&failed.get_output().stderr).contains("Opening USB lamp"));
    // Lamp control was off, so nothing was paused and no settings file appeared.
    assert!(!config.exists());
}
