use std::collections::HashMap;

use crate::state::{HookState, Status};

/// Shown wherever a session's working directory is unknown.
pub const NO_PROJECT: &str = "No project";

#[derive(Debug, Clone)]
pub struct DisplaySession {
    pub session_id: String,
    pub name: String,
    pub status: Status,
    /// A failed Codex turn that needs a different model or a retry.
    pub capacity_blocked: bool,
    /// The project label from [`project_labels`]: the folder name, with
    /// parent folders added only where two shown sessions' folders share a
    /// name; [`NO_PROJECT`] when the path is unknown.
    pub project_name: String,
    /// The working directory the hook recorded; empty when unknown.
    pub project_path: String,
    pub last_updated: String,
    /// Where the session runs ("Claude app", "Ghostty"), when known.
    pub host: Option<crate::terminal::Host>,
    /// Which coding agent the session comes from: `None` for Claude Code (the
    /// default), `Some("opencode")` etc. Drives the per-harness badge in the UI.
    pub harness: Option<String>,
}

/// Path components, ignoring empty ones (a trailing or doubled separator)
/// and accepting either separator so Windows paths label like unix ones.
fn components(path: &str) -> Vec<&str> {
    path.split(['/', '\\']).filter(|c| !c.is_empty()).collect()
}

/// The last `depth` components joined with `/`; the path itself when it has
/// no components (`/`).
fn suffix(path: &str, parts: &[&str], depth: usize) -> String {
    if parts.is_empty() {
        path.to_string()
    } else {
        parts[parts.len() - depth.min(parts.len())..].join("/")
    }
}

/// Display labels for project paths: the folder name, extended with parent
/// folders only as far as needed to tell same-named folders apart
/// (`arcadia/recco` vs `arcadia-wt/recco`, deeper if the parents match too).
/// Two paths whose components are identical (`/a/x` and `/a/x/`) fall back to
/// the path itself. Compute it over the paths a surface actually shows, so a
/// hidden session never lengthens a visible label.
pub fn project_labels<'a>(paths: impl IntoIterator<Item = &'a str>) -> HashMap<String, String> {
    let mut paths: Vec<&str> = paths.into_iter().collect();
    paths.sort_unstable();
    paths.dedup();
    let parts: Vec<Vec<&str>> = paths.iter().map(|p| components(p)).collect();
    let mut depth = vec![1usize; paths.len()];
    loop {
        let labels: Vec<String> = (0..paths.len())
            .map(|i| suffix(paths[i], &parts[i], depth[i]))
            .collect();
        let mut by_label: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, label) in labels.iter().enumerate() {
            by_label.entry(label).or_default().push(i);
        }
        let clashing: Vec<usize> = by_label
            .values()
            .filter(|group| group.len() > 1)
            .flatten()
            .copied()
            .collect();
        let growable: Vec<usize> = clashing
            .iter()
            .copied()
            .filter(|&i| depth[i] < parts[i].len())
            .collect();
        if growable.is_empty() {
            let mut out: HashMap<String, String> = paths
                .iter()
                .zip(labels)
                .map(|(p, label)| (p.to_string(), label))
                .collect();
            // Still identical at full depth: nothing shorter than the path
            // itself tells them apart.
            for i in clashing {
                out.insert(paths[i].to_string(), paths[i].to_string());
            }
            return out;
        }
        for i in growable {
            depth[i] += 1;
        }
    }
}

fn status_rank(status: &Status) -> u8 {
    match status {
        Status::NeedsHelp => 0,
        Status::Active => 1,
        Status::Inactive => 2,
        Status::Done => 3,
    }
}

