//! Multi-harness adapter registry.
//!
//! broglow mirrors more than one coding agent. Claude Code is the built-in
//! default — its sessions carry no `harness` tag and use the native hook
//! backend, so it has no entry in the table; its paths and detection live in
//! [`claude`] and its UI identity is [`CLAUDE_LABEL`] and [`CLAUDE_ICON`].
//! Every *other* agent (opencode, Codex CLI, GitHub Copilot CLI) is one
//! [`Adapter`]: its detection, its plugin/hook install + uninstall, and its UI
//! badge, label and icon, all behind one small table.
//!
//! Everything harness-specific lives here, so adding the next harness is a
//! single [`ADAPTERS`] entry plus its embedded asset — not edits scattered
//! across `main.rs`, `session.rs`, and `hook.rs`.

use std::path::PathBuf;

/// One non-Claude harness broglow knows how to set up and display.
pub struct Adapter {
    /// Value stored in [`SessionStatus.harness`](crate::state::SessionStatus)
    /// and sent in the normalized `harness` event field. Stable — it keys
    /// sessions across restarts and drives the reconnect sweep.
    pub name: &'static str,
    /// Short, **unique** tag shown in the TUI/popover (e.g. `"oc"`). Uniqueness
    /// is enforced by `adapter_badges_are_unique` below.
    pub badge: &'static str,
    /// Agent name shown beside the icon in the popover (e.g. `"Codex"`).
    pub label: &'static str,
    /// Our own geometric glyph for the popover, never a vendor logo: SVG path
    /// data for a 16×16 stroked `currentColor` icon. Unique, like `badge`.
    pub icon: &'static str,
    /// Whether this harness looks installed on the machine — gates writing its
    /// wiring so we stay silent otherwise.
    pub detected: fn() -> bool,
    /// Write/refresh this harness's plugin or hook wiring. Idempotent and
    /// best-effort; only called when `detected()` is true.
    pub install: fn() -> anyhow::Result<()>,
    /// Remove what `install` wrote — only files that still carry our managed-by
    /// marker, so a user's hand-rolled file is never deleted.
    pub uninstall: fn(),
}

/// Claude Code's own files. Every `~/.claude` path broglow reads or writes is
/// built here, and nothing is written there unless Claude looks installed.
pub mod claude {
    use std::path::PathBuf;

    pub fn home() -> PathBuf {
        dirs::home_dir()
            .expect("Home directory must exist")
            .join(".claude")
    }

    /// `settings.json`, where the native hook backend is registered.
    pub fn settings_path() -> PathBuf {
        home().join("settings.json")
    }

    /// Transcripts: `projects/<encoded working dir>/<session id>.jsonl`.
    pub fn projects_dir() -> PathBuf {
        home().join("projects")
    }

    /// OAuth credentials, read only for opt-in usage limits.
    pub fn credentials_path() -> PathBuf {
        home().join(".credentials.json")
    }

    pub fn detected() -> bool {
        home().exists() || super::is_on_path("claude")
    }
}

/// Every non-Claude harness broglow supports. Add an entry (and its asset) to
/// support a new one — nothing else in the codebase needs to change.
pub const ADAPTERS: &[Adapter] = &[opencode::ADAPTER, codex::ADAPTER, copilot::ADAPTER];

/// Set up every *detected* harness's wiring. Best-effort per adapter: a failure
/// to write one never blocks the others or the Claude hook registration.
/// Returns the labels of the harnesses actually wired up.
pub fn install_all() -> Vec<&'static str> {
    let mut installed = Vec::new();
    for a in ADAPTERS {
        if (a.detected)() {
            match (a.install)() {
                Ok(()) => installed.push(a.label),
                Err(e) => eprintln!("broglow: could not set up the {} adapter: {e:#}", a.name),
            }
        }
    }
    installed
}

/// Remove every harness's wiring. Each `uninstall` is marker-guarded, so this is
/// safe to call unconditionally.
pub fn uninstall_all() {
    for a in ADAPTERS {
        (a.uninstall)();
    }
}

