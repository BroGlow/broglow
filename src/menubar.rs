use std::path::Path;
use std::sync::mpsc;
use std::thread;

use anyhow::Context;
use notify::{EventKind, RecursiveMode, Watcher};
use tao::event::Event;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use tao::event::WindowEvent;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
use tray_icon::{Icon, TrayIconBuilder};

use crate::config;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::config::BillingMode;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::popover::{Popover, PopoverMsg};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use crate::usage;
use crate::state::{self, aggregate, read_hook_state, Aggregate, HookState};
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
use crate::state::Status;

pub(crate) const ICON_GREEN: &[u8] = include_bytes!("../assets/icons/clawd-green.png");
pub(crate) const ICON_YELLOW: &[u8] = include_bytes!("../assets/icons/clawd-yellow.png");
pub(crate) const ICON_RED: &[u8] = include_bytes!("../assets/icons/clawd-red.png");
pub(crate) const ICON_NONE: &[u8] = include_bytes!("../assets/icons/clawd-none.png");

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

/// Session display name: the auto-namer's title when present, else a
/// truncated session id. Char-based truncation — ids are ours, but keep the
/// repo-wide rule of never byte-slicing.
pub(crate) fn display_name(id: &str, s: &state::SessionStatus) -> String {
    s.name.clone().unwrap_or_else(|| {
        let prefix: String = id.chars().take(8).collect();
        format!("Session {prefix}")
    })
}

