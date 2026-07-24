mod app;
mod codex;
mod config;
mod copilot;
mod harness;
mod hook;
mod led;
mod menubar;
mod notification;
mod ota;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod popover;
mod session;
mod spawn;
mod state;
mod terminal;
mod ui;
mod usage;

use std::io;
use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::app::App;

#[derive(Parser)]
#[command(
    name = "clawlight",
    about = "TUI dashboard for Claude Code, opencode, Codex CLI, and GitHub Copilot CLI sessions"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Install hooks into ~/.claude/settings.json and start the tray daemon
    Install,
    /// Uninstall hooks and clean up
    Uninstall,
    /// Run the menu bar / system tray daemon (foreground)
    Menubar,
    /// Mirror session state to a Seeed XIAO ESP32-C6 over USB serial (foreground)
    Led {
        /// Serial device path (default: auto-detect the Seeed XIAO ESP32-C6)
        #[arg(long)]
        port: Option<String>,
    },
    /// Push new firmware to the ESP32-C6 over serial — no cable reflash
    Update {
        /// espflash image to install (output of `espflash save-image`)
        firmware: String,
        /// Serial device path (default: auto-detect / configured led_port)
        #[arg(long)]
        port: Option<String>,
    },
    /// (internal) Print today's usage snapshot (tokens / $ / plan %) as JSON
    #[command(hide = true)]
    Usage,
    /// (internal) Hook backend invoked by Claude Code over stdin
    #[command(hide = true)]
    Hook,
    /// (internal) Normalized-event backend for non-Claude harnesses (opencode)
    #[command(hide = true)]
    Event,
    /// (internal) Codex hook shim: Claude-dialect payload → normalized event
    #[command(hide = true)]
    CodexHook,
    /// (internal) Copilot hook shim: per-event payload → normalized event
    #[command(hide = true)]
    CopilotHook {
        /// Copilot lifecycle event this registration fires for (the payload
        /// itself doesn't name it)
        event: String,
    },
    /// (internal) Generate a session name from a transcript
    #[command(hide = true)]
    Name {
        session_id: String,
        transcript_path: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Install) => install_hooks(),
        Some(Commands::Uninstall) => uninstall_hooks(),
        Some(Commands::Menubar) => menubar::run(),
        Some(Commands::Led { port }) => led::run(port),
        Some(Commands::Update { firmware, port }) => ota::run(firmware, port),
        Some(Commands::Usage) => usage::run_once(),
        Some(Commands::Hook) => hook::run(),
        Some(Commands::Event) => hook::run_event(),
        Some(Commands::CodexHook) => hook::run_codex_hook(),
        Some(Commands::CopilotHook { event }) => hook::run_copilot_hook(&event),
        Some(Commands::Name {
            session_id,
            transcript_path,
        }) => hook::run_namer(&session_id, &transcript_path),
        None => run_tui(),
    }
}

fn run_tui() -> anyhow::Result<()> {
    // On a machine where clawlight isn't wired into Claude Code yet, treat the
    // first dashboard launch as the install step. Runs before we touch the
    // terminal so its output lands in normal scrollback, not the alt-screen.
    first_run_setup_tui();

    // Set up panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(panic_info);
    }));

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let mut app = App::new();
    let result = app.run(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn hook_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Home directory must exist")
        .join(".claude")
        .join("clawlight")
}

fn settings_path() -> PathBuf {
    dirs::home_dir()
        .expect("Home directory must exist")
        .join(".claude")
        .join("settings.json")
}

// ----------------------------------------------------------------------------
// Install / uninstall
// ----------------------------------------------------------------------------

/// Claude Code lifecycle events that clawlight's built-in hook backend
/// registers for. Single source of truth shared by install and uninstall.
const HOOK_EVENTS: [&str; 6] = [
    "SessionStart",
    "UserPromptSubmit",
    "Stop",
    "Notification",
    "SessionEnd",
    "PreToolUse",
];

