use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::Context;
use notify::{EventKind, RecursiveMode, Watcher};
use tao::event::Event;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use tao::event::WindowEvent;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};

use crate::config;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::popover::{Popover, PopoverMsg};
#[cfg(any(test, not(any(target_os = "macos", target_os = "windows"))))]
use crate::session::DisplaySession;
#[cfg(any(test, not(any(target_os = "macos", target_os = "windows"))))]
use crate::state::Status;
use crate::state::{self, aggregate, read_display_state, Aggregate, HookState};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::usage;

pub(crate) const ICON_GREEN: &[u8] = include_bytes!("../assets/icons/broglow-green.png");
pub(crate) const ICON_YELLOW: &[u8] = include_bytes!("../assets/icons/broglow-yellow.png");
pub(crate) const ICON_RED: &[u8] = include_bytes!("../assets/icons/broglow-red.png");
pub(crate) const ICON_NONE: &[u8] = include_bytes!("../assets/icons/broglow-none.png");

/// Tray icon for the current hook state, resolving inactive-vs-active through
/// the user's yellow-mode setting.
fn icon_for_state(state: &HookState) -> anyhow::Result<Icon> {
    icon_for(aggregate(state, config::read_config().yellow_mode))
}

fn icon_for(agg: Aggregate) -> anyhow::Result<Icon> {
    let bytes = match agg {
        Aggregate::Red => ICON_RED,
        Aggregate::Yellow => ICON_YELLOW,
        Aggregate::Green => ICON_GREEN,
        Aggregate::None => ICON_NONE,
    };
    let img = image::load_from_memory(bytes).context("Decoding embedded icon PNG")?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), w, h).context("Building tray icon")
}

// ---------------------------------------------------------------------------
// Daemon liveness / start — used by the TUI's `t` key and tray status chip
// ---------------------------------------------------------------------------

/// Whether a tray daemon is already running (launchd-managed or foreground).
/// Matched on the command line (`… broglow menubar`) so the TUI process
/// itself never counts.
#[cfg(unix)]
pub fn is_running() -> bool {
    let Ok(out) = std::process::Command::new("ps")
        .args(["-axo", "command="])
        .output()
    else {
        return false;
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .any(is_menubar_cmdline)
}

/// `ps` prints the binary path unquoted, so a home directory with a space
/// (`/Users/Jo Doe/Applications/BroGlow.app/.../broglow menubar`) splits
/// across tokens; match the `broglow` token immediately before `menubar`
/// instead of assuming the path is the first token.
#[cfg(unix)]
fn is_menubar_cmdline(line: &str) -> bool {
    let parts: Vec<&str> = line.split_whitespace().collect();
    parts
        .windows(2)
        .any(|w| (w[0] == "broglow" || w[0].ends_with("/broglow")) && w[1] == "menubar")
}

#[cfg(all(test, unix))]
mod cmdline_tests {
    use super::is_menubar_cmdline;

    #[test]
    fn menubar_daemon_is_recognized_even_with_spaces_in_its_path() {
        assert!(is_menubar_cmdline("broglow menubar"));
        assert!(is_menubar_cmdline("/usr/local/bin/broglow menubar"));
        assert!(is_menubar_cmdline(
            "/Users/Jo Doe/Applications/BroGlow.app/Contents/MacOS/broglow menubar"
        ));
        assert!(!is_menubar_cmdline("broglow"));
        assert!(!is_menubar_cmdline("/usr/local/bin/broglow hook"));
        assert!(!is_menubar_cmdline("vim notes/broglow menubar.md"));
        assert!(!is_menubar_cmdline("my-broglow menubar"));
    }
}

/// Windows: the daemon holds a named mutex for its lifetime (see
/// `acquire_single_instance_lock`); probing it is the liveness check.
#[cfg(windows)]
pub fn is_running() -> bool {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::OpenMutexW;
    const SYNCHRONIZE: u32 = 0x0010_0000;

    let name: Vec<u16> = "Local\\broglow-menubar"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        let handle = OpenMutexW(SYNCHRONIZE, 0, name.as_ptr());
        if handle.is_null() {
            false
        } else {
            CloseHandle(handle);
            true
        }
    }
}

