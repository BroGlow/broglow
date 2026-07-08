use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use fs4::fs_std::FileExt;
use serde::{Deserialize, Serialize};

const STALE_AFTER_HOURS: i64 = 24;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Active,
    Inactive,
    NeedsHelp,
    Done,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Active => write!(f, "active"),
            Status::Inactive => write!(f, "inactive"),
            Status::NeedsHelp => write!(f, "needs help"),
            Status::Done => write!(f, "done"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionStatus {
    pub status: Status,
    pub last_updated: String,
    pub project_path: Option<String>,
    pub notification_type: Option<String>,
    pub name: Option<String>,
    /// Where the session is running, captured by the hook backend from its
    /// own environment/process tree. Used for click-to-focus; absent in
    /// state files written before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<TerminalInfo>,
}

/// Identity of the terminal window / host app running a session, recorded at
/// hook time (see `terminal::capture`). All fields are best-effort: whatever
/// the hosting environment exposes.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct TerminalInfo {
    /// TERM_PROGRAM: "Apple_Terminal", "iTerm.app", "vscode", "ghostty", …
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term_program: Option<String>,
    /// TERM_SESSION_ID (Terminal.app per-tab UUID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub term_session_id: Option<String>,
    /// ITERM_SESSION_ID ("w0t2p0:UUID").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iterm_session_id: Option<String>,
    /// __CFBundleIdentifier of the hosting .app (macOS): com.apple.Terminal,
    /// com.microsoft.VSCode, com.anthropic.claudefordesktop, …
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    /// Controlling terminal device, e.g. "/dev/ttys012" (unix).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tty: Option<String>,
    /// WT_SESSION (Windows Terminal tab GUID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wt_session: Option<String>,
    /// KITTY_WINDOW_ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kitty_window_id: Option<String>,
    /// WEZTERM_PANE.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wezterm_pane: Option<String>,
    /// TMUX_PANE, only when TMUX is also set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tmux_pane: Option<String>,
    /// Ancestor process chain (nearest first) on Windows/Linux, where window
    /// focus goes through the owning process rather than scripting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ancestors: Option<Vec<Ancestor>>,
}

/// One entry of the captured ancestor process chain. The name is re-checked
/// at focus time so a reused PID is never brought to the front.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Ancestor {
    pub pid: u32,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HookState {
    pub sessions: HashMap<String, SessionStatus>,
}

/// Aggregate health across all live sessions. Any needs-help session always
/// wins (red); how inactive vs. active resolve is the user's
/// [`YellowMode`](crate::config::YellowMode) setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aggregate {
    Red,
    Yellow,
    Green,
    None,
}

pub fn aggregate(state: &HookState, yellow_mode: crate::config::YellowMode) -> Aggregate {
    use crate::config::YellowMode;

    let mut needs_help = 0;
    let mut active = 0;
    let mut inactive = 0;
    for s in state.sessions.values() {
        match s.status {
            Status::NeedsHelp => needs_help += 1,
            Status::Active => active += 1,
            Status::Inactive => inactive += 1,
            Status::Done => {}
        }
    }
    if needs_help > 0 {
        return Aggregate::Red;
    }
    let yellow_first = match yellow_mode {
        YellowMode::AnyInactive => true,
        YellowMode::ActiveWins => false,
    };
    if inactive > 0 && (yellow_first || active == 0) {
        Aggregate::Yellow
    } else if active > 0 {
        Aggregate::Green
    } else {
        Aggregate::None
    }
}

pub fn state_file_path() -> PathBuf {
    dirs::home_dir()
        .expect("Home directory must exist")
        .join(".claude")
        .join("clawlight")
        .join("state.json")
}

pub fn read_hook_state() -> HookState {
    let path = state_file_path();
    if !path.exists() {
        return HookState::default();
    }

    let mut state: HookState = std::fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str(&content).ok())
        .unwrap_or_default();

    // Downgrade sessions to Done if they haven't been touched in 24h. Catches
    // orphans from Claude crashes or kills that skipped the SessionEnd hook.
    let now = Utc::now();
    for s in state.sessions.values_mut() {
        if s.status == Status::Done {
            continue;
        }
        if let Ok(ts) = s.last_updated.parse::<DateTime<Utc>>() {
            if now.signed_duration_since(ts).num_hours() >= STALE_AFTER_HOURS {
                s.status = Status::Done;
            }
        }
    }
    state
}

