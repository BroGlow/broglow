//! Which terminal window hosts a session, and how to get the user back to it.
//!
//! The hook backend (`broglow hook`) runs as a descendant of whatever app
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
    let ancestors = capture_ancestors();
    // Windows has no controlling tty: the owner is the nearest ancestor that
    // isn't a wrapper shell, taken from the same ToolHelp walk focus uses.
    #[cfg(target_os = "windows")]
    let (tty, owner_pid) = (None, ancestors.as_deref().and_then(owner_from_ancestors));
    #[cfg(not(target_os = "windows"))]
    let (tty, owner_pid) = capture_tty_owner();
    TerminalInfo {
        term_program: env("TERM_PROGRAM"),
        term_session_id: env("TERM_SESSION_ID"),
        iterm_session_id: env("ITERM_SESSION_ID"),
        bundle_id: env("__CFBundleIdentifier"),
        host_session_id: env("CLAUDE_CODE_HOST_SESSION_ID"),
        wt_session: env("WT_SESSION"),
        kitty_window_id: env("KITTY_WINDOW_ID"),
        wezterm_pane: env("WEZTERM_PANE"),
        // TMUX_PANE can linger in the environment after leaving tmux; only
        // trust it while TMUX itself is set.
        tmux_pane: env("TMUX").and_then(|_| env("TMUX_PANE")),
        tty,
        owner_pid,
        ancestors,
    }
}

/// Whether an exe/comm name is a shell that merely wraps the hook (`sh -c`,
/// `cmd /c`, …). Such a shell dies with the hook, so it can never be the
/// session's owner. Matched by basename, minus the login-shell dash ("-zsh")
/// and a Windows `.exe` suffix, case-insensitively.
fn is_wrapper_shell(name: &str) -> bool {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let base = base.trim_start_matches('-').to_ascii_lowercase();
    let base = base.strip_suffix(".exe").unwrap_or(&base);
    matches!(
        base,
        "sh" | "bash" | "zsh" | "dash" | "ksh" | "fish" | "cmd" | "powershell" | "pwsh"
    )
}

/// Owner pid from a nearest-first ancestor chain: the first process above the
/// wrapper shells is the harness (`claude.exe`, `node.exe`, `codex.exe`, …).
/// The harness outlives the hook, so its pid is what the reap may probe later.
#[cfg(any(target_os = "windows", test))]
fn owner_from_ancestors(chain: &[Ancestor]) -> Option<u32> {
    chain
        .iter()
        .find(|a| !is_wrapper_shell(&a.name))
        .map(|a| a.pid)
}

/// The session's controlling terminal device ("/dev/ttys012") and the PID that
/// owns it (unix; Windows derives the owner from the ancestor chain in
/// [`capture`]). Claude Code spawns hook subprocesses without a controlling tty, so
/// walk up the parent chain (one `ps` snapshot) to the first ancestor that does
/// own a tty — that is the `claude` process itself. Its tty identifies the
/// window for click-to-focus; its PID lets the readers tell later whether the
/// session is still alive (see [`is_alive`]).
///
/// Not every harness detaches its hooks, though: Copilot runs them through a
/// tty-inheriting `sh -c`, so the first tty-having process is the hook itself.
/// The owner must outlive the hook — recording the hook (or its wrapper
/// shell) hands the reap a pid that is dead milliseconds later, and the live
/// session gets downgraded to Done. So the walk takes the tty from the first
/// tty-having process but keeps climbing past ourselves and wrapper shells to
/// the harness process for the owner pid.
#[cfg(unix)]
fn capture_tty_owner() -> (Option<String>, Option<u32>) {
    let out = match Command::new("ps")
        .args(["-axo", "pid=,ppid=,tty=,comm="])
        .output()
    {
        Ok(o) => o,
        Err(_) => return (None, None),
    };
    let table = parse_ps_table(&String::from_utf8_lossy(&out.stdout));
    tty_owner_from(&table, std::process::id())
}