/// Register clawlight's native hook backend in ~/.claude/settings.json for every
/// lifecycle event, and ensure the state/log dir exists. Idempotent. Does *not*
/// touch autostart — `install_hooks` layers that on top; the menu-bar daemon's
/// first-run path deliberately calls only this (see `first_run_setup_daemon`).
fn register_hooks() -> anyhow::Result<()> {
    // 1. Ensure the clawlight state/log directory exists.
    std::fs::create_dir_all(hook_dir())?;

    // 2. Register the native hook backend in settings.json. The hook command is
    //    this very binary invoked as `clawlight hook` — no bash or jq needed.
    let exe = std::env::current_exe().context("Resolving current executable path")?;
    let hook_cmd = format!("\"{}\" hook", exe.display());

    let settings_file = settings_path();
    let mut settings: serde_json::Value = if settings_file.exists() {
        let content = std::fs::read_to_string(&settings_file).context("Reading settings.json")?;
        serde_json::from_str(&content).context("Parsing settings.json")?
    } else {
        serde_json::json!({})
    };

    let hook_entry = serde_json::json!([
        {
            "hooks": [{
                "type": "command",
                "command": hook_cmd
            }]
        }
    ]);

    let hooks = settings
        .as_object_mut()
        .context("settings.json must be an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let hooks_obj = hooks.as_object_mut().context("hooks must be an object")?;

    for event in HOOK_EVENTS {
        hooks_obj.insert(event.to_string(), hook_entry.clone());
    }

    let settings_str = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&settings_file, settings_str)?;

    println!("Updated {}", settings_file.display());

    // Set up every detected non-Claude harness (opencode today). Best-effort and
    // detection-gated per adapter: a failure never breaks Claude hook
    // registration, and machines without those agents see nothing. Sits in
    // register_hooks (not install_hooks) so both first-run paths — the TUI's full
    // install and the daemon's hooks-only bootstrap — set them up; adapters
    // touch no autostart, so this is safe from the daemon path.
    harness::install_all();

    Ok(())
}

fn install_hooks() -> anyhow::Result<()> {
    // 1./2. Register the hook backend in settings.json.
    register_hooks()?;

    // 3. If a status board is already plugged in on this first setup, turn the
    //    LEDs on automatically so the lamp lights up with no extra step.
    let lamp_enabled = maybe_autoenable_led();

    // 4. Install autostart for the tray daemon (platform-specific).
    install_autostart()?;

    println!(
        "\nInstallation complete! Hooks are now active for new Claude Code sessions, plus opencode, Codex CLI, and GitHub Copilot CLI wherever detected."
    );
    println!("Run `clawlight` to launch the TUI dashboard.");
    if !lamp_enabled {
        println!("Optional: plug in a Seeed XIAO ESP32-C6 status board and press `l` in the dashboard to enable status LEDs.");
    }
    Ok(())
}

/// On a clean first install, enable the status LEDs automatically if a clawlight
/// board is already plugged in — so "plug it in, install, done" just works.
///
/// Gated on there being no `config.json` yet: a config file means the user has
/// already made a choice in the popover/TUI, so re-running install must never
/// flip the LEDs back on against a deliberate opt-out. Detection matches only
/// the XIAO's native USB vendor ID (see `led::detect_board`), so a positive hit
/// is unambiguously our board — enabling it can't drive an unrelated device.
///
/// Returns whether the LEDs ended up enabled (either just now or already), so
/// the caller can tailor the closing hint. Best-effort: a config write failure
/// just leaves the LEDs off, recoverable with `l` in the dashboard.
fn maybe_autoenable_led() -> bool {
    let cfg = config::read_config();
    if config::config_file_path().exists() {
        // Not a clean first run — respect whatever the user already chose.
        return cfg.led_enabled;
    }
    let Some(port) = led::detect_board() else {
        return false;
    };
    let cfg = config::Config {
        led_enabled: true,
        ..cfg
    };
    match config::write_config(&cfg) {
        Ok(()) => {
            println!("Detected a Claw Light board at {port} — status LEDs enabled.");
            true
        }
        Err(e) => {
            eprintln!("clawlight: could not enable status LEDs: {e:#} (press `l` in the dashboard to enable them).");
            false
        }
    }
}