/// The UI badge for a harness name: the adapter's explicit tag, or a two-char
/// fallback for an unknown/future harness so a session never renders blank.
/// Registered harnesses should always have an explicit, unique `badge`.
pub fn badge(name: &str) -> String {
    ADAPTERS
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.badge.to_string())
        .unwrap_or_else(|| name.chars().take(2).collect())
}

/// Claude Code's popover label; it's the built-in default, so no [`Adapter`].
pub const CLAUDE_LABEL: &str = "Claude";
/// Claude Code's popover glyph: an eight-ray spark.
pub const CLAUDE_ICON: &str = "M8 1.5v13M1.5 8h13M3.4 3.4l9.2 9.2M12.6 3.4l-9.2 9.2";
/// Claude Code's tag on text-only surfaces (TUI, Linux tray menu).
pub const CLAUDE_BADGE: &str = "cc";

/// How the popover identifies a session's coding agent.
#[derive(Debug, PartialEq, Eq)]
pub struct Agent {
    pub label: String,
    /// `None` for an unknown/future harness, which shows `badge` text instead.
    pub icon: Option<&'static str>,
    pub badge: String,
}

impl Agent {
    /// The popover's meta prefix for text-only surfaces, with the badge
    /// standing in for the icon: `"[cx] Codex · "`, or `"[cx] "` with names off.
    /// Only the Linux tray menu needs it; macOS and Windows use the popover.
    #[cfg(any(test, not(any(target_os = "macos", target_os = "windows"))))]
    pub fn text_prefix(&self, show_name: bool) -> String {
        if show_name {
            format!("[{}] {} · ", self.badge, self.label)
        } else {
            format!("[{}] ", self.badge)
        }
    }
}

/// The agent behind a session's `harness` tag (`None` = Claude Code).
pub fn agent(harness: Option<&str>) -> Agent {
    let Some(name) = harness else {
        return Agent {
            label: CLAUDE_LABEL.to_string(),
            icon: Some(CLAUDE_ICON),
            badge: CLAUDE_BADGE.to_string(),
        };
    };
    match ADAPTERS.iter().find(|a| a.name == name) {
        Some(a) => Agent {
            label: a.label.to_string(),
            icon: Some(a.icon),
            badge: a.badge.to_string(),
        },
        None => Agent {
            label: name.to_string(),
            icon: None,
            badge: badge(name),
        },
    }
}

/// Header line every broglow-written harness file carries. `uninstall` only
/// deletes a file that still has it, so a user's hand-rolled file at the same
/// path is never removed. Shared across adapters.
const MANAGED_MARKER: &str = "managed by broglow";

/// Whether `program` resolves to a file on the current `PATH`. On Windows also
/// tries the common executable extensions. Shared detection helper.
fn is_on_path(program: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    let candidates = |dir: &std::path::Path| -> Vec<PathBuf> {
        let base = dir.join(program);
        #[cfg(windows)]
        {
            let mut v = vec![base];
            for ext in ["exe", "cmd", "bat"] {
                v.push(dir.join(format!("{program}.{ext}")));
            }
            v
        }
        #[cfg(not(windows))]
        {
            vec![base]
        }
    };
    std::env::split_paths(&paths)
        .flat_map(|dir| candidates(&dir))
        .any(|p| p.is_file())
}

/// Escape a string for embedding inside a JS double-quoted literal — a baked
/// binary path can contain backslashes (Windows) or, in principle, quotes.
fn js_string_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Whether a file exists at `path` that broglow does not own (no
/// [`MANAGED_MARKER`]). `install` must refuse to overwrite such a file — it's
/// the user's hand-rolled version, mirroring how `uninstall` refuses to delete
/// it. An unreadable file counts as foreign: if the marker can't be verified,
/// overwriting isn't safe. Shared install guard.
fn is_foreign(path: &std::path::Path) -> bool {
    if !path.exists() {
        return false;
    }
    std::fs::read_to_string(path)
        .map(|c| !c.contains(MANAGED_MARKER))
        .unwrap_or(true)
}

/// Delete a broglow-managed file iff it still carries our marker (so a
/// hand-rolled file at the path is left alone). Shared uninstall helper.
fn remove_if_managed(path: &std::path::Path) {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };
    if !contents.contains(MANAGED_MARKER) {
        return;
    }
    match std::fs::remove_file(path) {
        Ok(()) => println!("Removed {}", path.display()),
        Err(e) => eprintln!("broglow: could not remove {}: {e}", path.display()),
    }
}

