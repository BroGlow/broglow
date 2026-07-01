use std::path::Path;
use std::sync::mpsc;
use std::thread;

use anyhow::Context;
use notify::{EventKind, RecursiveMode, Watcher};
use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
#[cfg(target_os = "macos")]
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};

use crate::state::{self, aggregate, read_hook_state, Aggregate, HookState, Status};

const ICON_GREEN: &[u8] = include_bytes!("../assets/icons/clawd-green.png");
const ICON_YELLOW: &[u8] = include_bytes!("../assets/icons/clawd-yellow.png");
const ICON_RED: &[u8] = include_bytes!("../assets/icons/clawd-red.png");
const ICON_NONE: &[u8] = include_bytes!("../assets/icons/clawd-none.png");

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

struct MenuIds {
    open_clawlight: MenuId,
    quit: MenuId,
}

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

    if live.is_empty() {
        menu.append(&MenuItem::new("No live sessions", false, None))?;
    } else {
        for (id, s) in &live {
            let name = s.name.clone().unwrap_or_else(|| {
                format!("Session {}", &id[..8.min(id.len())])
            });
            let project = s
                .project_path
                .as_deref()
                .map(|p| {
                    Path::new(p)
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| p.to_string())
                })
                .unwrap_or_else(|| "unknown".to_string());
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
            menu.append(&MenuItem::new(text, false, None))?;
        }
    }

    menu.append(&PredefinedMenuItem::separator())?;
    let open_clawlight = MenuItem::new("Open clawlight", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let ids = MenuIds {
        open_clawlight: open_clawlight.id().clone(),
        quit: quit.id().clone(),
    };
    menu.append(&open_clawlight)?;
    menu.append(&quit)?;

    Ok((menu, ids))
}

enum UserEvent {
    StateChanged,
    Menu(MenuEvent),
}

pub fn run() -> anyhow::Result<()> {
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

    // Drive the optional ESP32 status LEDs in the background. This is inert
    // (touches no serial port) unless the user has enabled it via `l` in the
    // TUI, so it's safe to always spawn.
    thread::spawn(|| crate::led::run_daemon());

    let initial_state = read_hook_state();
    let (menu, mut ids) = build_menu(&initial_state)?;
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_icon(icon_for(aggregate(&initial_state))?)
        .with_tooltip("clawlight")
        .build()
        .context("Building tray icon")?;

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
                if let Ok((menu, new_ids)) = build_menu(&state) {
                    if let Some(tray) = tray_holder.as_ref() {
                        let _ = tray.set_menu(Some(Box::new(menu)));
                        if let Ok(icon) = icon_for(aggregate(&state)) {
                            let _ = tray.set_icon(Some(icon));
                        }
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
                }
            }
            _ => {}
        }
    })
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

/// Detach the tray daemon from its console window on Windows so it runs purely
/// in the background. The process keeps stdout/stderr (redirected to the log
/// files by the launcher), it just has no visible window.
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
