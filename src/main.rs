mod app;
mod ble;
mod codex;
mod config;
mod copilot;
mod flash;
mod harness;
mod hook;
mod led;
mod menubar;
#[cfg(target_os = "macos")]
mod menubar_usage;
mod notification;
mod opencode_usage;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod popover;
mod session;
mod spawn;
mod state;
mod terminal;
mod ui;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod update;
mod usage;

use std::io;
#[cfg(not(windows))]
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
    name = "broglow",
    version,
    about = "TUI dashboard for Claude Code, opencode, Codex CLI, and GitHub Copilot CLI sessions"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Install agent hooks, enable start at login and launch the tray app
    Install,
    /// Uninstall hooks and clean up
    Uninstall,
    /// Run the menu bar / system tray daemon (foreground)
    Menubar,
    /// Show or hide terminal-dashboard entry points in the tray app (default: hidden)
    DashboardButton {
        #[arg(value_parser = ["show", "hide"])]
        visibility: String,
    },
    /// Mirror session state to an ESP32-C3 SuperMini lamp over USB serial (foreground)
    Led {
        /// Serial device path (default: scan Espressif USB devices)
        #[arg(long)]
        port: Option<String>,
    },
    /// Select and control the Bluetooth traffic light (macOS)
    Ble {
        #[command(subcommand)]
        command: ble::BleCommand,
    },
    /// Write this release's ESP32-C3 firmware to the USB lamp (keeps its Bluetooth bonds)
    Flash(flash::FlashArgs),
    /// (internal) Print today's usage snapshot (tokens / plan %) as JSON
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

    #[cfg(target_os = "macos")]
    let app_launch = {
        use std::io::IsTerminal;
        cli.command.is_none()
            && mac_app_launch(&std::env::current_exe()?, io::stdin().is_terminal())
    };
    #[cfg(not(target_os = "macos"))]
    let app_launch = false;
    let command = if app_launch {
        Some(Commands::Menubar)
    } else {
        cli.command
    };

    match command {
        Some(Commands::Install) => install_hooks(),
        Some(Commands::Uninstall) => uninstall_hooks(),
        Some(Commands::Menubar) => menubar::run(app_launch),
        Some(Commands::DashboardButton { visibility }) => {
            let enabled = visibility == "show";
            config::set_dashboard_button(enabled)?;
            println!("Dashboard {}.", if enabled { "shown" } else { "hidden" });
            Ok(())
        }
        Some(Commands::Led { port }) => led::run(port),
        Some(Commands::Flash(args)) => flash::run(args),
        Some(Commands::Ble { command }) => {
            #[cfg(target_os = "macos")]
            if matches!(
                command,
                ble::BleCommand::Scan | ble::BleCommand::Pair { .. }
            ) {
                // Discovery needs the same Info.plist/identity as the service.
                let bundled = mac_bundle_executable()?;
                if std::env::current_exe()? != bundled {
                    use std::os::unix::process::CommandExt;
                    anyhow::ensure!(
                        bundled.is_file(),
                        "Run `broglow install` before Bluetooth discovery"
                    );
                    return Err(std::process::Command::new(bundled)
                        .args(std::env::args_os().skip(1))
                        .exec()
                        .into());
                }
            }
            ble::command(command)
        }
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
    // On a machine where broglow isn't wired into Claude Code yet, treat the
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

// ----------------------------------------------------------------------------
// Install / uninstall
// ----------------------------------------------------------------------------

/// Claude Code lifecycle events that broglow's built-in hook backend
/// registers for. Single source of truth shared by install and uninstall.
const HOOK_EVENTS: [&str; 6] = [
    "SessionStart",
    "UserPromptSubmit",
    "Stop",
    "Notification",
    "SessionEnd",
    "PreToolUse",
];

/// Register broglow's native hook backend in ~/.claude/settings.json for every
/// lifecycle event, and ensure the data dir exists. Idempotent. Does *not*
/// touch autostart — `install_hooks` layers that on top; the menu-bar daemon's
/// first-run path deliberately calls only this (see `first_run_setup_daemon`).
fn register_hooks() -> anyhow::Result<()> {
    // 1. Ensure the broglow data directory exists. Its existence is also the
    //    "already set up" marker for the first-run paths.
    std::fs::create_dir_all(config::data_dir())?;

    // 2. Register the native hook backend in settings.json, detection-gated like
    //    every other adapter so a machine without Claude Code gets no ~/.claude.
    if harness::claude::detected() {
        register_claude_hooks()?;
    }

    // 3. Set up every detected non-Claude harness (opencode, Codex, Copilot).
    //    Best-effort and detection-gated per adapter: a failure never breaks
    //    Claude hook registration, and machines without those agents see
    //    nothing. Sits in register_hooks (not install_hooks) so both first-run
    //    paths — the TUI's full install and the daemon's hooks-only bootstrap —
    //    set them up; adapters touch no autostart, so this is safe from the
    //    daemon path.
    harness::install_all();

    Ok(())
}

fn register_claude_hooks() -> anyhow::Result<()> {
    // The hook command is this very binary invoked as `broglow hook`.
    let exe = std::env::current_exe().context("Resolving current executable path")?;
    let hook_cmd = format!("\"{}\" hook", exe.display());

    let settings_file = harness::claude::settings_path();
    let mut settings: serde_json::Value = if settings_file.exists() {
        let content = std::fs::read_to_string(&settings_file).context("Reading settings.json")?;
        serde_json::from_str(&content).context("Parsing settings.json")?
    } else {
        serde_json::json!({})
    };

    let hook_entry = serde_json::json!(
        {
            "hooks": [{
                "type": "command",
                "command": hook_cmd
            }]
        }
    );

    let hooks = settings
        .as_object_mut()
        .context("settings.json must be an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    let hooks_obj = hooks.as_object_mut().context("hooks must be an object")?;

    for event in HOOK_EVENTS {
        let groups = hooks_obj
            .entry(event)
            .or_insert_with(|| serde_json::json!([]))
            .as_array_mut()
            .with_context(|| format!("hooks.{event} must be an array"))?;
        codex::upsert_broglow_group(groups, &hook_entry);
    }

    codex::write_json_atomic(&settings_file, &settings)?;

    println!("Updated {}", settings_file.display());
    Ok(())
}

/// Temporary: adopts data from the previous layout. Remove in the release after next.
fn adopt_legacy_data_dir() {
    let Some(home) = dirs::home_dir() else {
        return;
    };
    let old = home.join(".claude").join("broglow");
    let new = config::data_dir();
    if old.is_dir() && !new.exists() {
        let _ = std::fs::rename(old, new);
    }
}

fn install_hooks() -> anyhow::Result<()> {
    adopt_legacy_data_dir();
    // 1./2. Register the hook backend in settings.json.
    register_hooks()?;

    // 3. If a status board is already plugged in on this first setup, turn the
    //    LEDs on automatically so the lamp lights up with no extra step.
    let lamp_enabled = maybe_autoenable_led();

    // 4. Install autostart for the tray daemon (platform-specific).
    install_autostart()?;

    println!(
        "\nInstallation complete! Hooks are now active for Claude Code and every detected coding agent (opencode, Codex, Copilot)."
    );
    println!("Run `broglow` to launch the TUI dashboard.");
    if !lamp_enabled {
        println!("Optional: enable a USB lamp with `l`, or use `broglow ble scan` to select a BLE lamp. See https://broglow.github.io/broglow/docs/hardware/.");
    }
    Ok(())
}

/// On a clean first install, enable the status LEDs automatically if an Espressif
/// USB candidate is already plugged in.
///
/// Gated on there being no `config.json` yet: a config file means the user has
/// already made a choice in the popover/TUI, so re-running install must never
/// flip the LEDs back on against a deliberate opt-out. Detection uses Espressif's
/// USB vendor ID (see `led::detect_board`), not a firmware identity check.
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
    let result = (|| -> anyhow::Result<(bool, bool)> {
        let _lock = config::acquire_config_lock()?;
        let mut cfg = config::read_config_strict()?;
        // Another settings writer may have created the file during detection.
        if config::config_file_path().exists() {
            return Ok((cfg.led_enabled, false));
        }
        cfg.led_enabled = true;
        config::write_config(&cfg)?;
        Ok((true, true))
    })();
    match result {
        Ok((enabled, created)) => {
            if created {
                println!("Detected an Espressif USB device at {port} — status LEDs enabled.");
            }
            enabled
        }
        Err(e) => {
            eprintln!("broglow: could not enable status LEDs: {e:#} (press `l` in the dashboard to enable them).");
            false
        }
    }
}

/// True once `register_hooks` has run on this machine: it creates the data
/// dir, and `uninstall` removes it. Gates the first-run auto-setup so a normal
/// launch (TUI or tray daemon) doesn't rewrite agent settings — or, on the TUI
/// path, re-bootstrap the LaunchAgent — every time. Harness-agnostic on
/// purpose: a Codex-only machine has no Claude settings to inspect.
fn already_set_up() -> bool {
    config::data_dir().exists()
}

/// First-run onboarding for the TUI. If broglow isn't set up on this machine
/// yet, run the full install (hooks + login autostart, which also brings up the
/// tray daemon) so that `brew install broglow && broglow` — or a plain
/// `cargo install` then launch — is a complete setup with no manual step.
///
/// Safe here because the TUI is a foreground process distinct from the daemon:
/// kickstarting the LaunchAgent spawns the tray we want, with nothing to collide
/// with on a fresh machine. Best-effort — a failure just leaves `broglow
/// install` as the manual fallback and never blocks the dashboard.
fn first_run_setup_tui() {
    if already_set_up() {
        return;
    }
    println!(
        "First run: registering hooks for Claude Code and every detected coding agent (opencode, Codex, Copilot), then starting the menu bar daemon…\n"
    );
    if let Err(e) = install_hooks() {
        eprintln!("broglow: first-run setup failed: {e:#}\nRun `broglow install` to finish setup.");
    }
}

/// First-run onboarding for the tray daemon. Ensure the hooks are registered so
/// sessions report status, but — unlike the TUI path — deliberately skip
/// autostart. The daemon must never bootstrap/kickstart its own LaunchAgent (or
/// spawn a detached `menubar` on Linux/Windows): it is already running, and
/// doing so would spin up a duplicate tray. Login autostart is established by
/// the TUI's first run or by `broglow install`.
pub fn first_run_setup_daemon() {
    if already_set_up() {
        return;
    }
    if let Err(e) = register_hooks() {
        eprintln!("broglow: first-run hook registration failed: {e:#}");
    }
}

fn uninstall_hooks() -> anyhow::Result<()> {
    // Hooks and harness wiring go first: they never depend on the daemon, and
    // a daemon that refuses to stop must not leave Claude Code calling a binary
    // that is about to be removed. The running tray re-registers hooks only at
    // its own startup (`first_run_setup_daemon`), so it cannot undo this.

    // Remove every harness's wiring (best-effort; each is marker-guarded so a
    // hand-rolled file is never deleted).
    harness::uninstall_all();

    // Remove hooks from settings.json
    let settings_file = harness::claude::settings_path();
    if settings_file.exists() {
        let content = std::fs::read_to_string(&settings_file)?;
        let mut settings: serde_json::Value = serde_json::from_str(&content)?;

        if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
            for event in HOOK_EVENTS {
                if let Some(groups) = hooks.get_mut(event).and_then(|v| v.as_array_mut()) {
                    // Claude trusts hooks by content, not position, so ours
                    // simply go; the positional placeholder is Codex-only.
                    groups.retain(|g| !codex::group_is_broglow(g));
                    if groups.is_empty() {
                        hooks.remove(event);
                    }
                }
            }
            if hooks.is_empty() {
                settings.as_object_mut().unwrap().remove("hooks");
            }
        }

        codex::write_json_atomic(&settings_file, &settings)?;
        println!("Removed hooks from {}", settings_file.display());
    }

    // Stop the daemon before deleting its bundle/state.
    uninstall_autostart()?;

    // Remove hook state and logs
    let dir = config::data_dir();
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
const RUN_VALUE: &str = "broglow";

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
    println!("Tray icon launched — look for the traffic-light icon in the system tray.");
    Ok(())
}

