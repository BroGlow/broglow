//! Cross-platform CLI smoke tests. These must stay safe to run on any machine
//! (including Windows, where the home directory can't be sandboxed via env),
//! so nothing here may read or write real state.

use assert_cmd::Command;

fn clawlight() -> Command {
    Command::cargo_bin("clawlight").expect("binary built")
}

#[test]
fn help_lists_the_public_subcommands() {
    let assert = clawlight().arg("--help").assert().success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();

    for subcommand in ["install", "uninstall", "menubar", "led", "update"] {
        assert!(stdout.contains(subcommand), "--help mentions {subcommand}");
    }
    // Internal backends stay hidden from users.
    for hidden in ["hook", "name"] {
        assert!(
            !stdout.contains(&format!("\n  {hidden}")),
            "{hidden} is hidden"
        );
    }
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
