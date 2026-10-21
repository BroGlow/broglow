use std::collections::HashMap;
use std::fs::File;
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
    /// Where the session is running, captured by the hook backend from its
    /// own environment/process tree. Used for click-to-focus; absent in
    /// state files written before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal: Option<TerminalInfo>,
    /// Which coding agent produced this session: absent for Claude Code (the
    /// original harness and the default), `"opencode"` for opencode, etc.
    /// Written by `broglow event` from the normalized-event `harness` field;
    /// Claude hooks leave it `None`. Lets later UI badge sessions per harness
    /// and keeps the reconnect sweep able to target one harness's sessions.
    /// Absent in state files written before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    /// Remember app housekeeping across hook processes, including hooks with
    /// no prompt/transcript. Readers hide these Done tombstones.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub codex_background: bool,
}

impl SessionStatus {
    pub fn is_codex_background(&self) -> bool {
        self.harness.as_deref() == Some("codex") && self.codex_background
    }
}

/// Identity of the terminal window / host app running a session, recorded at
/// hook time (see `terminal::capture`). All fields are best-effort: whatever
/// the hosting environment exposes.
#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq, Eq)]
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
    /// CLAUDE_CODE_HOST_SESSION_ID: the Claude desktop app's own session ID
    /// ("local_…"), which its `claude://code/continue` link selects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_session_id: Option<String>,
    /// Controlling terminal device, e.g. "/dev/ttys012" (unix).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tty: Option<String>,
    /// PID of the Claude Code process hosting the session, captured at hook
    /// time (unix: the first ancestor that owns a real tty; Windows: the first
    /// ancestor above the wrapper shells — see `terminal::capture`). Lets the
    /// readers reap a session whose process exited without a SessionEnd hook
    /// (window closed, crash, SIGKILL) instead of leaving its status stuck.
    /// Absent where the process can't be identified (older state files, no
    /// controlling tty on unix, other platforms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_pid: Option<u32>,
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Ancestor {
    pub pid: u32,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct HookState {
    pub sessions: HashMap<String, SessionStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question_expiry: Option<QuestionExpiry>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub codex_questions: HashMap<String, crate::codex::PendingQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuestionExpiry {
    pub id: String,
    pub occurred_at: String,
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
        .join("broglow")
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

    if !state.codex_questions.is_empty() {
        // Stop can precede the terminal record; readers retry without a hook.
        let _ = crate::hook::update_state(crate::codex::reconcile_questions);
        if let Some(updated) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            state = updated;
        }
    }

    // Apply before both display-name refresh and aggregate/LED computation.
    // Keep the tombstones on disk so later hooks cannot resurrect these runs.
    state.sessions.retain(|_, s| !s.is_codex_background());
    reap_ended_sessions(&mut state, Utc::now(), crate::terminal::is_alive);
    state
}

/// Refresh titles when displaying sessions, even if a rename fires no hook.
/// Only the snapshot changes: names must not reset activity times or statuses.
/// Keep this out of the USB/BLE polling path, which needs statuses only.
pub fn read_display_state() -> HookState {
    let mut state = read_hook_state();
    if state
        .sessions
        .values()
        .any(|s| s.harness.as_deref() == Some("codex"))
    {
        let mut names = crate::codex::thread_names();
        for (id, session) in &mut state.sessions {
            if session.harness.as_deref() == Some("codex") {
                if let Some(name) = names.remove(id) {
                    session.name = Some(name);
                }
            }
        }
    }
    state
}

/// Downgrade sessions that have ended to `Done`, in memory, before the caller
/// aggregates or displays them. Two signals, cheapest first:
///
/// 1. **Process gone.** If we captured the session's Claude Code PID and that
///    process is no longer alive, the session is over — it exited without a
///    SessionEnd hook (terminal window closed, crash, SIGKILL), which would
///    otherwise leave a mid-`NeedsHelp` session stuck red. All three of those
///    look identical to us (no hook, dead process) and want the same outcome,
///    so the session is cleared as soon as we notice — the same result a clean
///    `/exit` reaches instantly via its SessionEnd hook. `is_alive` fails safe
///    toward "alive", so a session that is genuinely still running is never
///    cleared out from under the user.
/// 2. **Stale.** For sessions with no captured PID (older state files, or a
///    platform where the process couldn't be identified), fall back to the 24h
///    staleness backstop.
///
/// This mutates a snapshot only; it never writes `state.json`.
fn reap_ended_sessions(state: &mut HookState, now: DateTime<Utc>, is_alive: impl Fn(u32) -> bool) {
    for s in state.sessions.values_mut() {
        if s.status == Status::Done {
            continue;
        }
        if let Some(pid) = s.terminal.as_ref().and_then(|t| t.owner_pid) {
            if !is_alive(pid) {
                s.status = Status::Done;
            }
            // A live PID is authoritative — the session is still running, so
            // don't let the staleness backstop reap a long-idle-but-alive one.
            continue;
        }
        if let Ok(ts) = s.last_updated.parse::<DateTime<Utc>>() {
            if now.signed_duration_since(ts).num_hours() >= STALE_AFTER_HOURS {
                s.status = Status::Done;
            }
        }
    }
}

pub fn clear_session(session_id: &str) -> anyhow::Result<()> {
    clear_sessions_matching(|id, _| id == session_id)
}

#[cfg(any(target_os = "macos", target_os = "windows", test))]
pub fn clear_paused_sessions() -> anyhow::Result<()> {
    // Check under the state lock so a session that resumed since rendering survives.
    clear_sessions_matching(|_, session| session.status == Status::Inactive)
}

fn clear_sessions_matching(matches: impl Fn(&str, &SessionStatus) -> bool) -> anyhow::Result<()> {
    let path = state_file_path();
    if !path.exists() {
        return Ok(());
    }
    // Hold the same lock the hook backend uses so this read-modify-write can't
    // interleave with a concurrent hook and clobber its status update.
    let _lock =
        acquire_state_lock().ok_or_else(|| anyhow::anyhow!("Could not lock session state"))?;
    let content = std::fs::read_to_string(&path)?;
    let mut state: HookState = serde_json::from_str(&content)?;
    let count = state.sessions.len();
    state.sessions.retain(|id, session| !matches(id, session));
    state
        .codex_questions
        .retain(|id, _| state.sessions.contains_key(id));
    if state.sessions.len() == count {
        return Ok(());
    }

    write_state_atomic(&state)
}

/// Take a blocking exclusive lock on `.state.lock` beside `state.json`, guarding
/// a read-modify-write span against concurrent writers (hooks from other
/// sessions, the TUI's `clear_session`). The lock releases when the returned
/// `File` drops. Returns `None` if the lock can't be acquired (e.g. the dir
/// can't be created); callers must skip the write when locking fails.
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
    file.lock().ok()?;
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

    #[cfg(unix)]
    #[test]
    fn reader_reconciles_expiry_after_stop_and_restart() {
        if std::env::var_os("BROGLOW_TEST_EXPIRY").is_none() {
            let home = tempfile::tempdir().unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "state::tests::reader_reconciles_expiry_after_stop_and_restart",
                ])
                .env("HOME", home.path())
                .env("BROGLOW_TEST_EXPIRY", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            return;
        }
        let rollout = tempfile::NamedTempFile::new().unwrap();
        let mut state = state_with(&[Status::NeedsHelp, Status::NeedsHelp]);
        for s in state.sessions.values_mut() {
            s.last_updated = Utc::now().to_rfc3339();
        }
        let session = state.sessions.get_mut("s0").unwrap();
        session.harness = Some("codex".into());
        session.notification_type = Some("codex_question_async:call".into());
        state.codex_questions.insert(
            "s0".into(),
            crate::codex::PendingQuestion {
                turn_id: "new-turn".into(),
                call_id: "call".into(),
                transcript_path: rollout.path().to_string_lossy().into_owned(),
                offset: 0,
                accepted: true,
                exec: false,
                question_count: 1,
                answered: Vec::new(),
                discard_line: false,
            },
        );
        write_state_atomic(&state).unwrap();
        let old = serde_json::json!({"type":"event_msg", "timestamp": Utc::now().to_rfc3339(),
            "payload":{"type":"task_complete","turn_id":"old-turn"}})
        .to_string()
            + "\n";
        let terminal = serde_json::json!({"type":"event_msg", "timestamp": Utc::now().to_rfc3339(),
            "payload":{"type":"turn_aborted","turn_id":"new-turn"}})
        .to_string();
        std::fs::write(rollout.path(), &old).unwrap();
        assert_eq!(read_hook_state().sessions["s0"].status, Status::NeedsHelp);
        // A partially flushed line must be retried, not consumed as malformed.
        std::fs::write(rollout.path(), format!("{old}{terminal}")).unwrap();
        assert_eq!(read_hook_state().sessions["s0"].status, Status::NeedsHelp);
        std::fs::write(rollout.path(), format!("{old}{terminal}\n")).unwrap();
        let reconciled = read_hook_state();
        assert_eq!(reconciled.sessions["s0"].status, Status::Inactive);
        assert!(reconciled.codex_questions.is_empty());
        assert_eq!(
            reconciled.question_expiry.as_ref().unwrap().id,
            "s0:new-turn:call"
        );
        assert_eq!(
            aggregate(&reconciled, YellowMode::AnyInactive),
            Aggregate::Red
        );
        let disk = std::fs::read(state_file_path()).unwrap();
        assert_eq!(
            read_hook_state().question_expiry,
            reconciled.question_expiry
        );
        assert_eq!(std::fs::read(state_file_path()).unwrap(), disk);
        std::fs::write(state_file_path(), "broken state").unwrap();
        read_hook_state();
        assert_eq!(
            std::fs::read_to_string(state_file_path()).unwrap(),
            "broken state"
        );
    }

    #[cfg(unix)]
    #[test]
    fn app_generated_codex_rows_do_not_reach_the_display_or_lamp() {
        if std::env::var_os("BROGLOW_TEST_BACKGROUND").is_none() {
            let home = tempfile::tempdir().unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "state::tests::app_generated_codex_rows_do_not_reach_the_display_or_lamp",
                ])
                .env("HOME", home.path())
                .env("CODEX_HOME", home.path().join("codex"))
                .env("BROGLOW_TEST_BACKGROUND", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            return;
        }

        let mut original = state_with(&[Status::Active]);
        original.sessions.get_mut("s0").unwrap().last_updated = Utc::now().to_rfc3339();
        for (id, flagged, harness) in [
            ("ordinary-title", false, "codex"),
            ("remembered", true, "codex"),
            ("other-agent", false, "opencode"),
        ] {
            let mut row = original.sessions["s0"].clone();
            row.harness = Some(harness.into());
            row.codex_background = flagged;
            row.name = Some(if flagged {
                "A later title".into()
            } else {
                "This is an app-generated request for a suggeste...".into()
            });
            row.status = if flagged {
                Status::Inactive
            } else {
                Status::Active
            };
            original.sessions.insert(id.into(), row);
        }
        write_state_atomic(&original).unwrap();
        let disk = std::fs::read(state_file_path()).unwrap();
        for snapshot in [read_hook_state(), read_display_state()] {
            assert_eq!(snapshot.sessions.len(), 3);
            assert!(snapshot.sessions.contains_key("s0"));
            assert!(snapshot.sessions.contains_key("other-agent"));
            assert!(snapshot.sessions.contains_key("ordinary-title"));
            assert_eq!(
                aggregate(&snapshot, YellowMode::AnyInactive),
                Aggregate::Green
            );
            assert!(crate::session::load_all_sessions(&snapshot, true)
                .iter()
                .all(|s| s.session_id != "remembered"));
        }
        assert_eq!(std::fs::read(state_file_path()).unwrap(), disk);
    }

    #[cfg(unix)]
    #[test]
    fn codex_renames_refresh_without_hooks() {
        // Follow the existing child-process test pattern: never change the runner's HOME.
        if std::env::var_os("BROGLOW_TEST_RENAME").is_none() {
            let home = tempfile::tempdir().unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "state::tests::codex_renames_refresh_without_hooks",
                ])
                .env("HOME", home.path())
                .env("CODEX_HOME", home.path().join("codex"))
                .env("BROGLOW_TEST_RENAME", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            return;
        }

        let mut original = HookState::default();
        for (id, status, name, harness) in [
            ("unnamed", Status::Active, None, Some("codex")),
            ("paused", Status::Inactive, Some("Old title"), Some("codex")),
            (
                "approval",
                Status::NeedsHelp,
                Some("First prompt"),
                Some("codex"),
            ),
            ("other", Status::Active, Some("Claude title"), None),
        ] {
            original.sessions.insert(
                id.into(),
                SessionStatus {
                    status,
                    last_updated: Utc::now().to_rfc3339(),
                    project_path: Some("/project".into()),
                    notification_type: None,
                    name: name.map(str::to_string),
                    terminal: None,
                    harness: harness.map(str::to_string),
                    codex_background: false,
                },
            );
        }
        write_state_atomic(&original).unwrap();
        let disk = std::fs::read(state_file_path()).unwrap();
        let index = crate::codex::codex_home()
            .unwrap()
            .join("session_index.jsonl");
        std::fs::create_dir_all(index.parent().unwrap()).unwrap();
        assert_eq!(
            read_display_state().sessions["paused"].name.as_deref(),
            Some("Old title")
        );

        let mut entries = String::new();
        for title in ["Generated name", "Renamed 日本語 🦀"] {
            for id in ["unnamed", "paused", "approval", "other", "dismissed"] {
                entries += &serde_json::json!({"id": id, "thread_name": title}).to_string();
                entries.push('\n');
            }
            entries += "not json\n{\"id\":\"paused\",\"thread_name\":\"  \"}\n";
            std::fs::write(&index, &entries).unwrap();
            let refreshed = read_display_state();
            let mut expected = original.clone();
            for id in ["unnamed", "paused", "approval"] {
                expected.sessions.get_mut(id).unwrap().name = Some(title.into());
                assert_eq!(
                    crate::menubar::display_name(id, &refreshed.sessions[id]),
                    title
                );
                assert_eq!(
                    crate::session::load_all_sessions(&refreshed, false)
                        .iter()
                        .find(|s| s.session_id == id)
                        .unwrap()
                        .name,
                    title
                );
            }
            assert_eq!(
                serde_json::to_value(refreshed).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
            assert_eq!(std::fs::read(state_file_path()).unwrap(), disk);
        }
        std::fs::remove_file(index).unwrap();
        assert_eq!(
            serde_json::to_value(read_display_state()).unwrap(),
            serde_json::to_value(original).unwrap()
        );
    }

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
                    harness: None,
                    codex_background: false,
                },
            );
        }
        state
    }

    #[test]
    fn aggregate_prioritizes_help_and_respects_the_selected_idle_policy() {
        use Aggregate::{Green, None, Red, Yellow};
        use Status::{Active, Done, Inactive, NeedsHelp};

        let cases: &[(&str, &[Status], Aggregate, Aggregate)] = &[
            ("empty", &[], None, None),
            ("working", &[Active, Done], Green, Green),
            ("paused", &[Inactive, Done], Yellow, Yellow),
            ("mixed", &[Active, Inactive], Yellow, Green),
            ("needs help", &[Active, Inactive, NeedsHelp], Red, Red),
            ("finished", &[Done], None, None),
        ];
        for (name, statuses, any_inactive, active_wins) in cases {
            let state = state_with(statuses);

            assert_eq!(
                aggregate(&state, YellowMode::AnyInactive),
                *any_inactive,
                "{name}"
            );
            assert_eq!(
                aggregate(&state, YellowMode::ActiveWins),
                *active_wins,
                "{name}"
            );
        }
    }

    /// Build a single-session state carrying `owner_pid`, whose last hook event
    /// was `last_updated`. Lets a test place the session inside or outside the
    /// reap-grace / staleness windows.
    fn state_with_owner(
        status: Status,
        owner_pid: Option<u32>,
        last_updated: DateTime<Utc>,
    ) -> HookState {
        let mut state = HookState::default();
        state.sessions.insert(
            "s".to_string(),
            SessionStatus {
                status,
                last_updated: last_updated.to_rfc3339(),
                project_path: None,
                notification_type: None,
                name: None,
                terminal: Some(TerminalInfo {
                    owner_pid,
                    ..Default::default()
                }),
                harness: None,
                codex_background: false,
            },
        );
        state
    }

    fn status_of(state: &HookState) -> Status {
        state.sessions["s"].status.clone()
    }

    #[test]
    fn a_dead_process_is_reaped_immediately() {
        let now = Utc::now();
        // Even with a just-now last event, a dead process is cleared at once —
        // a crash / kill / closed window has no SessionEnd to wait for.
        let mut state = state_with_owner(Status::NeedsHelp, Some(4242), now);
        reap_ended_sessions(&mut state, now, |_| false);
        assert_eq!(status_of(&state), Status::Done);
    }

    #[test]
    fn a_live_process_keeps_needs_help_no_matter_how_old() {
        let now = Utc::now();
        // A live PID must win even past the staleness window: the session is
        // genuinely still waiting for the user.
        let mut state = state_with_owner(
            Status::NeedsHelp,
            Some(4242),
            now - chrono::Duration::hours(STALE_AFTER_HOURS + 1),
        );
        reap_ended_sessions(&mut state, now, |_| true);
        assert_eq!(status_of(&state), Status::NeedsHelp);
    }

    #[test]
    fn without_a_pid_the_24h_staleness_backstop_still_applies() {
        let now = Utc::now();
        // Old timestamp, no owner_pid → the staleness path reaps it.
        let mut stale = state_with_owner(
            Status::NeedsHelp,
            None,
            now - chrono::Duration::hours(STALE_AFTER_HOURS + 1),
        );
        reap_ended_sessions(&mut stale, now, |_| unreachable!("no pid to probe"));
        assert_eq!(status_of(&stale), Status::Done);

        // Fresh timestamp, no owner_pid → left alone.
        let mut fresh = state_with_owner(Status::NeedsHelp, None, now);
        reap_ended_sessions(&mut fresh, now, |_| unreachable!("no pid to probe"));
        assert_eq!(status_of(&fresh), Status::NeedsHelp);
    }

    #[test]
    fn a_dead_process_is_not_probed_for_already_done_sessions() {
        let now = Utc::now();
        let mut state = state_with_owner(Status::Done, Some(4242), now);
        reap_ended_sessions(&mut state, now, |_| unreachable!("Done is skipped"));
        assert_eq!(status_of(&state), Status::Done);
    }
}