/// Parse `ps -axo pid=,ppid=,tty=,comm=` into pid → (ppid, tty, comm). Only
/// comm's first token is kept — enough to recognize a shell, and shells never
/// have spaces in their names.
#[cfg(unix)]
fn parse_ps_table(stdout: &str) -> std::collections::HashMap<u32, (u32, String, String)> {
    let mut table = std::collections::HashMap::new();
    for line in stdout.lines() {
        let mut cols = line.split_whitespace();
        let (Some(pid), Some(ppid), Some(tty)) = (cols.next(), cols.next(), cols.next()) else {
            continue;
        };
        let (Ok(pid), Ok(ppid)) = (pid.parse::<u32>(), ppid.parse::<u32>()) else {
            continue;
        };
        let comm = cols.next().unwrap_or("").to_string();
        table.insert(pid, (ppid, tty.to_string(), comm));
    }
    table
}

#[cfg(unix)]
fn tty_owner_from(
    table: &std::collections::HashMap<u32, (u32, String, String)>,
    self_pid: u32,
) -> (Option<String>, Option<u32>) {
    let mut tty_dev = None;
    let mut pid = self_pid;
    for _ in 0..16 {
        let Some((ppid, tty, comm)) = table.get(&pid) else {
            return (tty_dev, None);
        };
        // ps prints "??" (macOS) / "?" (Linux) when there is no controlling tty.
        if !tty.is_empty() && !tty.starts_with('?') && tty != "-" {
            if tty_dev.is_none() {
                tty_dev = Some(if tty.starts_with("/dev/") {
                    tty.clone()
                } else {
                    format!("/dev/{tty}")
                });
            }
            if pid != self_pid && !is_wrapper_shell(comm) {
                return (tty_dev, Some(pid));
            }
        }
        if *ppid <= 1 || *ppid == pid {
            return (tty_dev, None);
        }
        pid = *ppid;
    }
    (tty_dev, None)
}

#[cfg(not(any(unix, target_os = "windows")))]
fn capture_tty_owner() -> (Option<String>, Option<u32>) {
    (None, None)
}