pub fn clear_session(session_id: &str) -> anyhow::Result<()> {
    let path = state_file_path();
    if !path.exists() {
        return Ok(());
    }
    // Hold the same lock the hook backend uses so this read-modify-write can't
    // interleave with a concurrent hook and clobber its status update.
    let _lock = acquire_state_lock();
    let content = std::fs::read_to_string(&path)?;
    let mut state: HookState = serde_json::from_str(&content).unwrap_or_default();
    if state.sessions.remove(session_id).is_none() {
        return Ok(());
    }

    write_state_atomic(&state)
}

/// Take a blocking exclusive lock on `.state.lock` beside `state.json`, guarding
/// a read-modify-write span against concurrent writers (hooks from other
/// sessions, the TUI's `clear_session`). The lock releases when the returned
/// `File` drops. Returns `None` if the lock can't be acquired (e.g. the dir
/// can't be created); callers proceed unlocked rather than break over this.
pub fn acquire_state_lock() -> Option<File> {
    let path = state_file_path();
    let dir = path.parent()?;
    std::fs::create_dir_all(dir).ok()?;
    let lock_path = dir.join(".state.lock");
    let file = File::options()
        .create(true)
        .write(true)
        // The lock file is only ever empty; keep (non-)contents as-is.
        .truncate(false)
        .open(&lock_path)
        .ok()?;
    file.lock_exclusive().ok()?;
    Some(file)
}

/// Atomic write: serialize to a sibling temp file, then rename onto the
/// target. The temp name is PID-scoped so concurrent writers never collide
/// on the same temp path.
pub fn write_state_atomic(state: &HookState) -> anyhow::Result<()> {
    let path = state_file_path();
    let dir = path.parent().expect("state path must have a parent");
    std::fs::create_dir_all(dir)?;
    let tmp_path = dir.join(format!(".state.{}.tmp", std::process::id()));
    let serialized = serde_json::to_string(state)?;
    std::fs::write(&tmp_path, serialized)?;
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::YellowMode;

    fn state_with(statuses: &[Status]) -> HookState {
        let mut state = HookState::default();
        for (i, status) in statuses.iter().enumerate() {
            state.sessions.insert(
                format!("s{i}"),
                SessionStatus {
                    status: status.clone(),
                    last_updated: String::new(),
                    project_path: None,
                    notification_type: None,
                    name: None,
                    terminal: None,
                },
            );
        }
        state
    }

    #[test]
    fn any_inactive_shows_yellow_over_active() {
        let mixed = state_with(&[Status::Active, Status::Inactive]);
        assert_eq!(
            aggregate(&mixed, YellowMode::AnyInactive),
            Aggregate::Yellow
        );
    }

    #[test]
    fn active_wins_stays_green_while_anything_works() {
        let mixed = state_with(&[Status::Active, Status::Inactive]);
        assert_eq!(aggregate(&mixed, YellowMode::ActiveWins), Aggregate::Green);

        // With no active sessions left, both modes agree on yellow.
        let idle_only = state_with(&[Status::Inactive, Status::Done]);
        assert_eq!(
            aggregate(&idle_only, YellowMode::ActiveWins),
            Aggregate::Yellow
        );
        assert_eq!(
            aggregate(&idle_only, YellowMode::AnyInactive),
            Aggregate::Yellow
        );
    }

    #[test]
    fn needs_help_is_red_in_both_modes() {
        let help = state_with(&[Status::Active, Status::Inactive, Status::NeedsHelp]);
        assert_eq!(aggregate(&help, YellowMode::AnyInactive), Aggregate::Red);
        assert_eq!(aggregate(&help, YellowMode::ActiveWins), Aggregate::Red);
    }

    #[test]
    fn done_only_is_none() {
        let done = state_with(&[Status::Done]);
        assert_eq!(aggregate(&done, YellowMode::AnyInactive), Aggregate::None);
        assert_eq!(aggregate(&done, YellowMode::ActiveWins), Aggregate::None);
    }
}