// ---------------------------------------------------------------------------
// opencode
// ---------------------------------------------------------------------------

pub mod opencode {
    use anyhow::Context;
    use std::path::PathBuf;

    pub const ADAPTER: super::Adapter = super::Adapter {
        name: "opencode",
        badge: "oc",
        label: "opencode",
        // Square brackets.
        icon: "M5.5 2.5H3v11h2.5M10.5 2.5H13v11h-2.5",
        detected,
        install,
        uninstall,
    };

    /// The opencode plugin, embedded at build time and written at install with
    /// the version + this binary's absolute path substituted in.
    const PLUGIN_TEMPLATE: &str = include_str!("../assets/opencode-plugin.js");

    /// opencode's global config directory (`~/.config/opencode`). Its existence
    /// is one of the two opencode-present signals (the other is the binary on
    /// PATH).
    ///
    /// Note: `~/.config/opencode` is assumed on every platform. opencode's
    /// Windows config location (`%APPDATA%`?) is unconfirmed, so detection and
    /// the plugin path are effectively untested there — verify against a real
    /// Windows opencode before trusting it (the tests are unix-gated).
    fn config_dir() -> PathBuf {
        dirs::home_dir()
            .expect("Home directory must exist")
            .join(".config")
            .join("opencode")
    }

    /// Where the plugin is written. opencode auto-loads modules from the global
    /// `plugins/` directory (plural — confirmed against a live opencode; the
    /// singular `plugin/` is silently ignored).
    fn plugin_path() -> PathBuf {
        config_dir().join("plugins").join("broglow.js")
    }

    fn detected() -> bool {
        config_dir().exists() || super::is_on_path("opencode")
    }

    /// Write (or overwrite) the plugin with this binary's absolute path and
    /// version baked in. Idempotent: the unconditional overwrite of a *managed*
    /// plugin is also the version-skew fix — every `broglow install` re-syncs
    /// it to the current binary. A file at the path *without* our marker is the
    /// user's own and is never touched, matching `uninstall`.
    fn install() -> anyhow::Result<()> {
        let path = plugin_path();
        if super::is_foreign(&path) {
            eprintln!(
                "broglow: {} exists but isn't broglow-managed; leaving it alone",
                path.display()
            );
            return Ok(());
        }

        let exe = std::env::current_exe().context("Resolving current executable path")?;
        let contents = PLUGIN_TEMPLATE
            .replace("{{VERSION}}", env!("CARGO_PKG_VERSION"))
            .replace(
                "{{BIN}}",
                &super::js_string_escape(&exe.display().to_string()),
            );

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("Creating opencode plugin dir")?;
        }
        std::fs::write(&path, contents).context("Writing opencode plugin")?;
        println!("Wrote opencode plugin to {}", path.display());
        println!("  (restart any running opencode sessions to pick it up)");
        Ok(())
    }

    fn uninstall() {
        super::remove_if_managed(&plugin_path());
    }
}

// ---------------------------------------------------------------------------
// codex
// ---------------------------------------------------------------------------

pub mod codex {
    use std::path::PathBuf;

    pub const ADAPTER: super::Adapter = super::Adapter {
        name: "codex",
        badge: "cx", // not "co": reserved against the codex/copilot collision
        label: "Codex",
        // Hexagon.
        icon: "M8 1.5l5.6 3.25v6.5L8 14.5l-5.6-3.25v-6.5z",
        detected,
        install,
        uninstall,
    };

    fn detected() -> bool {
        crate::codex::codex_home().is_some_and(|p: PathBuf| p.exists())
            || super::is_on_path("codex")
    }