/// Start the tray daemon if it isn't running; returns a status-bar message.
/// macOS prefers launchd (restarts the installed LaunchAgent, so there is
/// never a second instance); elsewhere — or when no agent is installed — the
/// daemon is spawned detached, guarded by [`is_running`] (and by the named
/// mutex on Windows).
pub fn ensure_running() -> String {
    if is_running() {
        return "Tray daemon is already running.".to_string();
    }

    #[cfg(target_os = "macos")]
    {
        if crate::launch_agent_path().exists() {
            if let Ok(uid) = crate::current_uid() {
                let service = format!("gui/{uid}/{}", crate::LAUNCH_AGENT_LABEL);
                let started = std::process::Command::new("launchctl")
                    .args(["kickstart", &service])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);
                // Not loaded (e.g. booted out): load the agent, which starts
                // it via RunAtLoad.
                if !started {
                    let _ = std::process::Command::new("launchctl")
                        .args(["bootstrap", &format!("gui/{uid}")])
                        .arg(crate::launch_agent_path())
                        .output();
                    let _ = std::process::Command::new("launchctl")
                        .args(["kickstart", &service])
                        .output();
                }
                return "Started the menu bar daemon.".to_string();
            }
        }
    }

    // No launchd agent (never installed) or non-macOS: spawn directly.
    let Ok(exe) = std::env::current_exe() else {
        return "Couldn't locate the broglow binary.".to_string();
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("menubar")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::spawn::configure_detached(&mut cmd);
    match cmd.spawn() {
        Ok(_) => "Started the tray daemon (run `broglow install` for login autostart).".to_string(),
        Err(e) => format!("Couldn't start the tray daemon: {e}"),
    }
}

/// Session display name: the auto-namer's title when present, else a
/// truncated session id. Char-based truncation — ids are ours, but keep the
/// repo-wide rule of never byte-slicing. The popover's; the Linux menu takes
/// its names from `session::load_all_sessions`.
#[cfg(any(test, target_os = "macos", target_os = "windows"))]
pub(crate) fn display_name(id: &str, s: &state::SessionStatus) -> String {
    s.name.clone().unwrap_or_else(|| {
        let prefix: String = id.chars().take(8).collect();
        format!("Session {prefix}")
    })
}

/// One row of the Linux tray menu's session list.
#[cfg(any(test, not(any(target_os = "macos", target_os = "windows"))))]
enum MenuRow {
    /// A disabled project heading (grouped by project only).
    Heading(String),
    /// A clickable row and the session it focuses.
    Session { text: String, id: String },
}

/// The Linux menu's session rows from the shared order and labels
/// (`session::load_all_sessions`), live sessions only. Grouped by project, a
/// `📁 label` heading opens each project and the rows beneath are indented
/// and drop the project from their text.
#[cfg(any(test, not(any(target_os = "macos", target_os = "windows"))))]
fn menu_rows(
    sessions: &[DisplaySession],
    group_by_project: bool,
    show_names: bool,
) -> Vec<MenuRow> {
    let mut rows = Vec::new();
    let mut current: Option<&str> = None;
    for s in sessions.iter().filter(|s| s.status != Status::Done) {
        let (prefix, badge) = match s.status {
            Status::NeedsHelp => ("🔴", "needs help"),
            Status::Active => ("🟢", "working"),
            Status::Inactive => ("🟠", "paused"),
            Status::Done => ("⚪", "done"),
        };
        let agent = crate::harness::agent(s.harness.as_deref()).text_prefix(show_names);
        let text = if group_by_project {
            if current != Some(s.project_path.as_str()) {
                current = Some(&s.project_path);
                rows.push(MenuRow::Heading(format!("📁 {}", s.project_name)));
            }
            let agent = agent.trim_end_matches([' ', '·']);
            format!("  {prefix} {} ({badge}) — {agent}", s.name)
        } else {
            format!("{prefix} {} ({badge}) — {agent}{}", s.name, s.project_name)
        };
        rows.push(MenuRow::Session {
            text,
            id: s.session_id.clone(),
        });
    }
    rows
}

struct MenuIds {
    open_broglow: MenuId,
    quit: MenuId,
    /// Menu id → session id for the per-session rows (Linux native menu
    /// only); clicking one focuses that session's terminal window.
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    sessions: Vec<(MenuId, String)>,
}

/// Full native tray menu — Linux only. macOS and Windows render sessions in
/// the custom popover instead and keep just a minimal right-click menu.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn build_menu(state: &HookState) -> anyhow::Result<(Menu, MenuIds)> {
    let menu = Menu::new();

    let mut needs_help = 0u32;
    let mut active = 0u32;
    let mut inactive = 0u32;
    for s in state.sessions.values() {
        match s.status {
            Status::NeedsHelp => needs_help += 1,
            Status::Active => active += 1,
            Status::Inactive => inactive += 1,
            Status::Done => {}
        }
    }

    let header = MenuItem::new("Sessions", false, None);
    menu.append(&header)?;
    menu.append(&MenuItem::new(format!("  Active: {active}"), false, None))?;
    menu.append(&MenuItem::new(
        format!("  Inactive: {inactive}"),
        false,
        None,
    ))?;
    menu.append(&MenuItem::new(
        format!("  Needs Help: {needs_help}"),
        false,
        None,
    ))?;
    menu.append(&PredefinedMenuItem::separator())?;

    let cfg = config::read_config();
    let mut sessions = crate::session::load_all_sessions(state, cfg.group_by_project);
    // The menu shows live sessions only, so label over those: a done session
    // the TUI would still list must not lengthen a live label here.
    sessions.retain(|s| s.status != Status::Done);
    crate::session::relabel(&mut sessions);
    let rows = menu_rows(&sessions, cfg.group_by_project, cfg.show_agent_names);

    let mut session_ids = Vec::new();
    if rows.is_empty() {
        menu.append(&MenuItem::new("No live sessions", false, None))?;
    }
    for row in rows {
        match row {
            MenuRow::Heading(text) => menu.append(&MenuItem::new(text, false, None))?,
            MenuRow::Session { text, id } => {
                // Clickable: focus the session's terminal window (best-effort).
                let item = MenuItem::new(text, true, None);
                menu.append(&item)?;
                session_ids.push((item.id().clone(), id));
            }
        }
    }

    menu.append(&PredefinedMenuItem::separator())?;
    let open_broglow = MenuItem::new("Open BroGlow", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let ids = MenuIds {
        open_broglow: open_broglow.id().clone(),
        quit: quit.id().clone(),
        sessions: session_ids,
    };
    if cfg.show_dashboard_button {
        menu.append(&open_broglow)?;
    }
    menu.append(&quit)?;

    Ok((menu, ids))
}