/// True if clawlight's hook backend is already wired into settings.json for at
/// least one lifecycle event. Gates the first-run auto-setup so a normal launch
/// (TUI or tray daemon) doesn't rewrite settings — or, on the TUI path,
/// re-bootstrap the LaunchAgent — every time.
fn hooks_registered() -> bool {
    let Ok(content) = std::fs::read_to_string(settings_path()) else {
        return false;
    };
    let Ok(settings) = serde_json::from_str::<serde_json::Value>(&content) else {
        // Unparseable settings.json (a mid-write snapshot, or a schema we don't
        // understand): stay hands-off rather than auto-clobber a file we can't
        // read — mirrors hook.rs's "never write on a failed read" rule.
        return true;
    };
    let Some(hooks) = settings.get("hooks").and_then(|h| h.as_object()) else {
        return false;
    };
    HOOK_EVENTS.iter().any(|event| {
        hooks
            .get(*event)
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .filter_map(|matcher| matcher.get("hooks").and_then(|h| h.as_array()))
            .flatten()
            .filter_map(|hook| hook.get("command").and_then(|c| c.as_str()))
            .any(|cmd| cmd.contains("clawlight"))
    })
}

/// First-run onboarding for the TUI. If clawlight isn't wired into Claude Code
/// yet, run the full install (hooks + login autostart, which also brings up the
/// tray daemon) so that `brew install clawlight && clawlight` — or a plain
/// `cargo install` then launch — is a complete setup with no manual step.
///
/// Safe here because the TUI is a foreground process distinct from the daemon:
/// kickstarting the LaunchAgent spawns the tray we want, with nothing to collide
/// with on a fresh machine. Best-effort — a failure just leaves `clawlight
/// install` as the manual fallback and never blocks the dashboard.
fn first_run_setup_tui() {
    if hooks_registered() {
        return;
    }
    println!(
        "First run — registering hooks for Claude Code and any other detected coding agents, and starting the menu bar daemon…\n"
    );
    if let Err(e) = install_hooks() {
        eprintln!(
            "clawlight: first-run setup failed: {e:#}\nRun `clawlight install` to finish setup."
        );
    }
}

/// First-run onboarding for the tray daemon. Ensure the hooks are registered so
/// sessions report status, but — unlike the TUI path — deliberately skip
/// autostart. The daemon must never bootstrap/kickstart its own LaunchAgent (or
/// spawn a detached `menubar` on Linux/Windows): it is already running, and
/// doing so would spin up a duplicate tray. Login autostart is established by
/// the TUI's first run or by `clawlight install`.
pub fn first_run_setup_daemon() {
    if hooks_registered() {
        return;
    }
    if let Err(e) = register_hooks() {
        eprintln!("clawlight: first-run hook registration failed: {e:#}");
    }
}

fn uninstall_hooks() -> anyhow::Result<()> {
    // Remove autostart first (best-effort).
    let _ = uninstall_autostart();

    // Remove every harness's wiring (best-effort; each is marker-guarded so a
    // hand-rolled file is never deleted).
    harness::uninstall_all();

    // Remove hooks from settings.json
    let settings_file = settings_path();
    if settings_file.exists() {
        let content = std::fs::read_to_string(&settings_file)?;
        let mut settings: serde_json::Value = serde_json::from_str(&content)?;

        if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
            for event in HOOK_EVENTS {
                hooks.remove(event);
            }
            if hooks.is_empty() {
                settings.as_object_mut().unwrap().remove("hooks");
            }
        }

        let settings_str = serde_json::to_string_pretty(&settings)?;
        std::fs::write(&settings_file, settings_str)?;
        println!("Removed hooks from {}", settings_file.display());
    }

    // Remove hook state and logs
    let dir = hook_dir();
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
        println!("Removed {}", dir.display());
    }

    println!("\nUninstall complete.");
    Ok(())
}

// ----------------------------------------------------------------------------
// Autostart — platform dispatch
// ----------------------------------------------------------------------------

