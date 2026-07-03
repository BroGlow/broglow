//! Which terminal window hosts a session, and how to get the user back to it.
//!
//! The hook backend (`clawlight hook`) runs as a descendant of whatever app
//! hosts the Claude Code session — a terminal emulator, an IDE's integrated
//! terminal, or the desktop app — so its inherited environment and process
//! tree identify the exact window. [`capture`] records that identity into the
//! session's `state.json` entry; [`focus`] uses it later (from the tray
//! popover, the Linux tray menu, or the TUI's ↵) to raise the right
//! window/tab.
//!
//! Everything here is best-effort: focusing shells out (osascript / open /
//! tmux / xdotool), so callers on a UI event loop should run it off-thread.

use crate::state::{Ancestor, TerminalInfo};
#[cfg(unix)]
use std::process::Command;

/// Snapshot the hosting terminal's identity from this process's environment
/// and ancestry. Called by the hook backend while handling an event.
pub fn capture() -> TerminalInfo {
    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.is_empty());
    TerminalInfo {
        term_program: env("TERM_PROGRAM"),
        term_session_id: env("TERM_SESSION_ID"),
        iterm_session_id: env("ITERM_SESSION_ID"),
        bundle_id: env("__CFBundleIdentifier"),
        wt_session: env("WT_SESSION"),
        kitty_window_id: env("KITTY_WINDOW_ID"),
        wezterm_pane: env("WEZTERM_PANE"),
        // TMUX_PANE can linger in the environment after leaving tmux; only
        // trust it while TMUX itself is set.
        tmux_pane: env("TMUX").and_then(|_| env("TMUX_PANE")),
        tty: capture_tty(),
        ancestors: capture_ancestors(),
    }
}

/// Terminal device of the session ("/dev/ttys012"). Claude Code spawns hook
/// subprocesses without a controlling tty, so walk up the parent chain (one
/// `ps` snapshot) to the claude process, which does own the terminal's tty.
#[cfg(unix)]
fn capture_tty() -> Option<String> {
    let out = Command::new("ps")
        .args(["-axo", "pid=,ppid=,tty="])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut table = std::collections::HashMap::new();
    for line in stdout.lines() {
        let mut cols = line.split_whitespace();
        let (Some(pid), Some(ppid), Some(tty)) = (cols.next(), cols.next(), cols.next()) else {
            continue;
        };
        let (Ok(pid), Ok(ppid)) = (pid.parse::<u32>(), ppid.parse::<u32>()) else {
            continue;
        };
        table.insert(pid, (ppid, tty.to_string()));
    }

    let mut pid = std::process::id();
    for _ in 0..16 {
        let (ppid, tty) = table.get(&pid)?;
        // ps prints "??" (macOS) / "?" (Linux) when there is no controlling tty.
        if !tty.is_empty() && !tty.starts_with('?') && tty != "-" {
            return Some(if tty.starts_with("/dev/") {
                tty.clone()
            } else {
                format!("/dev/{tty}")
            });
        }
        if *ppid <= 1 || *ppid == pid {
            return None;
        }
        pid = *ppid;
    }
    None
}

#[cfg(not(unix))]
fn capture_tty() -> Option<String> {
    None
}

/// Ancestor process chain, nearest first. Only captured where focusing goes
/// through the window-owning process (Windows / Linux); macOS resolves
/// windows by tty / session id / bundle id instead.
#[cfg(target_os = "linux")]
fn capture_ancestors() -> Option<Vec<Ancestor>> {
    let mut chain = Vec::new();
    let mut pid = std::os::unix::process::parent_id();
    while pid > 1 && chain.len() < 16 {
        let Some((name, ppid)) = proc_name_ppid(pid) else {
            break;
        };
        chain.push(Ancestor { pid, name });
        if ppid == pid {
            break;
        }
        pid = ppid;
    }
    (!chain.is_empty()).then_some(chain)
}

#[cfg(target_os = "linux")]
fn proc_name_ppid(pid: u32) -> Option<(String, u32)> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let mut name = None;
    let mut ppid = None;
    for line in status.lines() {
        if let Some(v) = line.strip_prefix("Name:") {
            name = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("PPid:") {
            ppid = v.trim().parse().ok();
        }
    }
    Some((name?, ppid?))
}