/// Minimal right-click menu for macOS/Windows: the sessions live in the
/// popover (left click); this stays as a safety hatch so Quit is always
/// reachable even if the webview fails.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn build_fallback_menu(show_dashboard: bool) -> anyhow::Result<(Menu, MenuIds)> {
    let menu = Menu::new();
    let open_broglow = MenuItem::new("Open Dashboard", true, None);
    let quit = MenuItem::new("Quit BroGlow", true, None);
    let ids = MenuIds {
        open_broglow: open_broglow.id().clone(),
        quit: quit.id().clone(),
    };
    if show_dashboard {
        menu.append(&open_broglow)?;
        menu.append(&PredefinedMenuItem::separator())?;
    }
    menu.append(&quit)?;
    Ok((menu, ids))
}

enum UserEvent {
    StateChanged,
    /// Periodic re-evaluation. Session state can go stale with no file write to
    /// watch for — a session's Claude Code process can exit without a
    /// SessionEnd hook (window closed, crash, SIGKILL), so `read_display_state`
    /// reaps it only when something asks. This tick makes the tray icon (and an
    /// open popover) recompute on a timer so a stuck-red light clears itself.
    Tick,
    Menu(MenuEvent),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    Tray(TrayIconEvent),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    Popover(PopoverMsg),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    LampSetupFinished(Option<String>),
    /// The usage refresher produced a new snapshot (design 1a/1c readout).
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    UsageChanged,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn start_lamp_setup(
    popover: &mut Popover,
    proxy: &tao::event_loop::EventLoopProxy<UserEvent>,
    automatic: bool,
) {
    if popover.setup_busy {
        return;
    }
    popover.setup_busy = true;
    popover.push_state(&read_display_state());
    let proxy = proxy.clone();
    thread::spawn(move || {
        let error = crate::ble::pair_from_app(automatic)
            .err()
            .map(|e| e.to_string());
        let _ = proxy.send_event(UserEvent::LampSetupFinished(error));
    });
}

/// Short usage readout for the tray: the plan's 5h-block percentage, or
/// today's token count when plan data is unavailable. `None` until the first
/// scan lands (or nothing ran today), which keeps the bar clean instead of
/// showing a zero.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn usage_readout() -> Option<String> {
    let u = usage::latest()?;
    u.five_hour_pct.map(|p| format!("{p:.0}%")).or_else(|| {
        (u.today_tokens > 0).then(|| format!("{} tok", usage::compact_tokens(u.today_tokens)))
    })
}

