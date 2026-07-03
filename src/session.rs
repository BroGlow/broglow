use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Deserialize;

use crate::state::{HookState, Status};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsIndex {
    pub entries: Vec<SessionEntry>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SessionEntry {
    pub session_id: String,
    pub first_prompt: Option<String>,
    pub summary: Option<String>,
    pub custom_title: Option<String>,
    pub message_count: Option<u32>,
    pub created: Option<String>,
    pub modified: Option<String>,
    pub git_branch: Option<String>,
    pub project_path: Option<String>,
    pub is_sidechain: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct DisplaySession {
    pub session_id: String,
    pub name: String,
    pub status: Status,
    pub project_name: String,
    pub project_path: String,
    pub git_branch: Option<String>,
    pub message_count: u32,
    pub modified: String,
}

pub fn discover_projects() -> anyhow::Result<Vec<PathBuf>> {
    let claude_dir = dirs::home_dir()
        .context("No home directory")?
        .join(".claude")
        .join("projects");

    if !claude_dir.exists() {
        return Ok(Vec::new());
    }

    let mut projects = Vec::new();
    for entry in std::fs::read_dir(&claude_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() && path.join("sessions-index.json").exists() {
            projects.push(path);
        }
    }
    Ok(projects)
}

pub fn read_sessions_index(project_dir: &Path) -> anyhow::Result<SessionsIndex> {
    let index_path = project_dir.join("sessions-index.json");
    let content = std::fs::read_to_string(&index_path)
        .with_context(|| format!("Reading {}", index_path.display()))?;
    let index: SessionsIndex = serde_json::from_str(&content)
        .with_context(|| format!("Parsing {}", index_path.display()))?;
    Ok(index)
}

fn project_short_name(project_path: &str) -> String {
    Path::new(project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| project_path.to_string())
}

fn truncate(s: &str, max: usize) -> String {
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

pub fn load_all_sessions(hook_state: &HookState) -> Vec<DisplaySession> {
    let projects = discover_projects().unwrap_or_default();
    let mut all_entries: Vec<(String, SessionEntry)> = Vec::new();

    for project_dir in &projects {
        let dir_name = project_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if let Ok(index) = read_sessions_index(project_dir) {
            for entry in index.entries {
                all_entries.push((dir_name.clone(), entry));
            }
        }
    }

    merge_sessions(all_entries, hook_state)
}

fn merge_sessions(
    all_entries: Vec<(String, SessionEntry)>,
    hook_state: &HookState,
) -> Vec<DisplaySession> {
    let mut result: Vec<DisplaySession> = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    for (_dir_name, entry) in &all_entries {
        let status = hook_state
            .sessions
            .get(&entry.session_id)
            .map(|s| s.status.clone())
            .unwrap_or(Status::Done);

        // Priority: clawlight LLM name > customTitle > summary > firstPrompt
        let clawlight_name = hook_state
            .sessions
            .get(&entry.session_id)
            .and_then(|s| s.name.clone());

        let name = clawlight_name
            .or_else(|| entry.custom_title.clone())
            .or_else(|| entry.summary.clone())
            .or_else(|| entry.first_prompt.as_ref().map(|p| truncate(p, 50)))
            .unwrap_or_else(|| format!("Session {}", char_prefix(&entry.session_id, 8)));

        let project_path = entry.project_path.clone().unwrap_or_default();

        result.push(DisplaySession {
            session_id: entry.session_id.clone(),
            name,
            status,
            project_name: project_short_name(&project_path),
            project_path,
            git_branch: entry.git_branch.clone(),
            message_count: entry.message_count.unwrap_or(0),
            modified: entry.modified.clone().unwrap_or_default(),
        });

        seen_ids.insert(entry.session_id.clone());
    }

    // Add sessions from hook state that aren't in any sessions-index
    for (session_id, session_status) in &hook_state.sessions {
        if !seen_ids.contains(session_id) {
            let project_path = session_status
                .project_path
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let name = session_status
                .name
                .clone()
                .unwrap_or_else(|| format!("Session {}", char_prefix(session_id, 8)));
            result.push(DisplaySession {
                session_id: session_id.clone(),
                name,
                status: session_status.status.clone(),
                project_name: project_short_name(&project_path),
                project_path,
                git_branch: None,
                message_count: 0,
                modified: session_status.last_updated.clone(),
            });
        }
    }

    // Sort: needs_help first, then active, then done; within each by modified desc
    result.sort_by(|a, b| {
        let order = |s: &Status| match s {
            Status::NeedsHelp => 0,
            Status::Active => 1,
            Status::Inactive => 2,
            Status::Done => 3,
        };
        order(&a.status)
            .cmp(&order(&b.status))
            .then(b.modified.cmp(&a.modified))
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
}
