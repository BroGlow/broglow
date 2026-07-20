//! Multi-harness adapter registry.
//!
//! clawlight mirrors more than one coding agent. Claude Code is the built-in
//! default — its sessions carry no `harness` tag and use the native hook
//! backend, so it has no entry here. Every *other* agent (opencode today;
//! Codex/Copilot planned) is one [`Adapter`]: its detection, its plugin/hook
//! install + uninstall, and its UI badge, all behind one small table.
//!
//! Everything harness-specific lives here, so adding the next harness is a
//! single [`ADAPTERS`] entry plus its embedded asset — not edits scattered
//! across `main.rs`, `session.rs`, and `hook.rs`.

use std::path::PathBuf;

/// One non-Claude harness clawlight knows how to set up and display.
pub struct Adapter {
    /// Value stored in [`SessionStatus.harness`](crate::state::SessionStatus)
    /// and sent in the normalized `harness` event field. Stable — it keys
    /// sessions across restarts and drives the reconnect sweep.
    pub name: &'static str,
    /// Short, **unique** tag shown in the TUI/popover (e.g. `"oc"`). Uniqueness
    /// is enforced by `adapter_badges_are_unique` below.
    pub badge: &'static str,
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

/// Every non-Claude harness clawlight supports. Add an entry (and its asset) to
/// support a new one — nothing else in the codebase needs to change.
pub const ADAPTERS: &[Adapter] = &[opencode::ADAPTER, codex::ADAPTER];

/// Set up every *detected* harness's wiring. Best-effort per adapter: a failure
/// to write one never blocks the others or the Claude hook registration.
pub fn install_all() {
    for a in ADAPTERS {
        if (a.detected)() {
            if let Err(e) = (a.install)() {
                eprintln!("clawlight: could not set up the {} adapter: {e:#}", a.name);
            }
        }
    }
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

/// Header line every clawlight-written harness file carries. `uninstall` only
/// deletes a file that still has it, so a user's hand-rolled file at the same
/// path is never removed. Shared across adapters.
const MANAGED_MARKER: &str = "managed by clawlight";

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

/// Whether a file exists at `path` that clawlight does not own (no
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

/// Delete a clawlight-managed file iff it still carries our marker (so a
/// hand-rolled file at the path is left alone). Shared uninstall helper.
fn remove_if_managed(path: &std::path::Path) {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };
    if !contents.contains(MANAGED_MARKER) {
        return;
    }
    if std::fs::remove_file(path).is_ok() {
        println!("Removed {}", path.display());
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
        config_dir().join("plugins").join("clawlight.js")
    }

    fn detected() -> bool {
        config_dir().exists() || super::is_on_path("opencode")
    }

    /// Write (or overwrite) the plugin with this binary's absolute path and
    /// version baked in. Idempotent: the unconditional overwrite of a *managed*
    /// plugin is also the version-skew fix — every `clawlight install` re-syncs
    /// it to the current binary. A file at the path *without* our marker is the
    /// user's own and is never touched, matching `uninstall`.
    fn install() -> anyhow::Result<()> {
        let path = plugin_path();
        if super::is_foreign(&path) {
            eprintln!(
                "clawlight: {} exists but isn't clawlight-managed; leaving it alone",
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
    /// The registered command is `clawlight codex-hook`, the shim that maps
    /// Codex's hook events onto the normalized verbs.
    fn install() -> anyhow::Result<()> {
        crate::codex::install_hooks()
    }

    fn uninstall() {
        crate::codex::uninstall_hooks();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_returns_registered_tags_and_a_fallback() {
        assert_eq!(badge("opencode"), "oc");
        assert_eq!(badge("codex"), "cx");
        // An unknown/future harness (e.g. a session written by a newer build)
        // still renders a non-empty tag rather than blank.
        assert_eq!(badge("mystery"), "my");
    }

    #[test]
    fn adapter_badges_are_unique() {
        // A collision would make two harnesses indistinguishable in the UI.
        // This guards the *exact* roadmap footgun: codex -> "co", copilot -> "co".
        let mut seen = std::collections::HashSet::new();
        for a in ADAPTERS {
            assert!(
                seen.insert(a.badge),
                "duplicate harness badge {:?} ({})",
                a.badge,
                a.name
            );
        }
    }
}
