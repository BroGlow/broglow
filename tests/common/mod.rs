//! Fixtures shared by the binary integration suites.
// Each test file is a separate crate and uses only its relevant fixture helpers.
#![allow(dead_code)]

use std::path::PathBuf;

use assert_cmd::Command;
use serde_json::Value;
use tempfile::TempDir;

pub fn broglow() -> Command {
    Command::cargo_bin("broglow").expect("binary built")
}

#[cfg(unix)]
pub fn temp_home() -> TempDir {
    TempDir::new().expect("create temp home")
}

/// Isolate every agent's files and prevent an install from starting a real tray.
/// Only available where dirs::home_dir honors HOME.
#[cfg(unix)]
pub fn command(home: &TempDir) -> Command {
    let mut command = broglow();
    command
        .env("HOME", home.path())
        .env("CODEX_HOME", codex_home(home))
        .env("COPILOT_HOME", home.path().join(".copilot"))
        .env_remove("BROGLOW_NAMING")
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY");
    command
}

pub fn state_path(home: &TempDir) -> PathBuf {
    home.path().join(".broglow/state.json")
}

pub fn codex_home(home: &TempDir) -> PathBuf {
    home.path().join(".codex")
}

pub fn read_state(home: &TempDir) -> Value {
    serde_json::from_slice(&std::fs::read(state_path(home)).expect("state.json exists"))
        .expect("state.json parses")
}

pub fn seed_state(home: &TempDir, state: &Value) {
    let path = state_path(home);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, state.to_string()).unwrap();
}

pub fn session(state: &Value, session_id: &str) -> Value {
    state["sessions"][session_id].clone()
}

pub fn session_status<'a>(state: &'a Value, session_id: &str) -> &'a str {
    state["sessions"][session_id]["status"]
        .as_str()
        .unwrap_or_else(|| panic!("session {session_id} has a status"))
}

#[cfg(unix)]
pub fn run_hook(home: &TempDir, stdin: &str) -> assert_cmd::assert::Assert {
    command(home)
        .arg("hook")
        .write_stdin(stdin.to_string())
        .assert()
}