#[cfg(target_os = "windows")]
fn capture_ancestors() -> Option<Vec<Ancestor>> {
    let table = process_table()?;
    let mut chain = Vec::new();
    // PID reuse can make ppid links loop; the seen-set breaks cycles.
    let mut seen = std::collections::HashSet::new();
    let mut pid = table.get(&std::process::id())?.0;
    while pid > 4 && chain.len() < 16 && seen.insert(pid) {
        let Some((ppid, name)) = table.get(&pid) else {
            break;
        };
        chain.push(Ancestor {
            pid,
            name: name.clone(),
        });
        pid = *ppid;
    }
    (!chain.is_empty()).then_some(chain)
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn capture_ancestors() -> Option<Vec<Ancestor>> {
    None
}

/// pid → (parent pid, exe name) for every running process.
#[cfg(target_os = "windows")]
fn process_table() -> Option<std::collections::HashMap<u32, (u32, String)>> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut table = std::collections::HashMap::new();
        if Process32FirstW(snap, &mut entry) != 0 {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                table.insert(entry.th32ProcessID, (entry.th32ParentProcessID, name));
                if Process32NextW(snap, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snap);
        Some(table)
    }
}

// ---------------------------------------------------------------------------
// Focus
// ---------------------------------------------------------------------------

/// Bring the window hosting a session to the front. Returns `true` when a
/// window (or at least the hosting app) was raised. Blocking — can shell out
/// for a few hundred ms.
#[cfg(target_os = "macos")]
pub fn focus(info: Option<&TerminalInfo>, project_path: Option<&str>) -> bool {
    if let Some(t) = info {
        if focus_macos(t, project_path) {
            return true;
        }
    }
    // Sessions recorded before terminal capture existed: fall back to
    // scanning Terminal.app window titles for the project directory name.
    project_path.is_some_and(focus_terminal_by_title)
}

/// IDE bundles that focus the window showing a folder when asked to "open"
/// that folder again — so `open -b <bundle> <project>` lands on the right
/// window, not just the app.
#[cfg(target_os = "macos")]
const EDITOR_BUNDLES: [&str; 5] = [
    "com.microsoft.VSCode",
    "com.microsoft.VSCodeInsiders",
    "com.vscodium",
    "com.todesktop.230313mzl4w4u92", // Cursor
    "com.exafunction.windsurf",
];

#[cfg(target_os = "macos")]
fn focus_macos(t: &TerminalInfo, project_path: Option<&str>) -> bool {
    // Inside tmux, the pane is the real identity; the captured terminal vars
    // describe wherever the tmux *server* was started, which may be long gone.
    if let Some(pane) = &t.tmux_pane {
        if focus_tmux(pane) {
            return true;
        }
    }

    match t.term_program.as_deref() {
        Some("Apple_Terminal") => {
            if let Some(tty) = &t.tty {
                if focus_terminal_tab_by_tty(tty) {
                    return true;
                }
            }
        }
        Some("iTerm.app") => {
            let uuid = t
                .iterm_session_id
                .as_deref()
                .and_then(|s| s.split_once(':'))
                .map(|(_, uuid)| uuid);
            if focus_iterm_session(uuid, t.tty.as_deref()) {
                return true;
            }
        }
        _ => {}
    }

    // Terminals with a control CLI: select the exact window/pane first, then
    // fall through to activating the app itself.
    if let Some(id) = &t.kitty_window_id {
        let _ = Command::new("kitty")
            .args(["@", "focus-window", "--match", &format!("id:{id}")])
            .output();
    }
    if let Some(pane) = &t.wezterm_pane {
        let _ = Command::new("wezterm")
            .args(["cli", "activate-pane", "--pane-id", pane])
            .output();
    }

    // Generic: activate whatever app hosted the session (Ghostty, Warp,
    // Alacritty, kitty, WezTerm, the Claude desktop app, …). For IDEs, open
    // the project folder so the right *window* comes forward.
    if let Some(bundle) = &t.bundle_id {
        if let Some(path) = project_path {
            if EDITOR_BUNDLES.contains(&bundle.as_str()) && open_bundle(bundle, Some(path)) {
                return true;
            }
        }
        if open_bundle(bundle, None) {
            return true;
        }
    }
    false
}