/// Whether a process with this PID is still running. Used to reap sessions
/// whose Claude Code process exited without a SessionEnd hook (terminal window
/// closed, crash, SIGKILL), which would otherwise leave the status light stuck.
///
/// Fail-safe toward "alive": on any ambiguity (can't probe, access denied) we
/// report `true`, so a session that is genuinely still running is never cleared
/// out from under the user. The worst case is a dead session lingering a little
/// longer — the same outcome as before this check existed.
#[cfg(unix)]
pub fn is_alive(pid: u32) -> bool {
    // Signal 0 does the kernel's existence/permission checks without delivering
    // a signal: 0 => the process exists; ESRCH => it's gone; EPERM => it exists
    // but is owned by another user.
    if unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Windows liveness via the same process snapshot the focus path already uses:
/// a PID present in the table is live. If the snapshot can't be taken, fail
/// safe toward "alive".
#[cfg(target_os = "windows")]
pub fn is_alive(pid: u32) -> bool {
    process_table()
        .map(|t| t.contains_key(&pid))
        .unwrap_or(true)
}

#[cfg(not(any(unix, target_os = "windows")))]
pub fn is_alive(_pid: u32) -> bool {
    true
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
pub fn focus(
    session_id: &str,
    harness: Option<&str>,
    info: Option<&TerminalInfo>,
    project_path: Option<&str>,
) -> bool {
    if harness == Some("codex")
        && info.is_some_and(|t| t.bundle_id.as_deref() == Some("com.openai.codex"))
    {
        return open_codex_thread(session_id);
    }
    if let Some(t) = info {
        if t.bundle_id.as_deref() == Some(CLAUDE_BUNDLE) {
            if let Some(id) = t.host_session_id.as_deref().filter(|id| is_local_id(id)) {
                return open_bundle(
                    CLAUDE_BUNDLE,
                    Some(&format!("claude://code/continue?session={id}")),
                );
            }
        }
        if focus_macos(t, project_path) {
            return true;
        }
    }
    // Desktop hooks can have no terminal identity at all. Use the task ID
    // before the legacy Terminal lookup, which cannot find a desktop task.
    if harness == Some("codex") {
        return open_codex_thread(session_id);
    }
    // Sessions recorded before terminal capture existed: fall back to
    // scanning Terminal.app window titles for the project directory name.
    project_path.is_some_and(focus_terminal_by_title)
}

#[cfg(target_os = "macos")]
const CLAUDE_BUNDLE: &str = "com.anthropic.claudefordesktop";

/// The desktop app's own session ID shape, which its link handler also
/// requires; anything else must not reach the URL.
#[cfg(target_os = "macos")]
fn is_local_id(id: &str) -> bool {
    id.strip_prefix("local_").is_some_and(|rest| {
        (1..=64).contains(&rest.len())
            && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

#[cfg(target_os = "macos")]
fn open_codex_thread(session_id: &str) -> bool {
    let Ok(id) = uuid::Uuid::parse_str(session_id) else {
        return false;
    };
    // The same bundle ID is used when the desktop app is named ChatGPT.app.
    open_bundle("com.openai.codex", Some(&format!("codex://threads/{id}")))
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
pub fn focus(
    _session_id: &str,
    _harness: Option<&str>,
    info: Option<&TerminalInfo>,
    _project_path: Option<&str>,
) -> bool {
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
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
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
pub fn focus(
    _session_id: &str,
    _harness: Option<&str>,
    info: Option<&TerminalInfo>,
    _project_path: Option<&str>,
) -> bool {
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
pub fn focus(
    _session_id: &str,
    _harness: Option<&str>,
    _info: Option<&TerminalInfo>,
    _project_path: Option<&str>,
) -> bool {
    false
}

/// What kind of app hosts a session, for the popover's host glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
    /// An agent's own desktop app (Claude, Codex).
    App,
    Terminal,
    Ide,
}

impl HostKind {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub fn as_str(self) -> &'static str {
        match self {
            HostKind::App => "app",
            HostKind::Terminal => "terminal",
            HostKind::Ide => "ide",
        }
    }
}

/// Where a session runs: its kind plus a short name ("Claude app", "Ghostty").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Host {
    pub kind: HostKind,
    pub name: String,
}

const DESKTOP_APPS: &[(&str, &str)] = &[
    ("com.anthropic.claudefordesktop", "Claude app"),
    ("com.openai.codex", "Codex app"),
];

const IDE_BUNDLES: &[(&str, &str)] = &[
    ("com.microsoft.VSCode", "VS Code"),
    ("com.microsoft.VSCodeInsiders", "VS Code Insiders"),
    ("com.todesktop.230313mzl4w4u92", "Cursor"),
    ("com.exafunction.windsurf", "Windsurf"),
    ("dev.zed.Zed", "Zed"),
];

/// TERM_PROGRAM values with a friendlier name; others are shown as-is.
const TERM_PROGRAMS: &[(&str, &str)] = &[
    ("Apple_Terminal", "Terminal"),
    ("iTerm.app", "iTerm2"),
    ("ghostty", "Ghostty"),
    ("WarpTerminal", "Warp"),
];

/// Classify a session's captured identity. A desktop app's bundle wins over
/// stale inherited terminal variables, as in [`focus`]. Codex desktop hooks
/// carry no terminal identity at all, so an identity-less Codex session is its
/// desktop app — the same rule focusing uses. `None` when nothing is known.
pub fn host(harness: Option<&str>, info: Option<&TerminalInfo>) -> Option<Host> {
    let info = info?;
    let found = |kind, name: &str| {
        Some(Host {
            kind,
            name: name.to_string(),
        })
    };
    let lookup = |table: &[(&str, &'static str)], key: &str| {
        table.iter().find(|(k, _)| *k == key).map(|(_, name)| *name)
    };
    let bundle = info.bundle_id.as_deref().unwrap_or_default();

    if let Some(name) = lookup(DESKTOP_APPS, bundle) {
        return found(HostKind::App, name);
    }
    if let Some(name) = lookup(IDE_BUNDLES, bundle) {
        return found(HostKind::Ide, name);
    }
    if bundle.starts_with("com.jetbrains.") {
        return found(HostKind::Ide, "JetBrains");
    }
    if let Some(program) = info.term_program.as_deref() {
        if program == "vscode" {
            return found(HostKind::Ide, "VS Code");
        }
        return found(
            HostKind::Terminal,
            lookup(TERM_PROGRAMS, program).unwrap_or(program),
        );
    }
    let terminal = if info.kitty_window_id.is_some() {
        Some("kitty")
    } else if info.wezterm_pane.is_some() {
        Some("WezTerm")
    } else if info.wt_session.is_some() {
        Some("Windows Terminal")
    } else if info.tty.is_some() {
        Some("terminal")
    } else {
        None
    };
    match terminal {
        Some(name) => found(HostKind::Terminal, name),
        None if harness == Some("codex") => found(HostKind::App, "Codex app"),
        None => None,
    }
}

#[cfg(test)]
mod owner_tests {
    use super::*;

    fn chain(names: &[&str]) -> Vec<Ancestor> {
        names
            .iter()
            .enumerate()
            .map(|(i, n)| Ancestor {
                pid: 100 + i as u32,
                name: n.to_string(),
            })
            .collect()
    }

    #[test]
    fn owner_is_the_first_ancestor_above_the_wrapper_shells() {
        // Claude on Windows: hook ← cmd /c ← bash -c ← node.exe ← Windows Terminal.
        assert_eq!(
            owner_from_ancestors(&chain(&[
                "cmd.exe",
                "bash.exe",
                "node.exe",
                "WindowsTerminal.exe"
            ])),
            Some(102)
        );
        // A harness that spawns the hook directly is the owner itself.
        assert_eq!(
            owner_from_ancestors(&chain(&["claude.exe", "pwsh.exe"])),
            Some(100)
        );
        assert_eq!(
            owner_from_ancestors(&chain(&[
                "C:\\Program Files\\PowerShell\\7\\pwsh.EXE",
                "Codex.exe"
            ])),
            Some(101)
        );
        // Nothing but shells: no owner, so the staleness backstop applies.
        assert_eq!(owner_from_ancestors(&chain(&["sh", "-zsh"])), None);
        assert_eq!(owner_from_ancestors(&[]), None);
    }
}

#[cfg(test)]
mod host_tests {
    use super::*;

    fn info(change: impl FnOnce(&mut TerminalInfo)) -> TerminalInfo {
        let mut info = TerminalInfo::default();
        change(&mut info);
        info
    }

    fn name(harness: Option<&str>, info: &TerminalInfo) -> Option<(HostKind, String)> {
        host(harness, Some(info)).map(|h| (h.kind, h.name))
    }

    #[test]
    fn desktop_apps_win_over_stale_terminal_variables() {
        let claude = info(|i| {
            i.bundle_id = Some("com.anthropic.claudefordesktop".into());
            i.term_program = Some("Apple_Terminal".into());
        });
        assert_eq!(
            name(None, &claude),
            Some((HostKind::App, "Claude app".into()))
        );
        // Codex desktop hooks carry no identity; Claude's don't get that guess.
        let empty = TerminalInfo::default();
        assert_eq!(
            name(Some("codex"), &empty),
            Some((HostKind::App, "Codex app".into()))
        );
        assert_eq!(name(None, &empty), None);
        assert_eq!(host(Some("codex"), None), None, "Nothing captured yet");
    }

    #[test]
    fn ides_and_terminals_get_short_names() {
        let cases = [
            (
                info(|i| i.bundle_id = Some("com.microsoft.VSCode".into())),
                HostKind::Ide,
                "VS Code",
            ),
            (
                info(|i| i.bundle_id = Some("com.jetbrains.intellij".into())),
                HostKind::Ide,
                "JetBrains",
            ),
            (
                info(|i| i.term_program = Some("vscode".into())),
                HostKind::Ide,
                "VS Code",
            ),
            (
                info(|i| i.term_program = Some("ghostty".into())),
                HostKind::Terminal,
                "Ghostty",
            ),
            (
                info(|i| i.term_program = Some("Apple_Terminal".into())),
                HostKind::Terminal,
                "Terminal",
            ),
            (
                info(|i| i.term_program = Some("Hyper".into())),
                HostKind::Terminal,
                "Hyper",
            ),
            (
                info(|i| i.kitty_window_id = Some("1".into())),
                HostKind::Terminal,
                "kitty",
            ),
            (
                info(|i| i.wt_session = Some("guid".into())),
                HostKind::Terminal,
                "Windows Terminal",
            ),
            (
                info(|i| i.tty = Some("/dev/ttys001".into())),
                HostKind::Terminal,
                "terminal",
            ),
        ];
        for (terminal, kind, expected) in cases {
            assert_eq!(
                name(Some("codex"), &terminal),
                Some((kind, expected.into()))
            );
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn codex_desktop_focus_uses_thread_link() {
        use std::os::unix::fs::PermissionsExt;

        // A child test process isolates PATH from the parallel test suite;
        // fake launchers record requests without opening any real windows.
        if let Ok(dir) = std::env::var("BROGLOW_FOCUS_TEST_DIR") {
            let log = std::path::Path::new(&dir).join("calls");
            let id = "00000000-0000-4000-8000-000000000001";
            let empty = TerminalInfo::default();
            let desktop = TerminalInfo {
                bundle_id: Some("com.openai.codex".into()),
                // Stale inherited terminal variables must not override an
                // explicit desktop host.
                term_program: Some("Apple_Terminal".into()),
                tty: Some("/dev/ttys007".into()),
                ..Default::default()
            };
            for info in [None, Some(&empty), Some(&desktop)] {
                std::fs::write(&log, "").unwrap();
                assert!(focus(id, Some("codex"), info, Some("/tmp/project")));
                assert_eq!(
                    std::fs::read_to_string(&log).unwrap(),
                    format!("open\n-b\ncom.openai.codex\ncodex://threads/{id}\n")
                );
            }

            let cli = TerminalInfo {
                bundle_id: Some("com.apple.Terminal".into()),
                ..desktop
            };
            std::fs::write(&log, "").unwrap();
            assert!(focus(id, Some("codex"), Some(&cli), Some("/tmp/project")));
            let calls = std::fs::read_to_string(&log).unwrap();
            assert!(calls.starts_with("osascript\n"));
            assert!(calls.contains("/dev/ttys007"));
            assert!(!calls.contains("open\n"));

            std::fs::write(&log, "").unwrap();
            assert!(!focus(id, None, Some(&empty), Some("/tmp/project")));
            assert!(!std::fs::read_to_string(&log).unwrap().contains("codex://"));

            std::fs::write(&log, "").unwrap();
            assert!(!focus("new?prompt=injected", Some("codex"), None, None));
            assert!(std::fs::read_to_string(&log).unwrap().is_empty());

            // Claude desktop: select the session itself, not just the app.
            let local = "local_00000000-0000-4000-8000-000000000002";
            let mut claude = TerminalInfo {
                bundle_id: Some(CLAUDE_BUNDLE.into()),
                host_session_id: Some(local.into()),
                ..Default::default()
            };
            std::fs::write(&log, "").unwrap();
            assert!(focus(id, None, Some(&claude), None));
            assert_eq!(
                std::fs::read_to_string(&log).unwrap(),
                format!("open\n-b\n{CLAUDE_BUNDLE}\nclaude://code/continue?session={local}\n")
            );
            claude.host_session_id = Some("local_x&prompt=injected".into());
            std::fs::write(&log, "").unwrap();
            assert!(focus(id, None, Some(&claude), None));
            assert_eq!(
                std::fs::read_to_string(&log).unwrap(),
                format!("open\n-b\n{CLAUDE_BUNDLE}\n")
            );

            std::fs::write(std::path::Path::new(&dir).join("fail-open"), "").unwrap();
            assert!(!focus(id, Some("codex"), None, Some("/tmp/project")));
            assert!(!std::fs::read_to_string(&log).unwrap().contains("osascript"));
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        for (name, result) in [
            ("open", "[ ! -e \"$BROGLOW_FOCUS_TEST_DIR/fail-open\" ]"),
            (
                "osascript",
                "case \"$*\" in *'/dev/ttys007'*) echo found ;; *) echo missing ;; esac",
            ),
        ] {
            let path = dir.path().join(name);
            std::fs::write(
                &path,
                format!(
                    "#!/bin/sh\nprintf '%s\\n' '{name}' \"$@\" >> \"$BROGLOW_FOCUS_TEST_DIR/calls\"\n{result}\n"
                ),
            )
            .unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "terminal::tests::codex_desktop_focus_uses_thread_link",
                "--nocapture",
            ])
            .env("PATH", dir.path())
            .env("BROGLOW_FOCUS_TEST_DIR", dir.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn table(
        rows: &[(u32, u32, &str, &str)],
    ) -> std::collections::HashMap<u32, (u32, String, String)> {
        rows.iter()
            .map(|&(pid, ppid, tty, comm)| (pid, (ppid, tty.to_string(), comm.to_string())))
            .collect()
    }

    #[test]
    fn detached_hook_owner_is_the_harness() {
        // Claude Code detaches its hooks from the tty; the first tty-having
        // ancestor is the claude process itself.
        let t = table(&[
            (500, 400, "??", "broglow"),
            (400, 300, "ttys009", "claude"),
            (300, 200, "ttys009", "-zsh"),
            (200, 1, "??", "Terminal"),
        ]);
        assert_eq!(
            tty_owner_from(&t, 500),
            (Some("/dev/ttys009".into()), Some(400))
        );
    }

    #[test]
    fn tty_inheriting_hook_skips_itself_and_the_wrapper_shell() {
        // Copilot runs hooks through a tty-inheriting `sh -c`: recording the
        // hook or the sh as owner would reap the live session within seconds.
        let t = table(&[
            (500, 450, "ttys007", "broglow"),
            (450, 400, "ttys007", "sh"),
            (400, 300, "ttys007", "copilot"),
            (300, 200, "ttys007", "/bin/zsh"),
            (200, 1, "??", "Terminal"),
        ]);
        assert_eq!(
            tty_owner_from(&t, 500),
            (Some("/dev/ttys007".into()), Some(400))
        );
    }

    #[test]
    fn shell_only_chain_keeps_the_tty_but_names_no_owner() {
        // A hand-run hook straight under an interactive shell: better no owner
        // (24h staleness backstop) than a pid whose death is meaningless.
        let t = table(&[
            (500, 300, "ttys002", "broglow"),
            (300, 1, "ttys002", "-zsh"),
        ]);
        assert_eq!(tty_owner_from(&t, 500), (Some("/dev/ttys002".into()), None));
    }

    #[test]
    fn no_tty_anywhere_captures_nothing() {
        let t = table(&[(500, 400, "??", "broglow"), (400, 1, "??", "launchd")]);
        assert_eq!(tty_owner_from(&t, 500), (None, None));
    }
}