/// Put the readout where the platform can show it: next to the menu bar icon
/// on macOS (design 1a), in the tray tooltip on Windows (design 1c). Shows
/// nothing while usage tracking is off (the opt-in default).
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn apply_readout(tray: &tray_icon::TrayIcon) {
    let cfg = config::read_config();
    let text = cfg.usage_enabled.then(usage_readout).flatten();
    #[cfg(target_os = "macos")]
    tray.set_title(text.as_deref());
    #[cfg(target_os = "windows")]
    {
        let tooltip = match text {
            Some(t) => format!("BroGlow — {t}"),
            None => "BroGlow".to_string(),
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

pub fn run(_show_on_launch: bool) -> anyhow::Result<()> {
    // Guard against running a second tray daemon (e.g. re-running
    // `broglow install`, or a user manually launching `broglow menubar`
    // while the autostart instance is already up). Two daemons means two
    // tray icons and two LED/net background threads fighting over the same
    // serial port.
    #[cfg(target_os = "windows")]
    if !acquire_single_instance_lock() {
        println!("broglow tray is already running — not starting a second instance.");
        return Ok(());
    }

    // On Windows this binary is a console app (it also hosts the TUI), so the
    // tray daemon would otherwise leave an empty console window open. Hide it.
    #[cfg(target_os = "windows")]
    hide_console_window();

    // Fail before writing hooks if this session cannot run a graphical tray.
    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // One tray per machine: `install` and the TUI's `t` spawn the daemon
    // without checking first, so a second launch must exit. Taken only once
    // a graphical session exists, so a headless launch touches no files.
    // Windows uses the named mutex above.
    #[cfg(unix)]
    let _instance_lock = acquire_instance_lock()?;

    // If the daemon is up but the hooks were never registered (e.g. launched
    // manually before `broglow install`), wire them in so sessions report
    // status. Hooks only — never autostart from here, or we'd spawn a duplicate
    // tray (see first_run_setup_daemon).
    crate::first_run_setup_daemon();

    // macOS: keep the daemon out of the Dock / app switcher (menu-bar only).
    #[cfg(target_os = "macos")]
    event_loop.set_activation_policy(ActivationPolicy::Accessory);

    let proxy_for_menu = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let _ = proxy_for_menu.send_event(UserEvent::Menu(event));
    }));

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let proxy_for_tray = event_loop.create_proxy();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            let _ = proxy_for_tray.send_event(UserEvent::Tray(event));
        }));
    }

    // Drive enabled lamps in the background; USB setup separately pauses this
    // worker and acquires its lease before identifying a newly attached board.
    thread::spawn(|| crate::led::run_daemon());

    let initial_state = read_display_state();

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let mut dashboard_visible = config::read_config().show_dashboard_button;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let (menu, mut ids) = build_fallback_menu(dashboard_visible)?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let (menu, mut ids) = build_menu(&initial_state)?;

    #[allow(unused_mut)]
    let mut tray_builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon_for_state(&initial_state)?)
        .with_tooltip("BroGlow");
    // Left click opens the popover; the native menu stays on right click.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        tray_builder = tray_builder.with_menu_on_left_click(false);
    }
    let tray = tray_builder.build().context("Building tray icon")?;

    // Hooks name a Claude session only on its next turn; title idle ones now.
    // The state watcher re-renders once the names land.
    std::thread::spawn(|| {
        let _ = crate::hook::backfill_claude_titles();
    });

    // Usage readout (design 1a/1c): a background thread scans the transcript
    // JSONLs / plan endpoint and wakes the loop whenever a snapshot lands.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        let proxy = event_loop.create_proxy();
        usage::spawn_refresher(move || {
            let _ = proxy.send_event(UserEvent::UsageChanged);
        });
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let mut popover = {
        let proxy = event_loop.create_proxy();
        Popover::new(&event_loop, move |msg| {
            let _ = proxy.send_event(UserEvent::Popover(msg));
        })?
    };

    // Dev aid: open the popover immediately at a synthetic anchor so it can
    // be inspected/screenshotted without clicking the real tray icon. The
    // anchor mimics where the real icon lives — top of the screen on macOS
    // (menu bar), bottom on Windows (taskbar tray). The card opens on the
    // opposite side of the anchor, so a top-of-screen anchor on Windows
    // would push it off-screen above the monitor, where per-monitor DPI
    // assignment is ambiguous and flip-flops.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if std::env::var_os("BROGLOW_POPOVER_DEBUG").is_some() {
        #[cfg(target_os = "macos")]
        let anchor_y = 0.0;
        #[cfg(target_os = "windows")]
        let anchor_y = {
            #[link(name = "user32")]
            extern "system" {
                fn GetSystemMetrics(index: i32) -> i32;
            }
            // SM_CYSCREEN: primary monitor height — anchor at its bottom edge
            // like a real tray icon.
            f64::from(unsafe { GetSystemMetrics(1) }) - 48.0
        };
        popover.open_at(
            tray_icon::Rect {
                position: tao::dpi::PhysicalPosition::new(1200.0, anchor_y),
                size: tao::dpi::PhysicalSize::new(48, 48),
            },
            &initial_state,
        );
    }

    let proxy_for_watcher = event_loop.create_proxy();
    thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let mut watcher =
            match notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                let _ = tx.send(res);
            }) {
                Ok(w) => w,
                Err(_) => return,
            };

        if let Some(dir) = state::state_file_path().parent() {
            let _ = std::fs::create_dir_all(dir);
            let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
        }

        while let Ok(res) = rx.recv() {
            if let Ok(ev) = res {
                if matches!(
                    ev.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                ) {
                    let _ = proxy_for_watcher.send_event(UserEvent::StateChanged);
                }
            }
        }
    });

    // Periodic tick so the tray recomputes even when no file event fires — the
    // case that leaves a killed session's light stuck (its process exited
    // without a SessionEnd hook, so nothing writes state.json again). Matches
    // the TUI's 5s refresh cadence.
    let proxy_for_tick = event_loop.create_proxy();
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(5));
        if proxy_for_tick.send_event(UserEvent::Tick).is_err() {
            break; // event loop gone; stop ticking.
        }
    });

    let mut tray_holder = Some(tray);
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let setup_proxy = event_loop.create_proxy();
    #[cfg(target_os = "macos")]
    let mut usb_auto_pair = crate::ble::UsbAutoPair::default();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        // Finder/Spotlight should reveal the controls, including an already-running app.
        #[cfg(target_os = "macos")]
        if matches!(event, Event::Reopen { .. })
            || (_show_on_launch && matches!(event, Event::NewEvents(tao::event::StartCause::Init)))
        {
            if let Some(rect) = tray_holder.as_ref().and_then(|tray| tray.rect()) {
                popover.open_at(rect, &read_display_state());
            }
        }

        #[cfg(any(target_os = "macos", target_os = "windows"))]
        if matches!(
            event,
            Event::UserEvent(UserEvent::StateChanged | UserEvent::Tick)
        ) {
            let visible = config::read_config().show_dashboard_button;
            if visible != dashboard_visible {
                if let Ok((menu, new_ids)) = build_fallback_menu(visible) {
                    if let Some(tray) = tray_holder.as_ref() {
                        tray.set_menu(Some(Box::new(menu)));
                    }
                    ids = new_ids;
                    dashboard_visible = visible;
                }
            }
        }

        match event {
            Event::UserEvent(UserEvent::StateChanged) => {
                let state = read_display_state();
                if let Some(tray) = tray_holder.as_ref() {
                    if let Ok(icon) = icon_for_state(&state) {
                        let _ = tray.set_icon(Some(icon));
                    }
                }
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                popover.push_state(&state);
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                if let Ok((menu, new_ids)) = build_menu(&state) {
                    if let Some(tray) = tray_holder.as_ref() {
                        tray.set_menu(Some(Box::new(menu)));
                    }
                    ids = new_ids;
                }
            }
            Event::UserEvent(UserEvent::Tick) => {
                // Five-second polling can miss a faster unplug/replug; switch
                // to USB hotplug events if that ever matters.
                #[cfg(target_os = "macos")]
                if !popover.setup_busy && usb_auto_pair.poll() {
                    start_lamp_setup(&mut popover, &setup_proxy, true);
                }
                let state = read_display_state();
                if let Some(tray) = tray_holder.as_ref() {
                    if let Ok(icon) = icon_for_state(&state) {
                        let _ = tray.set_icon(Some(icon));
                    }
                }
                // Only push to the popover when it's actually on screen —
                // re-rendering a hidden webview every 5s would be wasted work.
                #[cfg(any(target_os = "macos", target_os = "windows"))]
                if popover.is_visible() {
                    popover.push_state(&state);
                }
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                if let Ok((menu, new_ids)) = build_menu(&state) {
                    if let Some(tray) = tray_holder.as_ref() {
                        tray.set_menu(Some(Box::new(menu)));
                    }
                    ids = new_ids;
                }
            }
            Event::UserEvent(UserEvent::Menu(ev)) => {
                if ev.id == ids.open_broglow {
                    open_dashboard();
                } else if ev.id == ids.quit {
                    tray_holder.take();
                    std::process::exit(0);
                } else {
                    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                    if let Some((_, sid)) = ids.sessions.iter().find(|(mid, _)| *mid == ev.id) {
                        let session = read_display_state().sessions.get(sid).cloned();
                        let sid = sid.clone();
                        thread::spawn(move || {
                            if let Some(s) = session {
                                let _ = crate::terminal::focus(
                                    &sid,
                                    s.harness.as_deref(),
                                    s.terminal.as_ref(),
                                    s.project_path.as_deref(),
                                );
                            }
                        });
                    }
                }
            }
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            Event::UserEvent(UserEvent::UsageChanged) => {
                if let Some(tray) = tray_holder.as_ref() {
                    apply_readout(tray);
                }
                // The popover payload embeds the snapshot; refresh it so an
                // open popover's usage section tracks the readout.
                popover.push_state(&read_display_state());
            }
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            Event::UserEvent(UserEvent::Tray(TrayIconEvent::Click {
                rect,
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            })) => {
                if popover.is_visible() {
                    popover.hide();
                } else if !popover.just_dismissed() {
                    popover.open_at(rect, &read_display_state());
                }
            }
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            Event::UserEvent(UserEvent::LampSetupFinished(error)) => {
                popover.lamp_setup_result(error);
            }
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            Event::UserEvent(UserEvent::Popover(msg)) => match msg {
                PopoverMsg::AddLamp => {
                    start_lamp_setup(&mut popover, &setup_proxy, false);
                }
                PopoverMsg::SetLed { .. }
                | PopoverMsg::ForgetLamp { .. }
                | PopoverMsg::RenameLamp { .. }
                | PopoverMsg::IdentifyLamp { .. }
                | PopoverMsg::SetBrightness { .. }
                | PopoverMsg::SetLampBrightness { .. }
                | PopoverMsg::PreviewBrightness { .. }
                    if popover.setup_busy => {}
                PopoverMsg::Ready => popover.push_state(&read_display_state()),
                PopoverMsg::Resize { height, dpr } => popover.on_resize(height, dpr),
                PopoverMsg::Focus { id } => {
                    popover.hide();
                    let state = read_display_state();
                    focus_session(&id, state.sessions.get(&id));
                }
                PopoverMsg::ClearSession { .. } | PopoverMsg::ClearPaused => {
                    let result = if let PopoverMsg::ClearSession { id } = msg {
                        state::clear_session(&id)
                    } else {
                        state::clear_paused_sessions()
                    };
                    let error = result
                        .err()
                        .map(|error| format!("Could not dismiss sessions: {error}"));
                    let state = read_display_state();
                    if let Some(tray) = tray_holder.as_ref() {
                        if let Ok(icon) = icon_for_state(&state) {
                            let _ = tray.set_icon(Some(icon));
                        }
                    }
                    popover.push_state(&state);
                    popover.config_action_result(error);
                }
                PopoverMsg::Dashboard => {
                    popover.hide();
                    open_dashboard();
                }
                PopoverMsg::Quit => {
                    tray_holder.take();
                    std::process::exit(0);
                }
                PopoverMsg::Hide => popover.hide(),
                PopoverMsg::SetYellowMode { mode } => {
                    let error = config::update_config(|cfg| {
                        cfg.yellow_mode = mode;
                        Ok(())
                    })
                    .err()
                    .map(|error| format!("Could not save yellow mode: {error}"));
                    // The config write also lands in the watched directory and
                    // triggers StateChanged, but update directly so the icon
                    // and popover can't lag behind the click.
                    let state = read_display_state();
                    if let Some(tray) = tray_holder.as_ref() {
                        if let Ok(icon) = icon_for_state(&state) {
                            let _ = tray.set_icon(Some(icon));
                        }
                    }
                    popover.push_state(&state);
                    popover.config_action_result(error);
                }
                PopoverMsg::SetDashboardButton { enabled } => {
                    let error = config::set_dashboard_button(enabled)
                        .err()
                        .map(|error| format!("Could not save Dashboard setting: {error}"));
                    popover.push_state(&read_display_state());
                    popover.config_action_result(error);
                }
                PopoverMsg::SetAgentNames { enabled } => {
                    let error = config::set_agent_names(enabled)
                        .err()
                        .map(|error| format!("Could not save agent name setting: {error}"));
                    popover.push_state(&read_display_state());
                    popover.config_action_result(error);
                }
                PopoverMsg::SetGroupByProject { enabled } => {
                    let error = config::set_group_by_project(enabled)
                        .err()
                        .map(|error| format!("Could not save project grouping: {error}"));
                    popover.push_state(&read_display_state());
                    popover.config_action_result(error);
                }
                PopoverMsg::SetUsage { enabled } => {
                    let error = config::update_config(|cfg| {
                        cfg.usage_enabled = enabled;
                        Ok(())
                    })
                    .err()
                    .map(|error| format!("Could not save usage settings: {error}"));
                    // Flip the tray readout and the popover's usage section
                    // immediately. When enabling, the readout stays blank until
                    // the first scan lands (seconds later); when disabling, it
                    // clears now instead of waiting for the refresher.
                    if let Some(tray) = tray_holder.as_ref() {
                        apply_readout(tray);
                    }
                    popover.push_state(&read_display_state());
                    popover.config_action_result(error);
                }
                PopoverMsg::SetLed { enabled } => {
                    let error = config::update_config(|cfg| {
                        cfg.led_enabled = enabled;
                        Ok(())
                    })
                    .err()
                    .map(|error| format!("Could not save lamp setting: {error}"));
                    // The LED daemon thread re-reads config on its own poll and
                    // (dis)connects within a couple of seconds; refresh the
                    // popover now so the footer indicator flips immediately.
                    popover.push_state(&read_display_state());
                    popover.config_action_result(error);
                }
                PopoverMsg::ForgetLamp { id } => {
                    let error = crate::ble::command(crate::ble::BleCommand::Remove { id })
                        .err()
                        .map(|error| format!("Could not forget lamp: {error}"));
                    popover.push_state(&read_display_state());
                    popover.config_action_result(error);
                }
                PopoverMsg::RenameLamp { id, name } => {
                    let error = crate::ble::command(crate::ble::BleCommand::Rename { id, name })
                        .err()
                        .map(|error| format!("Could not save lamp name: {error}"));
                    popover.push_state(&read_display_state());
                    popover.config_action_result(error);
                }
                PopoverMsg::IdentifyLamp { id } => {
                    let error = crate::ble::identify_lamp(&id)
                        .err()
                        .map(|error| format!("Could not identify lamp: {error}"));
                    popover.config_action_result(error);
                }
                PopoverMsg::SetBrightness { percent } => {
                    let error = crate::ble::command(crate::ble::BleCommand::Brightness {
                        percent,
                        id: None,
                    })
                    .err()
                    .map(|error| format!("Could not save light brightness: {error}"));
                    popover.push_state(&read_display_state());
                    popover.config_action_result(error);
                }
                PopoverMsg::SetLampBrightness { id, percent } => {
                    let error = crate::ble::command(crate::ble::BleCommand::Brightness {
                        percent,
                        id: Some(id),
                    })
                    .err()
                    .map(|error| format!("Could not save lamp brightness: {error}"));
                    popover.push_state(&read_display_state());
                    popover.config_action_result(error);
                }
                PopoverMsg::PreviewBrightness { id, percent } => {
                    if let Some(id) = id {
                        if let Err(error) = crate::ble::preview_lamp_brightness(&id, percent) {
                            eprintln!("Could not preview lamp brightness: {error}");
                        }
                    } else {
                        crate::ble::preview_brightness(percent);
                    }
                }
            },
            // Dismiss like a real popover: losing focus (clicking anywhere
            // else) hides it.
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Focused(false),
                ..
            } if window_id == popover.window_id() => popover.on_focus_lost(),
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            Event::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } if window_id == popover.window_id() => popover.hide(),
            // Windows: when the popover crosses (or is assigned) a monitor
            // DPI boundary, tao rescales it to the monitor's DPI — but
            // WebView2 renders at its own devicePixelRatio, which can differ
            // (e.g. a stale system DPI from logon). Keep the window at the
            // size derived from the page's reported ratio; overriding
            // new_inner_size here replaces tao's suggested rescale.
            #[cfg(target_os = "windows")]
            Event::WindowEvent {
                window_id,
                event: WindowEvent::ScaleFactorChanged { new_inner_size, .. },
                ..
            } if window_id == popover.window_id() => {
                if let Some(want) = popover.wanted_physical() {
                    *new_inner_size = want;
                }
            }
            // Belt to the veto's suspenders: if a resize still lands off the
            // wanted size (tao's DPI rescale ignoring the override above),
            // snap back.
            #[cfg(target_os = "windows")]
            Event::WindowEvent {
                window_id,
                event: WindowEvent::Resized(new_size),
                ..
            } if window_id == popover.window_id() => popover.on_window_resized(new_size),
            _ => {}
        }
    })
}

