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