fn install_autostart() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        install_launch_agent()
    }
    #[cfg(target_os = "windows")]
    {
        install_run_key()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        install_xdg_autostart()
    }
}

fn uninstall_autostart() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        uninstall_launch_agent()
    }
    #[cfg(target_os = "windows")]
    {
        uninstall_run_key()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        uninstall_xdg_autostart()
    }
}

// ----------------------------------------------------------------------------
// Autostart — Windows (registry Run key)
// ----------------------------------------------------------------------------

#[cfg(target_os = "windows")]
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(target_os = "windows")]
const RUN_VALUE: &str = "clawlight";

#[cfg(target_os = "windows")]
fn install_run_key() -> anyhow::Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let exe = std::env::current_exe().context("Resolving current executable path")?;

    // Launch through conhost in headless mode instead of invoking the
    // console-subsystem exe directly. On Windows 11 with Windows Terminal set
    // as the default terminal host, a directly-launched console app gets a
    // pseudoconsole HWND from GetConsoleWindow() that ShowWindow(SW_HIDE)
    // cannot hide (see https://github.com/microsoft/terminal/issues/12570),
    // so every login would otherwise leave a visible empty terminal window
    // whose closure kills the tray. `conhost.exe --headless` forces a
    // windowless pseudoconsole regardless of the user's default-terminal
    // setting.
    let command = format!("conhost.exe --headless \"{}\" menubar", exe.display());

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(RUN_KEY)
        .context("Opening HKCU Run registry key")?;
    key.set_value(RUN_VALUE, &command)
        .context("Writing autostart registry value")?;
    println!("Registered tray autostart (HKCU\\...\\Run\\{RUN_VALUE}).");

    // Start the tray now, detached and windowless, so the icon appears
    // immediately without waiting for the next login.
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("menubar")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::spawn::configure_detached(&mut cmd);
    let _ = cmd.spawn();
    println!("Tray icon launched — look for the Clawd icon in the system tray.");
    Ok(())
}

#[cfg(target_os = "windows")]
fn uninstall_run_key() -> anyhow::Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_ALL_ACCESS) {
        let _ = key.delete_value(RUN_VALUE);
        println!("Removed tray autostart registry value.");
    }
    println!(
        "Note: a running tray icon stays until you quit it (tray menu \u{2192} Quit) or log out."
    );
    Ok(())
}

// ----------------------------------------------------------------------------
// Autostart — macOS (launchd LaunchAgent)
// ----------------------------------------------------------------------------

#[cfg(target_os = "macos")]
pub(crate) const LAUNCH_AGENT_LABEL: &str = "io.roush.clawlight.menubar";

#[cfg(target_os = "macos")]
const LAUNCH_AGENT_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{{LABEL}}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{{BIN}}</string>
        <string>menubar</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{{HOME}}/.claude/clawlight/menubar.log</string>
    <key>StandardErrorPath</key>
    <string>{{HOME}}/.claude/clawlight/menubar.err</string>
</dict>
</plist>
"#;

#[cfg(target_os = "macos")]
pub(crate) fn launch_agent_path() -> PathBuf {
    dirs::home_dir()
        .expect("Home directory must exist")
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LAUNCH_AGENT_LABEL}.plist"))
}

