mod app;
mod config;
mod led;
mod menubar;
mod notification;
mod ota;
mod session;
mod state;
mod ui;

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
#[command(name = "clawlight", about = "TUI dashboard for Claude Code sessions")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Install hooks into ~/.claude/settings.json and set up the hook script
    Install,
    /// Uninstall hooks and clean up
    Uninstall,
    /// Run the macOS menu bar daemon (foreground)
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
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Install) => install_hooks(),
        Some(Commands::Uninstall) => uninstall_hooks(),
        Some(Commands::Menubar) => menubar::run(),
        Some(Commands::Led { port }) => led::run(port),
        Some(Commands::Update { firmware, port }) => ota::run(firmware, port),
        None => run_tui(),
    }
}

fn run_tui() -> anyhow::Result<()> {
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

const HOOK_SCRIPT: &str = r#"#!/bin/bash
# clawlight hook script
# Invoked by Claude Code hooks to track session state.

STATE_DIR="$HOME/.claude/clawlight"
STATE_FILE="$STATE_DIR/state.json"

mkdir -p "$STATE_DIR"

# Read hook input from stdin
INPUT=$(cat)
HOOK_EVENT=$(echo "$INPUT" | jq -r '.hook_event_name // empty')
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // empty')
CWD=$(echo "$INPUT" | jq -r '.cwd // empty')
NOTIFICATION_TYPE=$(echo "$INPUT" | jq -r '.notification_type // empty')
TRANSCRIPT_PATH=$(echo "$INPUT" | jq -r '.transcript_path // empty')

# Skip if this is a meta session spawned by clawlight for auto-naming
if [ -n "$CLAWLIGHT_NAMING" ]; then
    exit 0
fi

if [ -z "$SESSION_ID" ]; then
    exit 0
fi

# Determine status based on hook event
case "$HOOK_EVENT" in
    "SessionStart"|"UserPromptSubmit"|"PreToolUse")
        STATUS="active"
        ;;
    "Stop")
        STATUS="inactive"
        ;;
    "Notification")
        # idle_prompt is an informational nudge, not a user-action-required signal.
        # Skip so the icon doesn't flip red on idle warnings.
        if [ "$NOTIFICATION_TYPE" = "idle_prompt" ]; then
            exit 0
        fi
        STATUS="needs_help"
        ;;
    "SessionEnd")
        STATUS="done"
        ;;
    *)
        exit 0
        ;;
esac

# For PreToolUse, skip the write if already active (fires very frequently)
if [ "$HOOK_EVENT" = "PreToolUse" ]; then
    if [ -f "$STATE_FILE" ]; then
        CURRENT=$(jq -r --arg sid "$SESSION_ID" '.sessions[$sid].status // empty' "$STATE_FILE" 2>/dev/null)
        if [ "$CURRENT" = "active" ]; then
            exit 0
        fi
    fi
fi

TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Initialize state file if it doesn't exist
if [ ! -f "$STATE_FILE" ]; then
    echo '{"sessions":{}}' > "$STATE_FILE"
fi

# Preserve existing name if present
EXISTING_NAME=$(jq -r --arg sid "$SESSION_ID" '.sessions[$sid].name // empty' "$STATE_FILE" 2>/dev/null)

# Atomic update: read, modify, write to temp, rename
TMPFILE=$(mktemp "$STATE_DIR/state.XXXXXX.json")
jq --arg sid "$SESSION_ID" \
   --arg status "$STATUS" \
   --arg ts "$TIMESTAMP" \
   --arg cwd "$CWD" \
   --arg ntype "$NOTIFICATION_TYPE" \
   --arg name "$EXISTING_NAME" \
   '.sessions[$sid] = {
       "status": $status,
       "last_updated": $ts,
       "project_path": $cwd,
       "notification_type": (if $ntype != "" then $ntype else null end),
       "name": (if $name != "" then $name else null end)
   }' "$STATE_FILE" > "$TMPFILE" && mv "$TMPFILE" "$STATE_FILE"