/// Reveal a tmux pane and raise the terminal window of the client viewing it.
#[cfg(target_os = "macos")]
fn focus_tmux(pane: &str) -> bool {
    let run = |args: &[&str]| -> Option<String> {
        let out = Command::new("tmux").args(args).output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };

    // Select the pane's window inside tmux; failure means the pane (or the
    // whole server) is gone.
    if run(&["select-window", "-t", pane]).is_none() {
        return false;
    }
    let _ = run(&["select-pane", "-t", pane]);

    let Some(session) = run(&["display-message", "-p", "-t", pane, "#{session_name}"]) else {
        return false;
    };
    // Prefer a client already attached to the pane's session; otherwise
    // retarget the first client at it.
    let Some(clients) = run(&["list-clients", "-F", "#{client_tty}\t#{client_session}"]) else {
        return false;
    };
    let mut fallback: Option<&str> = None;
    let mut chosen: Option<&str> = None;
    for line in clients.lines() {
        let Some((tty, sess)) = line.split_once('\t') else {
            continue;
        };
        if sess == session {
            chosen = Some(tty);
            break;
        }
        fallback.get_or_insert(tty);
    }
    let client_tty = match (chosen, fallback) {
        (Some(tty), _) => tty.to_string(),
        (None, Some(tty)) => {
            let _ = run(&["switch-client", "-c", tty, "-t", &session]);
            tty.to_string()
        }
        (None, None) => return false, // detached session — no window to raise
    };

    focus_terminal_tab_by_tty(&client_tty) || focus_iterm_session(None, Some(&client_tty))
}

/// Select the Terminal.app tab running on the given tty and raise its window.
#[cfg(target_os = "macos")]
fn focus_terminal_tab_by_tty(tty: &str) -> bool {
    let tty = applescript_escape(tty);
    let script = format!(
        "if application \"Terminal\" is not running then return \"missing\"\n\
         tell application \"Terminal\"\n\
         repeat with w in windows\n\
         repeat with t in tabs of w\n\
         if tty of t is \"{tty}\" then\n\
         set selected of t to true\n\
         set index of w to 1\n\
         set frontmost of w to true\n\
         activate\n\
         return \"found\"\n\
         end if\n\
         end repeat\n\
         end repeat\n\
         end tell\n\
         return \"missing\""
    );
    run_osascript(&script)
}

/// Select the iTerm2 session matching the captured session UUID or tty.
#[cfg(target_os = "macos")]
fn focus_iterm_session(uuid: Option<&str>, tty: Option<&str>) -> bool {
    let mut conds = Vec::new();
    if let Some(u) = uuid {
        conds.push(format!("(id of s is \"{}\")", applescript_escape(u)));
    }
    if let Some(t) = tty {
        conds.push(format!("(tty of s is \"{}\")", applescript_escape(t)));
    }
    if conds.is_empty() {
        return false;
    }
    let cond = conds.join(" or ");
    let script = format!(
        "if application \"iTerm2\" is not running then return \"missing\"\n\
         tell application \"iTerm2\"\n\
         repeat with w in windows\n\
         repeat with t in tabs of w\n\
         repeat with s in sessions of t\n\
         if {cond} then\n\
         select s\n\
         select t\n\
         select w\n\
         activate\n\
         return \"found\"\n\
         end if\n\
         end repeat\n\
         end repeat\n\
         end repeat\n\
         end tell\n\
         return \"missing\""
    );
    run_osascript(&script)
}

/// Legacy fallback: raise the Terminal.app window whose title mentions the
/// project directory name.
#[cfg(target_os = "macos")]
fn focus_terminal_by_title(project_path: &str) -> bool {
    let name = std::path::Path::new(project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| project_path.to_string());
    if name.is_empty() {
        return false;
    }
    let escaped = applescript_escape(&name);
    let script = format!(
        "if application \"Terminal\" is not running then return \"missing\"\n\
         tell application \"Terminal\"\n\
         repeat with w in windows\n\
         if name of w contains \"{escaped}\" then\n\
         set index of w to 1\n\
         activate\n\
         return \"found\"\n\
         end if\n\
         end repeat\n\
         end tell\n\
         return \"missing\""
    );
    run_osascript(&script)
}