/// Last path component of the session's project directory.
pub(crate) fn project_label(s: &state::SessionStatus) -> String {
    s.project_path
        .as_deref()
        .map(|p| {
            Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

struct MenuIds {
    open_clawlight: MenuId,
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
    menu.append(&MenuItem::new(
        format!("  Active: {active}"),
        false,
        None,
    ))?;
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

    let mut live: Vec<(&String, &state::SessionStatus)> = state
        .sessions
        .iter()
        .filter(|(_, s)| s.status != Status::Done)
        .collect();
    live.sort_by_key(|(_, s)| match s.status {
        Status::NeedsHelp => 0,
        Status::Active => 1,
        Status::Inactive => 2,
        Status::Done => 3,
    });

    let mut session_ids = Vec::new();
    if live.is_empty() {
        menu.append(&MenuItem::new("No live sessions", false, None))?;
    } else {
        for (id, s) in &live {
            let name = display_name(id, s);
            let project = project_label(s);
            let badge = match s.status {
                Status::NeedsHelp => "needs help",
                Status::Active => "working",
                Status::Inactive => "paused",
                Status::Done => "done",
            };
            let prefix = match s.status {
                Status::NeedsHelp => "🔴",
                Status::Active => "🟢",
                Status::Inactive => "🟠",
                Status::Done => "⚪",
            };
            let text = format!("{prefix} {name} ({badge}) — {project}");
            // Clickable: focus the session's terminal window (best-effort).
            let item = MenuItem::new(text, true, None);
            menu.append(&item)?;
            session_ids.push((item.id().clone(), (*id).clone()));
        }
    }

    menu.append(&PredefinedMenuItem::separator())?;
    let open_clawlight = MenuItem::new("Open clawlight", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let ids = MenuIds {
        open_clawlight: open_clawlight.id().clone(),
        quit: quit.id().clone(),
        sessions: session_ids,
    };
    menu.append(&open_clawlight)?;
    menu.append(&quit)?;

    Ok((menu, ids))
}

/// Minimal right-click menu for macOS/Windows: the sessions live in the
/// popover (left click); this stays as a safety hatch so Quit is always
/// reachable even if the webview fails.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn build_fallback_menu() -> anyhow::Result<(Menu, MenuIds)> {
    let menu = Menu::new();
    let open_clawlight = MenuItem::new("Open Dashboard", true, None);
    let settings = MenuItem::new("Settings…  (soon)", false, None);
    let quit = MenuItem::new("Quit clawlight", true, None);
    let ids = MenuIds {
        open_clawlight: open_clawlight.id().clone(),
        quit: quit.id().clone(),
    };
    menu.append(&open_clawlight)?;
    menu.append(&settings)?;
    menu.append(&PredefinedMenuItem::separator())?;
    menu.append(&quit)?;
    Ok((menu, ids))
}

enum UserEvent {
    StateChanged,
    Menu(MenuEvent),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    Tray(TrayIconEvent),
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    Popover(PopoverMsg),
    /// The usage refresher produced a new snapshot (design 1a/1c readout).
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    UsageChanged,
}

/// Short usage readout for the tray: the plan's 5h-block percentage, or
/// today's API-equivalent dollars — per the billing mode setting. `None`
/// until the first scan lands (or plan data is unavailable and nothing ran
/// today), which keeps the bar clean instead of showing a zero.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn usage_readout(mode: BillingMode) -> Option<String> {
    let u = usage::latest()?;
    let dollars = || (u.today_tokens > 0).then(|| format!("${:.2}", u.today_cost));
    match mode {
        BillingMode::Plan => u
            .five_hour_pct
            .map(|p| format!("{p:.0}%"))
            .or_else(dollars),
        BillingMode::Api => dollars(),
    }
}

/// Put the readout where the platform can show it: next to the menu bar icon
/// on macOS (design 1a), in the tray tooltip on Windows (design 1c). Shows
/// nothing while usage tracking is off (the opt-in default).
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn apply_readout(tray: &tray_icon::TrayIcon) {
    let cfg = config::read_config();
    let text = cfg
        .usage_enabled
        .then(|| usage_readout(cfg.billing_mode))
        .flatten();
    #[cfg(target_os = "macos")]
    tray.set_title(text.as_deref());
    #[cfg(target_os = "windows")]
    {
        let tooltip = match text {
            Some(t) => format!("clawlight — {t}"),
            None => "clawlight".to_string(),
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

pub fn run() -> anyhow::Result<()> {
    // Guard against running a second tray daemon (e.g. re-running
    // `clawlight install`, or a user manually launching `clawlight menubar`
    // while the autostart instance is already up). Two daemons means two
    // tray icons and two LED/net background threads fighting over the same
    // serial port.
    #[cfg(target_os = "windows")]
    if !acquire_single_instance_lock() {
        println!("clawlight tray is already running — not starting a second instance.");
        return Ok(());
    }

    // On Windows this binary is a console app (it also hosts the TUI), so the
    // tray daemon would otherwise leave an empty console window open. Hide it.
    #[cfg(target_os = "windows")]
    hide_console_window();

    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
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

    // Drive the optional ESP32 status LEDs in the background. This is inert
    // (touches no serial port) unless the user has enabled it via `l` in the
    // TUI, so it's safe to always spawn.
    thread::spawn(|| crate::led::run_daemon());

    let initial_state = read_hook_state();

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    let (menu, ids) = build_fallback_menu()?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let (menu, mut ids) = build_menu(&initial_state)?;

    #[allow(unused_mut)]
    let mut tray_builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon_for_state(&initial_state)?)
        .with_tooltip("clawlight");
    // Left click opens the popover; the native menu stays on right click.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        tray_builder = tray_builder.with_menu_on_left_click(false);
    }
    let tray = tray_builder.build().context("Building tray icon")?;

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
    // be inspected/screenshotted without clicking the real tray icon.
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    if std::env::var_os("CLAWLIGHT_POPOVER_DEBUG").is_some() {
        popover.open_at(
            tray_icon::Rect {
                position: tao::dpi::PhysicalPosition::new(1200.0, 0.0),
                size: tao::dpi::PhysicalSize::new(48, 48),
            },
            &initial_state,
        );
    }

    let proxy_for_watcher = event_loop.create_proxy();
    thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let mut watcher = match notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| {
                let _ = tx.send(res);
            },
        ) {
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

    let mut tray_holder = Some(tray);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(UserEvent::StateChanged) => {
                let state = read_hook_state();
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
                        let _ = tray.set_menu(Some(Box::new(menu)));
                    }
                    ids = new_ids;
                }
            }
            Event::UserEvent(UserEvent::Menu(ev)) => {
                if ev.id == ids.open_clawlight {
                    open_dashboard();
                } else if ev.id == ids.quit {
                    tray_holder.take();
                    std::process::exit(0);
                } else {
                    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                    if let Some((_, sid)) = ids.sessions.iter().find(|(mid, _)| *mid == ev.id) {
                        let session = read_hook_state().sessions.get(sid).cloned();
                        thread::spawn(move || {
                            if let Some(s) = session {
                                let _ = crate::terminal::focus(
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
                popover.push_state(&read_hook_state());
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
                    popover.open_at(rect, &read_hook_state());
                }
            }
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            Event::UserEvent(UserEvent::Popover(msg)) => match msg {
                PopoverMsg::Ready => popover.push_state(&read_hook_state()),
                PopoverMsg::Resize { height } => popover.on_resize(height),
                PopoverMsg::Focus { id } => {
                    popover.hide();
                    let state = read_hook_state();
                    focus_session(state.sessions.get(&id));
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
                    let mut cfg = config::read_config();
                    cfg.yellow_mode = mode;
                    if let Err(e) = config::write_config(&cfg) {
                        eprintln!("Failed to save settings: {e}");
                    }
                    // The config write also lands in the watched directory and
                    // triggers StateChanged, but update directly so the icon
                    // and popover can't lag behind the click.
                    let state = read_hook_state();
                    if let Some(tray) = tray_holder.as_ref() {
                        if let Ok(icon) = icon_for_state(&state) {
                            let _ = tray.set_icon(Some(icon));
                        }
                    }
                    popover.push_state(&state);
                }
                PopoverMsg::SetUsage { enabled, mode } => {
                    let mut cfg = config::read_config();
                    cfg.usage_enabled = enabled;
                    cfg.billing_mode = mode;
                    if let Err(e) = config::write_config(&cfg) {
                        eprintln!("Failed to save settings: {e}");
                    }
                    // Flip the tray readout and the popover's usage section
                    // immediately. When enabling, the readout stays blank until
                    // the first scan lands (seconds later); when disabling, it
                    // clears now instead of waiting for the refresher.
                    if let Some(tray) = tray_holder.as_ref() {
                        apply_readout(tray);
                    }
                    popover.push_state(&read_hook_state());
                }
            },
            // Dismiss like a real popover: losing focus (clicking anywhere
            // else) hides it.
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
            _ => {}
        }
    })
}

/// Bring the terminal window hosting a session to the front, using the
/// identity the hook captured (tty / terminal session ids / host app /
/// ancestor processes — see `terminal::focus`). Falls back to opening the
/// dashboard when the window can't be found.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn focus_session(session: Option<&state::SessionStatus>) {
    let session = session.cloned();
    // Focusing shells out (osascript/open can take a few hundred ms); keep it
    // off the event loop.
    thread::spawn(move || {
        let focused = session.as_ref().is_some_and(|s| {
            crate::terminal::focus(s.terminal.as_ref(), s.project_path.as_deref())
        });
        if !focused {
            open_dashboard();
        }
    });
}

/// Launch the TUI dashboard in a fresh terminal window from the tray menu.
#[cfg(target_os = "macos")]
fn open_dashboard() {
    let _ = std::process::Command::new("osascript")
        .args([
            "-e",
            "tell application \"Terminal\" to do script \"clawlight\"",
            "-e",
            "tell application \"Terminal\" to activate",
        ])
        .spawn();
}

/// Launch the TUI dashboard in a fresh console window. Prefers Windows Terminal
/// (`wt`) when available, falling back to a plain console via `cmd /c start`.
/// Uses the running executable's own path so it works before install puts
/// `clawlight` on PATH.
#[cfg(target_os = "windows")]
fn open_dashboard() {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "clawlight".to_string());

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
fn open_dashboard() {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "clawlight".to_string());
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

/// Acquire a process-lifetime named mutex to ensure only one tray daemon runs
/// at a time. Returns `true` if this process is the sole holder (safe to
/// proceed), `false` if another instance already holds it.
#[cfg(target_os = "windows")]
fn acquire_single_instance_lock() -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;

    // UTF-16, NUL-terminated, "Local\" scope keeps it per-session.
    let name: Vec<u16> = "Local\\clawlight-menubar"
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