/// Bring the terminal window or desktop task hosting a session to the front, using the
/// identity the hook captured (tty / terminal session ids / host app /
/// ancestor processes — see `terminal::focus`). Falls back to opening the
/// dashboard when enabled and the window can't be found.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn focus_session(session_id: &str, session: Option<&state::SessionStatus>) {
    let session_id = session_id.to_owned();
    let session = session.cloned();
    // Focusing shells out (osascript/open can take a few hundred ms); keep it
    // off the event loop.
    thread::spawn(move || {
        let focused = session.as_ref().is_some_and(|s| {
            crate::terminal::focus(
                &session_id,
                s.harness.as_deref(),
                s.terminal.as_ref(),
                s.project_path.as_deref(),
            )
        });
        if !focused {
            open_dashboard();
        }
    });
}

fn open_dashboard() {
    if config::read_config().show_dashboard_button {
        launch_dashboard();
    }
}

/// Launch the TUI dashboard in a fresh terminal window from the tray menu.
#[cfg(target_os = "macos")]
fn launch_dashboard() {
    let _ = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"Terminal\" to do script \"broglow\"",
            "-e",
            "tell application \"Terminal\" to activate",
        ])
        .spawn();
}

/// Launch the TUI dashboard in a fresh console window. Prefers Windows Terminal
/// (`wt`) when available, falling back to a plain console via `cmd /c start`.
/// Uses the running executable's own path so it works before install puts
/// `broglow` on PATH.
#[cfg(target_os = "windows")]
fn launch_dashboard() {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "broglow".to_string());

    if std::process::Command::new("wt")
        .args(["-w", "0", "nt", &exe])
        .spawn()
        .is_ok()
    {
        return;
    }

    // `start` needs an (empty) title argument when the target path is quoted.
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", &exe])
        .spawn();
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn launch_dashboard() {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "broglow".to_string());
    // Best effort across common Linux terminal emulators.
    for term in ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"] {
        if std::process::Command::new(term)
            .args(["-e", &exe])
            .spawn()
            .is_ok()
        {
            return;
        }
    }
}