#[cfg(target_os = "windows")]
fn uninstall_run_key() -> anyhow::Result<()> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_ALL_ACCESS) {
        match key.delete_value(RUN_VALUE) {
            Ok(()) => println!("Removed tray autostart registry value."),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).context("Removing the tray autostart registry value"),
        }
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
pub(crate) const LAUNCH_AGENT_LABEL: &str = "io.github.broglow.menubar";
#[cfg(target_os = "macos")]
const INFO_PLIST: &str = "Contents/Info.plist";

#[cfg(target_os = "macos")]
const MAC_INFO_PLIST: &str = include_str!("../assets/macos-info.plist");

#[cfg(target_os = "macos")]
fn mac_app_launch(exe: &std::path::Path, stdin_is_terminal: bool) -> bool {
    !stdin_is_terminal && exe.ends_with("Contents/MacOS/broglow")
}

#[cfg(target_os = "macos")]
fn mac_bundle_path() -> anyhow::Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("No home directory")?
        .join("Applications/BroGlow.app"))
}

#[cfg(target_os = "macos")]
pub(crate) fn mac_bundle_executable() -> anyhow::Result<PathBuf> {
    Ok(mac_bundle_path()?.join("Contents/MacOS/broglow"))
}

#[cfg(target_os = "macos")]
fn check_managed_bundle(bundle: &std::path::Path) -> anyhow::Result<()> {
    if bundle.exists() {
        let info = std::fs::read_to_string(bundle.join(INFO_PLIST))?;
        anyhow::ensure!(
            info.contains("<key>BroGlowManaged</key><true/>")
                && info.contains(&format!("<string>{LAUNCH_AGENT_LABEL}</string>")),
            "{} is not a BroGlow-managed bundle; leaving it alone",
            bundle.display()
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn create_mac_bundle(
    exe: &std::path::Path,
    bundle: &std::path::Path,
    identity: &str,
) -> anyhow::Result<()> {
    std::fs::create_dir(bundle)?;
    std::fs::create_dir_all(bundle.join("Contents/MacOS"))?;
    std::fs::copy(exe, bundle.join("Contents/MacOS/broglow"))?;
    std::fs::create_dir_all(bundle.join("Contents/Resources"))?;
    std::fs::write(
        bundle.join("Contents/Resources/BroGlow.icns"),
        include_bytes!("../assets/icons/BroGlow.icns"),
    )?;
    std::fs::write(
        bundle.join(INFO_PLIST),
        MAC_INFO_PLIST.replace("{{VERSION}}", env!("CARGO_PKG_VERSION")),
    )?;
    // The tray flashes from inside the bundle, so give it the release images.
    if let Some(images) = flash::bundled_dir() {
        flash::copy_images(
            &images,
            &bundle.join("Contents/Resources/firmware/esp32-c3-supermini"),
        )?;
    }
    let mut sign = std::process::Command::new("/usr/bin/codesign");
    sign.args([
        "--force",
        "--sign",
        identity,
        "--identifier",
        LAUNCH_AGENT_LABEL,
    ]);
    if identity == "-" {
        // Local builds have no signing certificate. Keep their DR stable across
        // rebuilds; this identifies the app, not a trusted publisher. Distribution
        // should use BROGLOW_SIGNING_IDENTITY and its signer-bound default DR.
        sign.args([
            "--requirements",
            &format!("=designated => identifier \"{LAUNCH_AGENT_LABEL}\""),
        ]);
    }
    let out = sign.arg(bundle).output().context("Signing BroGlow.app")?;
    anyhow::ensure!(
        out.status.success(),
        "codesign failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    let out = std::process::Command::new("/usr/bin/codesign")
        .args(["--verify", "--strict"])
        .arg(bundle)
        .output()?;
    anyhow::ensure!(
        out.status.success(),
        "BroGlow.app signature verification failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    Ok(())
}

#[cfg(target_os = "macos")]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

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
    <string>{{DATA}}/menubar.log</string>
    <key>StandardErrorPath</key>
    <string>{{DATA}}/menubar.err</string>
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
    let bundle = mac_bundle_path()?;
    check_managed_bundle(&bundle)?;
    let applications = bundle.parent().context("Bundle must have a parent")?;
    std::fs::create_dir_all(applications)?;
    let staging = applications.join(format!(".BroGlow.{}.app", std::process::id()));
    let previous = applications.join(format!(".BroGlow.previous.{}.app", std::process::id()));
    anyhow::ensure!(
        !previous.exists(),
        "Recovery bundle already exists: {}",
        previous.display()
    );
    let plist_content = LAUNCH_AGENT_PLIST
        .replace("{{LABEL}}", LAUNCH_AGENT_LABEL)
        .replace(
            "{{BIN}}",
            &xml_escape(&mac_bundle_executable()?.display().to_string()),
        )
        .replace(
            "{{DATA}}",
            &xml_escape(&config::data_dir().display().to_string()),
        );

    let plist_path = launch_agent_path();
    if let Some(parent) = plist_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let uid = current_uid()?;
    let target = format!("gui/{uid}");
    let service = format!("gui/{uid}/{LAUNCH_AGENT_LABEL}");
    let lock = menubar::instance_lock_path();

    // Everything between creating the staging bundle and swapping it into
    // place can fail (codesign, a daemon that won't stop); never leave a
    // hidden .BroGlow.<pid>.app behind in ~/Applications when it does.
    let installed = (|| -> anyhow::Result<()> {
        create_mac_bundle(
            &exe,
            &staging,
            &std::env::var("BROGLOW_SIGNING_IDENTITY").unwrap_or_else(|_| "-".to_string()),
        )?;
        stop_launch_agent(&service)?;
        // A tray started by opening BroGlow.app is not the launchd job, so
        // bootout leaves it running and holding the lock the new job needs.
        stop_lock_holders(&lock)?;
        if bundle.exists() {
            std::fs::rename(&bundle, &previous)?;
        }
        std::fs::rename(&staging, &bundle).context("Installing BroGlow.app")?;

        std::fs::write(&plist_path, &plist_content)?;
        println!("Wrote LaunchAgent plist to {}", plist_path.display());

        let bootstrap = std::process::Command::new("launchctl")
            .args(["bootstrap", &target])
            .arg(&plist_path)
            .output()
            .context("Running `launchctl bootstrap`")?;
        if !bootstrap.status.success() {
            let stderr = String::from_utf8_lossy(&bootstrap.stderr);
            anyhow::bail!("launchctl bootstrap failed: {}", stderr.trim());
        }

        // RunAtLoad usually starts it already; no -k/redundant restart is needed.
        let start = std::process::Command::new("launchctl")
            .args(["kickstart", &service])
            .output()?;
        anyhow::ensure!(
            start.status.success(),
            "launchctl kickstart failed: {}",
            String::from_utf8_lossy(&start.stderr).trim()
        );
        // kickstart succeeds even when the tray exits at once; only a held
        // lock proves it came up.
        for _ in 0..40 {
            if !lock_holders(&lock)?.is_empty() {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }
        anyhow::bail!(
            "The BroGlow tray did not start; see {}",
            config::data_dir().join("menubar.err").display()
        )
    })();
    if let Err(error) = installed {
        let _ = std::fs::remove_dir_all(&staging);
        if previous.exists() {
            // The swap may already have happened: put the old app back and
            // reload the agent that `stop_launch_agent` booted out, so the
            // tray does not stay down until the next login.
            let _ = stop_launch_agent(&service);
            let _ = std::fs::remove_dir_all(&bundle);
            let _ = std::fs::rename(&previous, &bundle);
            if plist_path.exists() {
                let _ = std::process::Command::new("launchctl")
                    .args(["bootstrap", &target])
                    .arg(&plist_path)
                    .output();
            }
        }
        return Err(error);
    }

    if previous.exists() {
        std::fs::remove_dir_all(previous)?;
    }

    println!("LaunchAgent loaded from {} — allow BroGlow in Privacy & Security → Bluetooth when selecting a lamp.", bundle.display());
    Ok(())
}

#[cfg(target_os = "macos")]
fn stop_launch_agent(service: &str) -> anyhow::Result<()> {
    // A missing agent on first install is normal. Wait for an existing one to
    // unload before replacing its app or deleting the state directory.
    let _ = std::process::Command::new("launchctl")
        .args(["bootout", service])
        .output();
    for _ in 0..20 {
        if !std::process::Command::new("launchctl")
            .args(["print", service])
            .output()?
            .status
            .success()
        {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    anyhow::bail!("LaunchAgent did not stop: {service}; app and state were retained")
}

/// PIDs with the tray's instance lock open: every running tray, whether the
/// launchd job or one started by opening BroGlow.app.
#[cfg(target_os = "macos")]
fn lock_holders(lock: &std::path::Path) -> anyhow::Result<Vec<i32>> {
    // lsof exits 1 with no output when nothing (or no file) matches.
    let out = std::process::Command::new("/usr/sbin/lsof")
        .arg("-t")
        .arg(lock)
        .output()
        .context("Running `lsof`")?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .filter(|&pid| pid != std::process::id() as i32)
        .collect())
}

/// SIGTERM every tray still holding the lock and wait for it to let go.
#[cfg(target_os = "macos")]
fn stop_lock_holders(lock: &std::path::Path) -> anyhow::Result<()> {
    let holders = lock_holders(lock)?;
    for &pid in &holders {
        println!("Stopping a BroGlow tray outside the LaunchAgent (pid {pid})");
        unsafe { libc::kill(pid, libc::SIGTERM) };
    }
    for _ in 0..20 {
        if lock_holders(lock)?.is_empty() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    anyhow::bail!(
        "BroGlow tray {holders:?} did not quit; quit it from its menu and re-run this command"
    )
}

#[cfg(target_os = "macos")]
fn uninstall_launch_agent() -> anyhow::Result<()> {
    let service = format!("gui/{}/{LAUNCH_AGENT_LABEL}", current_uid()?);
    stop_launch_agent(&service)?;
    stop_lock_holders(&menubar::instance_lock_path())?;

    let plist_path = launch_agent_path();
    if plist_path.exists() {
        std::fs::remove_file(&plist_path)?;
        println!("Removed {}", plist_path.display());
    }
    let bundle = mac_bundle_path()?;
    check_managed_bundle(&bundle)?;
    if bundle.exists() {
        std::fs::remove_dir_all(&bundle)?;
        println!("Removed {}", bundle.display());
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
        .join("broglow.desktop")
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn install_xdg_autostart() -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("Resolving current executable path")?;

    let desktop_entry = format!(
        "[Desktop Entry]\nType=Application\nName=BroGlow\nExec=\"{}\" menubar\nX-GNOME-Autostart-enabled=true\n",
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

#[cfg(all(test, target_os = "macos"))]
mod mac_install_tests {
    use super::*;

    #[test]
    fn app_launch_uses_the_tray_but_terminal_launch_keeps_the_dashboard() {
        let bundled =
            std::path::Path::new("/Users/test/Applications/BroGlow.app/Contents/MacOS/broglow");
        assert!(mac_app_launch(bundled, false));
        assert!(!mac_app_launch(bundled, true));
        assert!(!mac_app_launch(
            std::path::Path::new("/usr/local/bin/broglow"),
            false
        ));
    }

    #[test]
    fn launch_agent_paths_escape_xml_metacharacters() {
        assert_eq!(
            xml_escape("/Users/A & B/<work>/\"quoted\"'s/broglow"),
            "/Users/A &amp; B/&lt;work&gt;/&quot;quoted&quot;&apos;s/broglow"
        );
    }

    #[test]
    fn a_tray_outside_launchd_holding_the_lock_is_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let lock = dir.path().join(crate::menubar::INSTANCE_LOCK_NAME);
        assert!(lock_holders(&lock).unwrap().is_empty());
        let mut tray = std::process::Command::new("/bin/sleep")
            .arg("30")
            .stdin(std::fs::File::create(&lock).unwrap())
            .spawn()
            .unwrap();
        assert_eq!(lock_holders(&lock).unwrap(), vec![tray.id() as i32]);
        stop_lock_holders(&lock).unwrap();
        assert!(!tray.wait().unwrap().success());
        assert!(lock_holders(&lock).unwrap().is_empty());
    }

    #[test]
    fn bundle_metadata_and_local_identity_survive_changed_executables() {
        let dir = tempfile::tempdir().unwrap();
        let mut requirements = Vec::new();
        for (i, source) in ["/bin/echo", "/bin/sleep"].iter().enumerate() {
            let bundle = dir.path().join(format!("BroGlow{i}.app"));
            create_mac_bundle(std::path::Path::new(source), &bundle, "-").unwrap();
            check_managed_bundle(&bundle).unwrap();
            let info = bundle.join(INFO_PLIST);
            assert!(std::process::Command::new("/usr/bin/plutil")
                .args(["-lint", "--"])
                .arg(&info)
                .output()
                .unwrap()
                .status
                .success());
            let info = std::fs::read_to_string(info).unwrap();
            assert!(info.contains("NSBluetoothAlwaysUsageDescription"));
            assert!(info.contains("<key>CFBundleExecutable</key><string>broglow</string>"));
            assert!(info.contains("<key>CFBundleIconFile</key><string>BroGlow.icns</string>"));
            assert_eq!(
                std::fs::read(bundle.join("Contents/Resources/BroGlow.icns")).unwrap(),
                include_bytes!("../assets/icons/BroGlow.icns")
            );
            let output = std::process::Command::new("/usr/bin/codesign")
                .args(["-d", "-r-"])
                .arg(&bundle)
                .output()
                .unwrap();
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            requirements.push(
                text.lines()
                    .find(|l| l.contains("designated =>"))
                    .unwrap()
                    .to_string(),
            );
        }
        assert_eq!(requirements[0], requirements[1]);
        assert!(requirements[0].contains(LAUNCH_AGENT_LABEL));
        assert!(!requirements[0].contains("cdhash"));
    }
}