    /// Codex speaks a Claude-compatible hooks dialect, so its wiring is JSON
    /// matcher groups merged into `$CODEX_HOME/hooks.json` (a file shared
    /// with other tools — not a marker-owned file like the opencode plugin;
    /// the command-string predicate in codex.rs is the ownership marker).
    /// The registered command is `broglow codex-hook`, the shim that maps
    /// Codex's hook events onto the normalized verbs.
    fn install() -> anyhow::Result<()> {
        crate::codex::install_hooks()
    }

    fn uninstall() {
        crate::codex::uninstall_hooks();
    }
}

// ---------------------------------------------------------------------------
// copilot
// ---------------------------------------------------------------------------

pub mod copilot {
    pub const ADAPTER: super::Adapter = super::Adapter {
        name: "copilot",
        badge: "co", // codex holds "cx", deliberately leaving "co" to copilot
        label: "Copilot",
        // Goggles: two rings and a bridge.
        icon: "M1.5 8.5a2.75 2.75 0 1 0 5.5 0a2.75 2.75 0 1 0-5.5 0\
               M9 8.5a2.75 2.75 0 1 0 5.5 0a2.75 2.75 0 1 0-5.5 0M7 8.5h2",
        detected,
        install,
        uninstall,
    };

    fn detected() -> bool {
        crate::copilot::copilot_home().is_some_and(|p| p.exists()) || super::is_on_path("copilot")
    }

    /// Copilot CLI loads user-level hook files from `$COPILOT_HOME/hooks/`
    /// (default `~/.copilot/hooks/`), so the wiring is a whole file broglow
    /// owns — `broglow.json` — registering `broglow copilot-hook <event>`
    /// per lifecycle event (Copilot payloads don't name their event, so it
    /// rides on argv). Ownership is the `copilot-hook` command string in the
    /// file, not the comment marker: JSON has no comments. The shim maps each
    /// event onto the normalized verbs — see copilot.rs and
    /// `hook::run_copilot_hook`.
    fn install() -> anyhow::Result<()> {
        crate::copilot::install_hooks()
    }

    fn uninstall() {
        crate::copilot::uninstall_hooks();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_identifies_claude_registered_and_unknown_harnesses() {
        let claude = agent(None);
        assert_eq!(claude.label, "Claude");
        assert_eq!(claude.icon, Some(CLAUDE_ICON));
        assert_eq!(claude.badge, "cc");

        let codex = agent(Some("codex"));
        assert_eq!(codex.label, "Codex");
        assert_eq!(codex.icon, Some(codex::ADAPTER.icon));

        // An unknown harness has no glyph of ours; the page shows its badge.
        let unknown = agent(Some("mystery"));
        assert_eq!(unknown.label, "mystery");
        assert_eq!(unknown.icon, None);
        assert_eq!(unknown.badge, "my");
    }

    #[test]
    fn text_prefix_mirrors_the_popover_meta_line() {
        assert_eq!(agent(Some("codex")).text_prefix(true), "[cx] Codex · ");
        assert_eq!(agent(Some("codex")).text_prefix(false), "[cx] ");
        assert_eq!(agent(None).text_prefix(true), "[cc] Claude · ");
    }

    #[test]
    fn registered_agents_are_distinguishable_and_icons_are_attribute_safe() {
        let mut badges = std::collections::HashSet::from([CLAUDE_BADGE]);
        let mut labels = std::collections::HashSet::from([CLAUDE_LABEL]);
        let mut icons = std::collections::HashSet::from([CLAUDE_ICON]);
        for a in ADAPTERS {
            assert!(badges.insert(a.badge), "duplicate badge for {}", a.name);
            assert!(labels.insert(a.label), "duplicate label {:?}", a.label);
            assert!(icons.insert(a.icon), "duplicate icon for {}", a.name);
        }
        // Path data only: the page places it in an SVG `d` attribute.
        for icon in icons {
            assert!(!icon.is_empty());
            assert!(
                icon.chars()
                    .all(|c| c.is_ascii_alphanumeric() || " .-,".contains(c)),
                "icon must be plain path data: {icon:?}"
            );
        }
    }
}