#[cfg(target_os = "macos")]
pub(crate) fn current_uid() -> anyhow::Result<String> {
    let out = std::process::Command::new("id")
        .arg("-u")
        .output()
        .context("Running `id -u`")?;
    if !out.status.success() {
        anyhow::bail!("`id -u` failed");
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(target_os = "macos")]
fn install_launch_agent() -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("Resolving current executable path")?;
    let home = dirs::home_dir().context("No home directory")?;

    let plist_content = LAUNCH_AGENT_PLIST
        .replace("{{LABEL}}", LAUNCH_AGENT_LABEL)
        .replace("{{BIN}}", &exe.display().to_string())
        .replace("{{HOME}}", &home.display().to_string());

    let plist_path = launch_agent_path();
    if let Some(parent) = plist_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&plist_path, plist_content)?;
    println!("Wrote LaunchAgent plist to {}", plist_path.display());

    // Ad-hoc code signing (best effort — silently skip if codesign isn't available)
    let _ = std::process::Command::new("codesign")
        .args(["--sign", "-", "--force"])
        .arg(&exe)
        .output();

    let uid = current_uid()?;
    let target = format!("gui/{uid}");
    let service = format!("gui/{uid}/{LAUNCH_AGENT_LABEL}");

    // Bootout any existing instance (ignore failure — first install will have nothing to remove)
    let _ = std::process::Command::new("launchctl")
        .args(["bootout", &service])
        .output();

    // bootout returns before the daemon process has actually exited.
    // If we bootstrap too quickly, launchd returns "I/O error" because the
    // service is mid-teardown. Poll up to ~5s for the service to fully unload.
    for _ in 0..20 {
        let still_loaded = std::process::Command::new("launchctl")
            .args(["print", &service])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !still_loaded {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    let bootstrap = std::process::Command::new("launchctl")
        .args(["bootstrap", &target])
        .arg(&plist_path)
        .output()
        .context("Running `launchctl bootstrap`")?;
    if !bootstrap.status.success() {
        let stderr = String::from_utf8_lossy(&bootstrap.stderr);
        anyhow::bail!("launchctl bootstrap failed: {}", stderr.trim());
    }

    // Force immediate start (RunAtLoad usually does this, but -k makes it deterministic)
    let _ = std::process::Command::new("launchctl")
        .args(["kickstart", "-k", &service])
        .output();

    println!("LaunchAgent loaded — menu bar icon should appear momentarily.");
    Ok(())
}

#[cfg(target_os = "macos")]
fn uninstall_launch_agent() -> anyhow::Result<()> {
    if let Ok(uid) = current_uid() {
        let service = format!("gui/{uid}/{LAUNCH_AGENT_LABEL}");
        let _ = std::process::Command::new("launchctl")
            .args(["bootout", &service])
            .output();
    }

    let plist_path = launch_agent_path();
    if plist_path.exists() {
        std::fs::remove_file(&plist_path)?;
        println!("Removed {}", plist_path.display());
    }
    Ok(())
}

// ----------------------------------------------------------------------------
// Autostart — Linux (XDG autostart entry)
// ----------------------------------------------------------------------------

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn xdg_autostart_path() -> PathBuf {
    dirs::home_dir()
        .expect("Home directory must exist")
        .join(".config")
        .join("autostart")
        .join("clawlight.desktop")
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn install_xdg_autostart() -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("Resolving current executable path")?;

    let desktop_entry = format!(
        "[Desktop Entry]\nType=Application\nName=clawlight\nExec=\"{}\" menubar\nX-GNOME-Autostart-enabled=true\n",
        exe.display()
    );

    let entry_path = xdg_autostart_path();
    if let Some(parent) = entry_path.parent() {
        std::fs::create_dir_all(parent).context("Creating ~/.config/autostart")?;
    }
    std::fs::write(&entry_path, desktop_entry).context("Writing XDG autostart entry")?;
    println!("Wrote XDG autostart entry to {}", entry_path.display());

    // Start the tray now, detached, so the icon appears immediately without
    // waiting for the next login — mirrors the Windows/macOS behavior.
    // Note: whether a tray icon actually shows up depends on the desktop
    // environment's support for the AppIndicator/StatusNotifierItem
    // protocol (GNOME needs an extension; most other DEs work out of the
    // box), so this is best-effort on Linux.
    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("menubar")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::spawn::configure_detached(&mut cmd); // no-op on this platform
    let _ = cmd.spawn();
    println!("Tray icon launched (if your desktop environment supports system-tray icons).");
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn uninstall_xdg_autostart() -> anyhow::Result<()> {
    let entry_path = xdg_autostart_path();
    if entry_path.exists() {
        std::fs::remove_file(&entry_path)?;
        println!("Removed {}", entry_path.display());
    }
    println!(
        "Note: a running tray icon stays until you quit it (tray menu \u{2192} Quit) or log out."
    );
    Ok(())
}
