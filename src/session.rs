use std::path::Path;

use crate::state::{HookState, Status};

#[derive(Debug, Clone)]
pub struct DisplaySession {
    pub session_id: String,
    pub name: String,
    pub status: Status,
    pub project_name: String,
    pub project_path: String,
    pub last_updated: String,
    /// Where the session runs ("Claude app", "Ghostty"), when known.
    pub host: Option<crate::terminal::Host>,
    /// Which coding agent the session comes from: `None` for Claude Code (the
    /// default), `Some("opencode")` etc. Drives the per-harness badge in the UI.
    pub harness: Option<String>,
}

fn project_short_name(project_path: &str) -> String {
    Path::new(project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| project_path.to_string())
}

pub(crate) fn truncate(s: &str, max: usize) -> String {
    // Count by chars, not bytes: byte-slicing a multibyte UTF-8 sequence (emoji,
    // CJK, accented text, curly quotes) at a non-boundary index panics, which
    // would take the whole TUI down on the next refresh.
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let kept: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{kept}...")
    }
}

/// Char-safe first-N-chars prefix, used for the `Session <id-prefix>` fallback
/// name. Session IDs are ASCII UUIDs today, but slicing them by chars keeps the
/// helper reusable and immune to the non-boundary panic that byte-slicing hits.
fn char_prefix(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Every session in the hook state, as the TUI shows it. Names come from the
/// hooks (Claude's own titles, harness titles, or the auto-namer).
pub fn load_all_sessions(hook_state: &HookState) -> Vec<DisplaySession> {
    let mut result: Vec<DisplaySession> = hook_state
        .sessions
        .iter()
        .map(|(session_id, session_status)| {
            let project_path = session_status
                .project_path
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let name = session_status
                .name
                .clone()
                .unwrap_or_else(|| format!("Session {}", char_prefix(session_id, 8)));
            DisplaySession {
                session_id: session_id.clone(),
                name,
                status: session_status.status.clone(),
                project_name: project_short_name(&project_path),
                project_path,
                last_updated: session_status.last_updated.clone(),
                host: crate::terminal::host(
                    session_status.harness.as_deref(),
                    session_status.terminal.as_ref(),
                ),
                harness: session_status.harness.clone(),
            }
        })
        .collect();

    // Sort: needs_help first, then active, then done; within each by recency
    result.sort_by(|a, b| {
        let order = |s: &Status| match s {
            Status::NeedsHelp => 0,
            Status::Active => 1,
            Status::Inactive => 2,
            Status::Done => 3,
        };
        order(&a.status)
            .cmp(&order(&b.status))
            .then(b.last_updated.cmp(&a.last_updated))
    });

    // Keep only the 5 most recent done sessions
    let mut done_count = 0;
    result.retain(|s| {
        if s.status == Status::Done {
            done_count += 1;
            done_count <= 5
        } else {
            true
        }
    });

    result
}

#[cfg(test)]
mod tests {
    use super::{char_prefix, truncate};

    #[test]
    fn truncate_is_char_boundary_safe() {
        // A multibyte string longer than the limit whose byte cut lands inside a
        // char used to panic; it must now truncate cleanly on a char boundary.
        let s = "日本語のプロンプトでセッションを開始してくださいね、これはとても長いテスト文字列でありまして五十文字を超えています本当に超えていますよ";
        assert!(s.chars().count() > 50);
        let out = truncate(s, 50);
        assert!(out.ends_with("..."));
        assert!(out.chars().count() <= 50);
    }

    #[test]
    fn truncate_leaves_short_strings_untouched() {
        assert_eq!(truncate("hello", 50), "hello");
        assert_eq!(truncate("日本語", 50), "日本語");
    }

    #[test]
    fn char_prefix_never_splits_a_char() {
        assert_eq!(char_prefix("abcdefghij", 8), "abcdefgh");
        // Fewer chars than requested → returns the whole string, no panic.
        assert_eq!(char_prefix("日本", 8), "日本");
    }

    #[test]
    fn merge_carries_the_harness_tag_from_hook_state() {
        use crate::state::{HookState, SessionStatus, Status};

        let session = |name: &str, harness: Option<&str>| SessionStatus {
            status: Status::Active,
            last_updated: "2026-01-01T00:00:00Z".to_string(),
            project_path: Some("/tmp/proj".to_string()),
            notification_type: None,
            name: Some(name.to_string()),
            terminal: None,
            harness: harness.map(str::to_string),
        };

        let mut hook_state = HookState::default();
        hook_state.sessions.insert(
            "ses_oc".to_string(),
            session("Wire opencode", Some("opencode")),
        );
        hook_state
            .sessions
            .insert("cc_1".to_string(), session("Fix bug", None));

        let sessions = super::load_all_sessions(&hook_state);
        let oc = sessions.iter().find(|s| s.session_id == "ses_oc").unwrap();
        let cc = sessions.iter().find(|s| s.session_id == "cc_1").unwrap();
        assert_eq!(oc.harness.as_deref(), Some("opencode"));
        assert_eq!(cc.harness, None, "claude sessions carry no harness tag");
    }
}