#[cfg(target_os = "macos")]
fn run_osascript(script: &str) -> bool {
    Command::new("osascript")
        .args(["-e", script])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("found"))
        .unwrap_or(false)
}

/// Escape before shelling out: values land inside AppleScript string
/// literals — escape `\` before `"`.
#[cfg(target_os = "macos")]
fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(target_os = "macos")]
fn open_bundle(bundle: &str, path: Option<&str>) -> bool {
    let mut cmd = Command::new("open");
    cmd.args(["-b", bundle]);
    if let Some(p) = path {
        cmd.arg(p);
    }
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

/// Windows: raise the nearest still-alive ancestor process that owns a
/// visible top-level window (the shell's console, Windows Terminal, an IDE).
#[cfg(target_os = "windows")]
pub fn focus(info: Option<&TerminalInfo>, _project_path: Option<&str>) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    let Some(ancestors) = info.and_then(|t| t.ancestors.as_ref()) else {
        return false;
    };
    let Some(table) = process_table() else {
        return false;
    };
    for a in ancestors {
        // PID-reuse guard: only trust the pid if its exe name still matches
        // what was captured.
        let alive = table
            .get(&a.pid)
            .is_some_and(|(_, name)| name.eq_ignore_ascii_case(&a.name));
        if !alive {
            continue;
        }
        if let Some(hwnd) = main_window_of(a.pid) {
            unsafe {
                if IsIconic(hwnd) != 0 {
                    ShowWindow(hwnd, SW_RESTORE);
                }
                if SetForegroundWindow(hwnd) != 0 {
                    return true;
                }
            }
        }
    }
    false
}

/// First visible, unowned top-level window belonging to a process.
#[cfg(target_os = "windows")]
fn main_window_of(pid: u32) -> Option<windows_sys::Win32::Foundation::HWND> {
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER,
    };

    struct Search {
        pid: u32,
        hwnd: HWND,
    }
    unsafe extern "system" fn matches(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = &mut *(lparam as *mut Search);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == search.pid && IsWindowVisible(hwnd) != 0 && GetWindow(hwnd, GW_OWNER).is_null() {
            search.hwnd = hwnd;
            return 0; // stop enumerating
        }
        1
    }

    let mut search = Search {
        pid,
        hwnd: std::ptr::null_mut(),
    };
    unsafe {
        let _ = EnumWindows(Some(matches), &mut search as *mut Search as LPARAM);
    }
    (!search.hwnd.is_null()).then_some(search.hwnd)
}

/// Linux: activate a window owned by a still-alive ancestor process via
/// xdotool (X11 only; silently a no-op on Wayland or without xdotool).
#[cfg(target_os = "linux")]
pub fn focus(info: Option<&TerminalInfo>, _project_path: Option<&str>) -> bool {
    let Some(ancestors) = info.and_then(|t| t.ancestors.as_ref()) else {
        return false;
    };
    for a in ancestors {
        // PID-reuse guard, mirroring capture (both use the 15-char comm name).
        let name_matches = std::fs::read_to_string(format!("/proc/{}/comm", a.pid))
            .map(|s| s.trim() == a.name)
            .unwrap_or(false);
        if !name_matches {
            continue;
        }
        let Ok(out) = Command::new("xdotool")
            .args(["search", "--onlyvisible", "--pid", &a.pid.to_string()])
            .output()
        else {
            return false; // xdotool missing — later ancestors won't fare better
        };
        let ids = String::from_utf8_lossy(&out.stdout);
        let Some(wid) = ids.lines().last().map(str::trim).filter(|w| !w.is_empty()) else {
            continue;
        };
        let activated = Command::new("xdotool")
            .args(["windowactivate", wid])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if activated {
            return true;
        }
    }
    false
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn focus(_info: Option<&TerminalInfo>, _project_path: Option<&str>) -> bool {
    false
}
