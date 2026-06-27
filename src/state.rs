use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HookState {
    pub sessions: HashMap<String, SessionStatus>,
}

impl Default for HookState {
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }
}

/// Aggregate health across all live sessions, in priority order:
/// any needs-help session wins, then any inactive, then any active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aggregate {
    Red,
    Yellow,
    Green,
    None,
}

pub fn aggregate(state: &HookState) -> Aggregate {
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
        Aggregate::Red
    } else if inactive > 0 {
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
    let content = std::fs::read_to_string(&path)?;
    let mut state: HookState = serde_json::from_str(&content).unwrap_or_default();
    if state.sessions.remove(session_id).is_none() {
        return Ok(());
    }

    // Atomic write: write to sibling temp file, then rename onto the target.
    let dir = path.parent().expect("state path must have a parent");
    let tmp_path = dir.join(format!(".state.{}.tmp", std::process::id()));
    let serialized = serde_json::to_string(&state)?;
    std::fs::write(&tmp_path, serialized)?;
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}