/// Process-lifetime flock beside `config.json`; a second daemon fails to take
/// it and exits. Released by the OS when this process ends, however it ends.
#[cfg(unix)]
pub(crate) fn instance_lock_path() -> std::path::PathBuf {
    config::config_file_path().with_file_name(".menubar.lock")
}

#[cfg(unix)]
fn acquire_instance_lock() -> anyhow::Result<std::fs::File> {
    let path = instance_lock_path();
    std::fs::create_dir_all(path.parent().expect("config path must have a parent"))?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    lock.try_lock()
        .map_err(|e| anyhow::anyhow!("A BroGlow tray service is already running: {e}"))?;
    Ok(lock)
}

/// Acquire a process-lifetime named mutex to ensure only one tray daemon runs
/// at a time. Returns `true` if this process is the sole holder (safe to
/// proceed), `false` if another instance already holds it.
#[cfg(target_os = "windows")]
fn acquire_single_instance_lock() -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;

    // UTF-16, NUL-terminated, "Local\" scope keeps it per-session.
    let name: Vec<u16> = "Local\\broglow-menubar"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        let already_running = GetLastError() == ERROR_ALREADY_EXISTS;

        if handle.is_null() {
            // Couldn't create the mutex at all; fail open rather than refuse
            // to start the tray.
            return true;
        }

        // Intentionally leak the handle (never call CloseHandle): we want the
        // mutex held for the entire lifetime of this process so later
        // launches see ERROR_ALREADY_EXISTS. Windows releases it
        // automatically when the process exits (normally or via
        // TerminateProcess), so there is no real leak in practice.

        !already_running
    }
}