/// Reorder a status-sorted list into project groups: groups ranked by their
/// most urgent session, then that session's recency (newest first), then the
/// path for a stable order; sessions without a known directory last. The
/// stable sort keeps the status-then-recency order inside each group. The
/// popover's JS applies the same keys.
pub fn group_by_project(sessions: &mut [DisplaySession]) {
    let mut best: HashMap<&str, (u8, &str)> = HashMap::new();
    for s in sessions.iter() {
        let key = (status_rank(&s.status), s.last_updated.as_str());
        let entry = best.entry(&s.project_path).or_insert(key);
        if (key.0, std::cmp::Reverse(key.1)) < (entry.0, std::cmp::Reverse(entry.1)) {
            *entry = key;
        }
    }
    let best: HashMap<String, (u8, String)> = best
        .into_iter()
        .map(|(path, (rank, at))| (path.to_string(), (rank, at.to_string())))
        .collect();
    sessions.sort_by_cached_key(|s| {
        let (rank, at) = &best[&s.project_path];
        (
            s.project_path.is_empty(),
            *rank,
            std::cmp::Reverse(at.clone()),
            s.project_path.clone(),
        )
    });
}

/// Give every row the label [`project_labels`] computes over exactly these
/// rows (so a surface that hides some rows relabels the rest), or
/// [`NO_PROJECT`].
pub fn relabel(sessions: &mut [DisplaySession]) {
    let labels = project_labels(
        sessions
            .iter()
            .map(|s| s.project_path.as_str())
            .filter(|p| !p.is_empty()),
    );
    for s in sessions {
        s.project_name = labels
            .get(&s.project_path)
            .cloned()
            .unwrap_or_else(|| NO_PROJECT.to_string());
    }
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
/// hooks (Claude's own titles, harness titles, or the auto-namer). With
/// `group_by_project` the rows come in project groups (see
/// [`group_by_project`]); otherwise needs-help first, then working, paused
/// and done, newest first within each.
pub fn load_all_sessions(hook_state: &HookState, grouped: bool) -> Vec<DisplaySession> {
    let mut result: Vec<DisplaySession> = hook_state
        .sessions
        .iter()
        .map(|(session_id, session_status)| {
            let name = session_status
                .name
                .clone()
                .unwrap_or_else(|| format!("Session {}", char_prefix(session_id, 8)));
            DisplaySession {
                session_id: session_id.clone(),
                name,
                status: session_status.status.clone(),
                capacity_blocked: session_status.is_codex_capacity(),
                project_name: String::new(),
                project_path: session_status.project_path.clone().unwrap_or_default(),
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
        status_rank(&a.status)
            .cmp(&status_rank(&b.status))
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

    // Labels over the rows that survived the cap, so a dropped done session
    // never forces a longer label on a visible one.
    relabel(&mut result);

    if grouped {
        group_by_project(&mut result);
    }
    result
}

#[cfg(test)]
mod tests {
    use crate::state::{HookState, SessionStatus, Status};

    fn stored_session(status: Status, at: &str, path: Option<&str>) -> SessionStatus {
        SessionStatus {
            status,
            last_updated: at.into(),
            project_path: path.map(str::to_string),
            notification_type: None,
            name: None,
            terminal: None,
            harness: None,
            codex_background: false,
        }
    }

    #[test]
    fn session_rows_keep_agent_identity_and_use_unicode_safe_fallback_names() {
        let mut state = HookState::default();
        state.sessions.insert(
            "ses_oc".into(),
            SessionStatus {
                name: Some("Wire opencode".into()),
                harness: Some("opencode".into()),
                ..stored_session(Status::Active, "2026-01-01T00:00:00Z", Some("/tmp/proj"))
            },
        );
        let unnamed_id = "日本語日本語日本語";
        state.sessions.insert(
            unnamed_id.into(),
            stored_session(Status::Active, "2026-01-01T00:00:00Z", None),
        );

        let sessions = super::load_all_sessions(&state, false);

        let oc = sessions.iter().find(|s| s.session_id == "ses_oc").unwrap();
        let cc = sessions
            .iter()
            .find(|s| s.session_id == unnamed_id)
            .unwrap();
        assert_eq!(oc.harness.as_deref(), Some("opencode"));
        assert_eq!(oc.name, "Wire opencode");
        assert_eq!(cc.harness, None);
        assert_eq!(cc.name, "Session 日本語日本語日本");
    }

    #[test]
    fn session_rows_expose_capacity_only_while_the_codex_failure_needs_help() {
        let mut state = HookState::default();
        state.sessions.insert(
            "capacity".into(),
            SessionStatus {
                harness: Some("codex".into()),
                notification_type: Some("codex_capacity".into()),
                ..stored_session(Status::NeedsHelp, "2026-01-01T00:00:00Z", None)
            },
        );
        assert!(super::load_all_sessions(&state, false)[0].capacity_blocked);
        state.sessions.get_mut("capacity").unwrap().status = Status::Active;
        assert!(!super::load_all_sessions(&state, false)[0].capacity_blocked);
    }

    #[test]
    fn project_labels_grow_only_until_same_named_folders_differ() {
        let labels = super::project_labels([
            "/Users/x/Documents/broglow",
            "/Users/x/arcadia/recco",
            "/Users/x/arcadia-wt/recco/",
            "C:\\Users\\x\\repo",
            "/",
        ]);
        assert_eq!(labels["/Users/x/Documents/broglow"], "broglow");
        assert_eq!(labels["/Users/x/arcadia/recco"], "arcadia/recco");
        assert_eq!(labels["/Users/x/arcadia-wt/recco/"], "arcadia-wt/recco");
        assert_eq!(labels["C:\\Users\\x\\repo"], "repo");
        assert_eq!(labels["/"], "/");

        // Parents match too: keep climbing until something differs.
        let labels = super::project_labels(["/w1/proj/repo", "/w2/proj/repo"]);
        assert_eq!(labels["/w1/proj/repo"], "w1/proj/repo");
        assert_eq!(labels["/w2/proj/repo"], "w2/proj/repo");

        // Identical components: only the path itself tells them apart.
        let labels = super::project_labels(["/a/x", "/a/x/"]);
        assert_eq!(labels["/a/x"], "/a/x");
        assert_eq!(labels["/a/x/"], "/a/x/");
        assert!(super::project_labels([]).is_empty());
    }

    fn fixture() -> HookState {
        let mut state = HookState::default();
        let mut add = |id: &str, s: SessionStatus| state.sessions.insert(id.into(), s);
        add(
            "help",
            stored_session(Status::NeedsHelp, "2026-01-01T10:03:00Z", Some("/a/recco")),
        );
        add(
            "review",
            stored_session(Status::Active, "2026-01-01T10:05:00Z", Some("/a/recco")),
        );
        add(
            "grouping",
            stored_session(Status::Active, "2026-01-01T10:06:00Z", Some("/d/broglow")),
        );
        add(
            "focus",
            stored_session(Status::Inactive, "2026-01-01T09:49:00Z", Some("/d/broglow")),
        );
        add(
            "deploy",
            stored_session(Status::Inactive, "2026-01-01T09:24:00Z", Some("/b/recco")),
        );
        add(
            "done",
            stored_session(Status::Done, "2026-01-01T09:00:00Z", Some("/b/recco")),
        );
        add(
            "notes",
            stored_session(Status::Inactive, "2026-01-01T08:00:00Z", None),
        );
        state
    }

    #[test]
    fn ungrouped_order_is_status_then_recency_with_shared_labels() {
        let sessions = super::load_all_sessions(&fixture(), false);
        let ids: Vec<&str> = sessions.iter().map(|s| s.session_id.as_str()).collect();
        assert_eq!(
            ids,
            ["help", "grouping", "review", "focus", "deploy", "notes", "done"]
        );
        let by_id = |id: &str| sessions.iter().find(|s| s.session_id == id).unwrap();
        assert_eq!(by_id("help").project_name, "a/recco");
        assert_eq!(by_id("deploy").project_name, "b/recco");
        assert_eq!(by_id("grouping").project_name, "broglow");
        assert_eq!(by_id("notes").project_name, super::NO_PROJECT);
        assert_eq!(by_id("notes").project_path, "");
    }

    #[test]
    fn grouped_order_ranks_projects_by_urgency_then_recency_with_no_project_last() {
        let sessions = super::load_all_sessions(&fixture(), true);
        let ids: Vec<&str> = sessions.iter().map(|s| s.session_id.as_str()).collect();
        // /a/recco (needs help) → /d/broglow (working, 10:06) → /b/recco
        // (paused; its done row stays inside the group) → no project.
        assert_eq!(
            ids,
            ["help", "review", "grouping", "focus", "deploy", "done", "notes"]
        );
    }

    #[test]
    fn grouped_order_puts_the_fresher_of_two_equally_urgent_projects_first() {
        let mut state = HookState::default();
        // Path order (/a before /z) is the opposite of recency, so only the
        // recency key can put /z first.
        state.sessions.insert(
            "a".into(),
            stored_session(Status::Active, "2026-01-01T10:00:00Z", Some("/a")),
        );
        state.sessions.insert(
            "z".into(),
            stored_session(Status::Active, "2026-01-01T11:00:00Z", Some("/z")),
        );
        let paths: Vec<String> = super::load_all_sessions(&state, true)
            .into_iter()
            .map(|s| s.project_path)
            .collect();
        assert_eq!(paths, ["/z", "/a"]);
    }

    #[test]
    fn labels_ignore_done_sessions_dropped_by_the_cap() {
        let mut state = HookState::default();
        state.sessions.insert(
            "live".into(),
            stored_session(Status::Active, "2026-01-01T12:00:00Z", Some("/y/repo")),
        );
        // Five newer done rows survive the cap; the sixth, older one at a
        // same-named folder is dropped and must not lengthen the live label.
        for i in 0..5 {
            state.sessions.insert(
                format!("done{i}"),
                stored_session(Status::Done, "2026-01-01T11:00:00Z", Some("/d/other")),
            );
        }
        state.sessions.insert(
            "stale".into(),
            stored_session(Status::Done, "2026-01-01T01:00:00Z", Some("/x/repo")),
        );
        let sessions = super::load_all_sessions(&state, false);
        assert!(sessions.iter().all(|s| s.session_id != "stale"));
        let live = sessions.iter().find(|s| s.session_id == "live").unwrap();
        assert_eq!(live.project_name, "repo");
    }

    #[test]
    fn relabel_recomputes_over_the_rows_given() {
        let mut sessions = super::load_all_sessions(&fixture(), false);
        assert_eq!(sessions[0].project_name, "a/recco", "help row");
        sessions.retain(|s| s.project_path != "/b/recco");
        super::relabel(&mut sessions);
        assert_eq!(sessions[0].project_name, "recco");
        let notes = sessions.iter().find(|s| s.session_id == "notes").unwrap();
        assert_eq!(notes.project_name, super::NO_PROJECT);
    }

    #[test]
    fn grouped_order_breaks_recency_ties_by_path() {
        let mut state = HookState::default();
        state.sessions.insert(
            "z".into(),
            stored_session(Status::Active, "2026-01-01T10:00:00Z", Some("/z")),
        );
        state.sessions.insert(
            "m".into(),
            stored_session(Status::Active, "2026-01-01T10:00:00Z", Some("/m")),
        );
        state.sessions.insert(
            "a".into(),
            stored_session(Status::Active, "2026-01-01T10:00:00Z", Some("/a")),
        );
        let paths: Vec<String> = super::load_all_sessions(&state, true)
            .into_iter()
            .map(|s| s.project_path)
            .collect();
        assert_eq!(paths, ["/a", "/m", "/z"]);
    }
}
