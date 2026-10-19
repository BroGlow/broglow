//! Shared helper for spawning detached, windowless child processes on Windows.
//!
//! Every detached spawn takes its `CREATE_NO_WINDOW` / `DETACHED_PROCESS`
//! flags from here, so callers cannot drift apart.

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

/// Apply the flags needed to spawn a fully detached, windowless process on
/// Windows. No-op on other platforms.
///
/// Flags:
/// - `CREATE_NO_WINDOW` (0x0800_0000) — prevents Windows from allocating a
///   fresh console window for the child process.
/// - `DETACHED_PROCESS` (0x0000_0008) — prevents the child from inheriting
///   the parent's console, so it survives the parent exiting/closing its
///   console without being torn down with it.
pub fn configure_detached(cmd: &mut std::process::Command) {
    #[cfg(target_os = "windows")]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = cmd;
    }
}