# Auto-name: on first Stop event, generate a name via claude CLI in background
if [ "$HOOK_EVENT" = "Stop" ] && [ -z "$EXISTING_NAME" ] && [ -n "$TRANSCRIPT_PATH" ]; then
    (
        # Unset to allow nested claude CLI usage from hook context
        unset CLAUDECODE

        # Extract the first user prompt from the transcript
        # content may be a plain string or an array of content blocks
        FIRST_PROMPT=$(jq -r '
            select(.message.role == "user")
            | .message.content
            | if type == "array" then
                map(select(.type == "text") | .text) | join(" ")
              else
                tostring
              end' "$TRANSCRIPT_PATH" 2>/dev/null \
            | head -c 500 \
            | head -1)

        if [ -z "$FIRST_PROMPT" ]; then
            exit 0
        fi

        # Generate name via claude CLI (haiku for speed/cost)
        # Export CLAWLIGHT_NAMING so hooks triggered by this call skip processing
        NAME=$(CLAWLIGHT_NAMING=1 claude -p --model haiku "Generate a concise 3-5 word title for this coding session. Output ONLY the title, nothing else. No quotes. User's request: $FIRST_PROMPT" 2>/dev/null)

        if [ -n "$NAME" ]; then
            # Write the name back to state.json
            TMPFILE2=$(mktemp "$STATE_DIR/state.XXXXXX.json")
            jq --arg sid "$SESSION_ID" \
               --arg name "$NAME" \
               '.sessions[$sid].name = $name' "$STATE_FILE" > "$TMPFILE2" && mv "$TMPFILE2" "$STATE_FILE"
        fi
    ) &
fi

exit 0
"#;

const LAUNCH_AGENT_LABEL: &str = "io.roush.clawlight.menubar";

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

fn hook_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Home directory must exist")
        .join(".claude")
        .join("clawlight")
}

fn hook_script_path() -> PathBuf {
    hook_dir().join("hook.sh")
}

fn launch_agent_path() -> PathBuf {
    dirs::home_dir()
        .expect("Home directory must exist")
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LAUNCH_AGENT_LABEL}.plist"))
}

fn current_uid() -> anyhow::Result<String> {
    let out = std::process::Command::new("id")
        .arg("-u")
        .output()
        .context("Running `id -u`")?;
    if !out.status.success() {
        anyhow::bail!("`id -u` failed");
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn settings_path() -> PathBuf {
    dirs::home_dir()
        .expect("Home directory must exist")
        .join(".claude")
        .join("settings.json")
}

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

fn install_hooks() -> anyhow::Result<()> {
    // 1. Create directory and write hook script
    let dir = hook_dir();
    std::fs::create_dir_all(&dir)?;

    let script_path = hook_script_path();
    std::fs::write(&script_path, HOOK_SCRIPT)?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o755))?;
    }

    println!("Wrote hook script to {}", script_path.display());

    // 2. Update settings.json
    let settings_file = settings_path();
    let mut settings: serde_json::Value = if settings_file.exists() {
        let content =
            std::fs::read_to_string(&settings_file).context("Reading settings.json")?;
        serde_json::from_str(&content).context("Parsing settings.json")?
    } else {
        serde_json::json!({})
    };

    let hook_cmd = format!("bash {}", script_path.display());

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

    for event in &["SessionStart", "UserPromptSubmit", "Stop", "Notification", "SessionEnd", "PreToolUse"] {
        hooks_obj.insert(event.to_string(), hook_entry.clone());
    }

    let settings_str = serde_json::to_string_pretty(&settings)?;
    std::fs::write(&settings_file, settings_str)?;

    println!("Updated {}", settings_file.display());

    // 3. Install the menu bar LaunchAgent
    install_launch_agent()?;

    println!("\nInstallation complete! Hooks are now active for new Claude Code sessions.");
    println!("Run `clawlight` to launch the TUI dashboard.");
    println!("Optional: plug in a Seeed XIAO ESP32-C6 status board and press `l` in the dashboard to enable status LEDs.");
    Ok(())
}

fn uninstall_hooks() -> anyhow::Result<()> {
    // Unload and remove the LaunchAgent first (best-effort)
    let _ = uninstall_launch_agent();

    // Remove hooks from settings.json
    let settings_file = settings_path();
    if settings_file.exists() {
        let content = std::fs::read_to_string(&settings_file)?;
        let mut settings: serde_json::Value = serde_json::from_str(&content)?;

        if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
            for event in &["SessionStart", "UserPromptSubmit", "Stop", "Notification", "SessionEnd", "PreToolUse"] {
                hooks.remove(*event);
            }
            if hooks.is_empty() {
                settings.as_object_mut().unwrap().remove("hooks");
            }
        }

        let settings_str = serde_json::to_string_pretty(&settings)?;
        std::fs::write(&settings_file, settings_str)?;
        println!("Removed hooks from {}", settings_file.display());
    }

    // Remove hook script and state
    let dir = hook_dir();
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
        println!("Removed {}", dir.display());
    }

    println!("\nUninstall complete.");
    Ok(())
}