/// Hide the tray daemon's console window on Windows so it runs purely in the
/// background. This is a backstop for classic conhost setups (e.g. Windows
/// Terminal not set as the default host) — the primary fix is that
/// `install_run_key` in `main.rs` launches via `conhost.exe --headless`,
/// which sidesteps the pseudoconsole-hiding bug entirely
/// (see https://github.com/microsoft/terminal/issues/12570). When this
/// function does apply, it only ever hides a console window; it has no
/// effect on where stdout/stderr go.
#[cfg(target_os = "windows")]
fn hide_console_window() {
    use windows_sys::Win32::System::Console::{GetConsoleProcessList, GetConsoleWindow};
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    unsafe {
        // Only hide a console we own. When launched at login (its own fresh
        // console) we're the sole attached process and hiding is correct; when
        // run in the foreground from a terminal, a parent shell shares the
        // console, so hiding its window would hide the user's terminal — skip.
        let mut buf = [0u32; 2];
        if GetConsoleProcessList(buf.as_mut_ptr(), buf.len() as u32) != 1 {
            return;
        }
        let hwnd = GetConsoleWindow();
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

#[cfg(test)]
mod menu_rows_tests {
    use super::{menu_rows, MenuRow};
    use crate::session::DisplaySession;
    use crate::state::Status;

    fn session(id: &str, status: Status, project: &str, harness: Option<&str>) -> DisplaySession {
        DisplaySession {
            session_id: id.into(),
            name: format!("Task {id}"),
            status,
            project_name: if project.is_empty() {
                crate::session::NO_PROJECT.into()
            } else {
                project.rsplit('/').next().unwrap().into()
            },
            project_path: project.into(),
            last_updated: String::new(),
            host: None,
            harness: harness.map(str::to_string),
        }
    }

    #[test]
    fn menu_rows_group_under_project_headings_and_drop_done_sessions() {
        // Already in the shared order: grouped by project, done inside its group.
        let sessions = vec![
            session("help", Status::NeedsHelp, "/a/recco", None),
            session("review", Status::Active, "/a/recco", Some("codex")),
            session("old", Status::Done, "/a/recco", None),
            session("notes", Status::Inactive, "", Some("copilot")),
        ];
        let text = |row: &MenuRow| match row {
            MenuRow::Heading(t) => t.clone(),
            MenuRow::Session { text, .. } => text.clone(),
        };
        let grouped: Vec<String> = menu_rows(&sessions, true, true).iter().map(text).collect();
        assert_eq!(
            grouped,
            [
                "📁 recco",
                "  🔴 Task help (needs help) — [cc] Claude",
                "  🟢 Task review (working) — [cx] Codex",
                "📁 No project",
                "  🟠 Task notes (paused) — [co] Copilot",
            ]
        );
        let tags: Vec<String> = menu_rows(&sessions, true, false).iter().map(text).collect();
        assert_eq!(tags[1], "  🔴 Task help (needs help) — [cc]");
        assert!(
            matches!(&menu_rows(&sessions, true, true)[1], MenuRow::Session { id, .. } if id == "help")
        );

        let flat: Vec<String> = menu_rows(&sessions, false, true).iter().map(text).collect();
        assert_eq!(
            flat,
            [
                "🔴 Task help (needs help) — [cc] Claude · recco",
                "🟢 Task review (working) — [cx] Codex · recco",
                "🟠 Task notes (paused) — [co] Copilot · No project",
            ]
        );
        assert!(menu_rows(&[], true, true).is_empty());
    }
}
