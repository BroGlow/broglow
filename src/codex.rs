//! Codex CLI specifics, behind the `codex` harness adapter (see harness.rs).
//!
//! Codex (>= 0.144) speaks a Claude-compatible hooks dialect: command hooks in
//! `$CODEX_HOME/hooks.json` receive the same stdin JSON shape as Claude Code's
//! (`hook_event_name`, `session_id`, `cwd`, `transcript_path`). The adapter
//! registers `broglow codex-hook` for eight events; that shim (hook.rs) maps
//! them onto the normalized harness verbs. This module holds everything else
//! Codex-specific: where its home lives, the non-destructive hooks.json
//! merge, thread names from `session_index.jsonl`, exec-vs-interactive from
//! rollout metadata, and first-prompt extraction for fallback naming.
//!
//! Two Codex facts shape the adapter:
//! - **Permission requests can be automatically reviewed.** The current
//!   rollout turn context distinguishes automatic review from human approval.
//!   Older versions lack `Interrupt`/`SessionEnd`; process reaping and one-shot
//!   `Stop` handling remain their backstops (see [`rollout_is_exec`]).
//! - **Codex trusts hooks by content hash + position** (`[hooks.state]` in
//!   its config.toml, keys like `hooks.json:<event>:<group>:<hook>`). New or
//!   changed entries need a one-time `/hooks` approval inside Codex, and
//!   neither the merge nor the removal below may shift another tool's
//!   matcher group — that would invalidate *their* trust. Removal therefore
//!   neutralizes our groups into hookless placeholders wherever a foreign
//!   group sits after ours. Never write Codex's config.toml:
//!   Codex rewrites it while running, and the trust grant is the user's.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::Context;
use serde_json::Value;

/// Persist the generation and cursor so idle/restarted readers can reconcile
/// only the question that owns this marker, without rescanning old rollouts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PendingQuestion {
    pub turn_id: String,
    pub call_id: String,
    pub transcript_path: String,
    pub offset: u64,
    pub accepted: bool,
    pub exec: bool,
    #[serde(default)]
    pub question_count: usize,
    #[serde(default)]
    pub answered: Vec<usize>,
    #[serde(default)]
    pub discard_line: bool,
}

impl PendingQuestion {
    pub fn marker(&self) -> String {
        format!("codex_question_async:{}", self.call_id)
    }
}

/// Called under the state lock. Missing/malformed files cannot clear a request.
pub(crate) fn reconcile_questions(state: &mut crate::state::HookState) -> bool {
    use crate::state::{QuestionExpiry, Status};
    let mut changed = false;
    let mut resolved = Vec::new();
    for (id, pending) in &mut state.codex_questions {
        let Some(session) = state.sessions.get_mut(id).filter(|s| {
            s.harness.as_deref() == Some("codex")
                && !s.codex_background
                && s.status == Status::NeedsHelp
                && s.notification_type.as_deref() == Some(pending.marker().as_str())
        }) else {
            resolved.push(id.clone());
            continue;
        };
        let offset = pending.offset;
        if let Some(resolution) = question_resolution(pending) {
            session.status = Status::Active;
            session.notification_type = None;
            if let QuestionResolution::Terminal(timestamp) = resolution {
                session.status = if pending.exec {
                    Status::Done
                } else {
                    Status::Inactive
                };
                session.last_updated.clone_from(&timestamp);
                if pending.accepted
                    && (pending.question_count == 0
                        || pending.answered.len() < pending.question_count)
                {
                    let expiry = QuestionExpiry {
                        id: format!("{id}:{}:{}", pending.turn_id, pending.call_id),
                        occurred_at: timestamp,
                    };
                    let previous = state.question_expiry.as_ref().and_then(|old| {
                        chrono::DateTime::parse_from_rfc3339(&old.occurred_at).ok()
                    });
                    if previous.is_none_or(|old| {
                        chrono::DateTime::parse_from_rfc3339(&expiry.occurred_at).unwrap() > old
                    }) {
                        state.question_expiry = Some(expiry);
                    }
                }
            }
            resolved.push(id.clone());
        }
        changed |= pending.offset != offset;
    }
    changed |= !resolved.is_empty();
    for id in resolved {
        state.codex_questions.remove(&id);
    }
    changed
}

enum QuestionResolution {
    Answered,
    Rejected,
    Terminal(String),
}

fn question_resolution(pending: &mut PendingQuestion) -> Option<QuestionResolution> {
    // Bound each tick, including an unexpectedly enormous or incomplete JSON line.
    const BUDGET: u64 = 256 * 1024;
    let mut file = File::open(&pending.transcript_path).ok()?;
    if file.metadata().ok()?.len() < pending.offset {
        // A rewritten/truncated transcript no longer proves this generation.
        return None;
    }
    file.seek(SeekFrom::Start(pending.offset)).ok()?;
    let mut bytes = Vec::new();
    file.take(BUDGET).read_to_end(&mut bytes).ok()?;
    let complete = bytes.iter().rposition(|b| *b == b'\n').map(|i| i + 1);
    let Some(complete) = complete else {
        if bytes.len() as u64 == BUDGET {
            pending.offset += BUDGET;
            pending.discard_line = true;
        }
        return None;
    };
    pending.offset += complete as u64;
    for line in bytes[..complete].split(|b| *b == b'\n') {
        if pending.discard_line {
            pending.discard_line = false;
            continue;
        }
        let Ok(record) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        let payload = &record["payload"];
        if record["type"] == "event_msg"
            && payload["type"] == "item_completed"
            && payload["turn_id"].as_str() == Some(&pending.turn_id)
            && payload["item"]["type"] == "AgentMessage"
            && payload["item"]["id"].as_str() == Some(&pending.call_id)
        {
            if let Some(questions) = payload["item"]["questions"]
                .as_array()
                .filter(|q| !q.is_empty())
            {
                pending.accepted = true;
                pending.question_count = questions.len();
            }
        }
        if record["type"] == "response_item" {
            if payload["type"] == "function_call_output"
                && payload["call_id"].as_str() == Some(&pending.call_id)
            {
                let output = payload["output"]
                    .as_str()
                    .and_then(|s| serde_json::from_str::<Value>(s).ok())
                    .unwrap_or_else(|| payload["output"].clone());
                if output["accepted"] == false || output["isError"] == true {
                    return Some(QuestionResolution::Rejected);
                }
                pending.accepted |= output["accepted"] == true;
            }
            if payload["type"] == "message" && payload["role"] == "user" {
                if let Some(content) = payload["content"].as_array() {
                    for part in content {
                        if part["type"] == "input_text" {
                            record_question_answers(
                                pending,
                                part["text"].as_str().unwrap_or_default(),
                            );
                        }
                    }
                }
            }
        }
        if record["type"] == "event_msg"
            && matches!(
                payload["type"].as_str(),
                Some("task_complete" | "turn_aborted")
            )
            && payload["turn_id"].as_str() == Some(&pending.turn_id)
        {
            if let Some(timestamp) = record["timestamp"]
                .as_str()
                .filter(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok())
            {
                return Some(QuestionResolution::Terminal(timestamp.to_string()));
            }
        }
    }
    (pending.question_count > 0 && pending.answered.len() == pending.question_count)
        .then_some(QuestionResolution::Answered)
}

fn record_question_answers(pending: &mut PendingQuestion, text: &str) {
    let Some(reply) = text
        .trim()
        .strip_prefix("<send_user_message_question_reply>")
        .and_then(|s| s.strip_suffix("</send_user_message_question_reply>"))
    else {
        return;
    };
    let answers = match serde_json::from_str::<Value>(reply) {
        Ok(Value::Array(answers)) => answers,
        Ok(answer @ Value::Object(_)) => vec![answer],
        _ => return,
    };
    for answer in answers {
        let Some(key) = answer["questionItemId"]
            .as_str()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
        else {
            continue;
        };
        if key.as_array().is_none_or(|key| key.len() != 3)
            || key[0] != "request_user_input_async"
            || key[1].as_str() != Some(&pending.call_id)
            || !answer["answer"].is_string()
        {
            continue;
        }
        if let Some(index) = key[2].as_u64().and_then(|i| usize::try_from(i).ok()) {
            if index < pending.question_count && !pending.answered.contains(&index) {
                pending.answered.push(index);
            }
        }
    }
}

/// Codex's home directory: `$CODEX_HOME` when set (tests, relocated installs),
/// else `~/.codex`.
pub fn codex_home() -> Option<PathBuf> {
    if let Some(h) = std::env::var_os("CODEX_HOME") {
        if !h.is_empty() {
            return Some(PathBuf::from(h));
        }
    }
    dirs::home_dir().map(|home| home.join(".codex"))
}

fn hooks_json_path() -> Option<PathBuf> {
    codex_home().map(|h| h.join("hooks.json"))
}

/// Lifecycle events the adapter registers, including the newer
/// `Interrupt`/`SessionEnd` signals.
const HOOK_EVENTS: [&str; 8] = [
    "SessionStart",
    "UserPromptSubmit",
    "Stop",
    "PreToolUse",
    "PostToolUse",
    "PermissionRequest",
    "Interrupt",
    "SessionEnd",
];

// ---------------------------------------------------------------------------
// hooks.json registration (called via the harness adapter)
// ---------------------------------------------------------------------------

/// Register `broglow codex-hook` under each event in `hooks.json`,
/// preserving every foreign matcher group *in place* — Codex keys its
/// hook-trust state by position in the file, so reordering another tool's
/// group would flag it for re-review. Our own group is replaced in place
/// (refreshing a stale binary path), else fills a placeholder slot left by a
/// prior removal, else appends. Idempotent.
pub fn install_hooks() -> anyhow::Result<()> {
    let path = hooks_json_path().context("No home directory")?;

    let mut root: Value = match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content)
            // Never clobber a file we can't parse — rewriting it would drop
            // the user's other hooks.
            .with_context(|| format!("{} exists but isn't valid JSON", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::json!({}),
        Err(e) => return Err(e).with_context(|| format!("Reading {}", path.display())),
    };

    let exe = std::env::current_exe().context("Resolving current executable path")?;
    let group = serde_json::json!({
        "hooks": [{
            "type": "command",
            "command": format!("\"{}\" codex-hook", exe.display()),
            "statusMessage": "BroGlow: update session status",
        }]
    });

    let root_obj = root
        .as_object_mut()
        .with_context(|| format!("{} must be a JSON object", path.display()))?;
    let hooks = root_obj
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));
    let hooks_obj = hooks
        .as_object_mut()
        .with_context(|| format!("\"hooks\" in {} must be an object", path.display()))?;
    for event in HOOK_EVENTS {
        let entry = hooks_obj
            .entry(event.to_string())
            .or_insert_with(|| serde_json::json!([]));
        if let Some(groups) = entry.as_array_mut() {
            upsert_broglow_group(groups, &group);
        }
    }

    write_json_atomic(&path, &root)?;
    println!("Wrote Codex hooks to {}", path.display());
    println!("  (Codex asks once to trust new hooks — approve broglow via /hooks)");
    Ok(())
}

/// Strip broglow's matcher groups from every event, leaving all other
/// hooks untouched — and *in place*: a group of ours sitting in front of a
/// foreign group is neutralized into a hookless placeholder rather than
/// removed, so the foreign group's trust-by-position survives (see
/// [`remove_broglow_groups`]). The adapter's `uninstall`: best-effort, and
/// the file itself stays (other tools register hooks there too).
pub fn uninstall_hooks() {
    let Some(path) = hooks_json_path().filter(|p| p.exists()) else {
        return;
    };
    let Some(mut root) = std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str::<Value>(&c).ok())
    else {
        return; // unreadable: leave it alone
    };
    let Some(hooks_obj) = root.get_mut("hooks").and_then(|h| h.as_object_mut()) else {
        return;
    };

    let mut changed = false;
    let events: Vec<String> = hooks_obj.keys().cloned().collect();
    for event in events {
        if let Some(groups) = hooks_obj.get_mut(&event).and_then(|v| v.as_array_mut()) {
            changed |= remove_broglow_groups(groups);
            if groups.is_empty() {
                hooks_obj.remove(&event);
            }
        }
    }
    if changed {
        match write_json_atomic(&path, &root) {
            Ok(()) => println!("Removed Codex hooks from {}", path.display()),
            Err(e) => eprintln!("broglow: could not update {}: {e:#}", path.display()),
        }
    }
}

/// Arguments of a direct BroGlow invocation. Reject wrappers, lookalike
/// executables and malformed quotes before claiming ownership of a hook.
pub(crate) fn broglow_command_args(cmd: &str) -> Option<&str> {
    let cmd = cmd.trim();
    let (exe, args) = if let Some(quoted) = cmd.strip_prefix('"') {
        let (exe, rest) = quoted.split_once('"')?;
        if !rest.starts_with(char::is_whitespace) {
            return None;
        }
        (exe, rest.trim())
    } else {
        cmd.split_once(char::is_whitespace)?
    };
    matches!(exe.rsplit(['/', '\\']).next()?, "broglow" | "broglow.exe").then_some(args.trim())
}

pub(crate) fn command_is_broglow(cmd: &str) -> bool {
    matches!(broglow_command_args(cmd), Some("hook" | "codex-hook"))
}

/// A matcher group is ours only when *every* hook in it is broglow's — a
/// group mixing in anyone else's command is never replaced or removed.
pub(crate) fn group_is_broglow(group: &Value) -> bool {
    let Some(hooks) = group.get("hooks").and_then(|h| h.as_array()) else {
        return false;
    };
    !hooks.is_empty()
        && hooks.iter().all(|h| {
            h.get("command")
                .and_then(|c| c.as_str())
                .is_some_and(command_is_broglow)
        })
}

/// The inert group left behind when one of ours must vanish from *in front
/// of* a foreign group: hookless, so Codex runs nothing, while every later
/// group keeps its index (Codex trusts hooks by content hash + position — a
/// plain `retain` would shift the foreign group down and invalidate its
/// trust).
fn placeholder_group() -> Value {
    serde_json::json!({ "hooks": [] })
}

/// A hookless group with nothing else in it — either our own placeholder or
/// a semantically inert group someone left behind. Safe to reuse as a slot.
fn group_is_placeholder(group: &Value) -> bool {
    group
        .as_object()
        .is_some_and(|o| o.keys().all(|k| k == "hooks"))
        && group
            .get("hooks")
            .and_then(|h| h.as_array())
            .is_some_and(|h| h.is_empty())
}

/// Remove broglow's groups from one event's array without shifting any
/// foreign group's index: everything past the last foreign group (ours and
/// stale placeholders) is truncated off the tail, and one of ours *followed
/// by* a foreign group is neutralized into a [`placeholder_group`] in place.
/// Returns whether anything changed.
pub(crate) fn remove_broglow_groups(groups: &mut Vec<Value>) -> bool {
    let mut changed = false;
    let last_foreign = groups
        .iter()
        .rposition(|g| !group_is_broglow(g) && !group_is_placeholder(g));
    let keep = last_foreign.map_or(0, |f| f + 1);
    for g in groups.iter_mut().take(keep) {
        if group_is_broglow(g) {
            *g = placeholder_group();
            changed = true;
        }
    }
    if groups.len() > keep {
        groups.truncate(keep);
        changed = true;
    }
    changed
}

/// Put our matcher group into an event's array: refresh our existing group in
/// place, else fill a placeholder slot (left by [`remove_broglow_groups`] —
/// reusing it keeps foreign positions stable across install/uninstall
/// cycles), else append.
pub(crate) fn upsert_broglow_group(groups: &mut Vec<Value>, group: &Value) {
    if let Some(ours) = groups.iter_mut().find(|g| group_is_broglow(g)) {
        ours.clone_from(group);
    } else if let Some(slot) = groups.iter_mut().find(|g| group_is_placeholder(g)) {
        slot.clone_from(group);
    } else {
        groups.push(group.clone());
    }
}

/// Atomic JSON write (sibling temp file + rename): a crash mid-write must
/// never leave Codex a half-written hooks file.
pub(crate) fn write_json_atomic(path: &Path, value: &Value) -> anyhow::Result<()> {
    // Write through a symlink (dotfiles setups link ~/.claude/settings.json)
    // instead of replacing the link itself with a plain file.
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path = resolved.as_path();
    let dir = path.parent().context("hooks.json must have a parent")?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".broglow.{}.tmp", std::process::id()));
    std::fs::write(&tmp, serde_json::to_string_pretty(value)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Session naming
// ---------------------------------------------------------------------------

/// Hooks run before approval review, so a request need not involve the user.
/// Only the matching turn's explicit reviewer can suppress the red signal;
/// missing, unreadable or older rollout formats retain human-approval behavior.
pub fn approval_is_automatic(transcript_path: &str, turn_id: Option<&str>) -> bool {
    let Some(turn_id) = turn_id.filter(|id| !id.is_empty()) else {
        return false;
    };
    let Ok(file) = File::open(transcript_path) else {
        return false;
    };
    let mut automatic = false;
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if !line.contains("\"turn_context\"") {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if v["type"] == "turn_context" {
            automatic = v["payload"]["turn_id"].as_str() == Some(turn_id)
                && v["payload"]["approvals_reviewer"] == "auto_review";
        }
    }
    automatic
}

/// Desktop provenance is stronger than terminal variables inherited when the
/// app was launched from a shell or IDE. `source: vscode` alone is ambiguous.
#[cfg(target_os = "macos")]
pub fn rollout_is_desktop(transcript_path: &str) -> bool {
    rollout_metadata(transcript_path).is_some_and(|v| v["originator"] == "Codex Desktop")
}

/// Child hooks can carry the parent's session_id. The rollout's modern id
/// identifies the actual thread, even when its legacy session_id is inherited.
pub fn rollout_thread_id(transcript_path: &str) -> Option<String> {
    let meta = rollout_metadata(transcript_path)?;
    ["id", "session_id"].into_iter().find_map(|key| {
        meta[key]
            .as_str()
            .filter(|id| !id.is_empty())
            .map(str::to_owned)
    })
}

/// Unsaved app jobs have no transcript to reopen. Internal agents and approval
/// reviewers can have rollouts, but belong to their parent chat's status.
/// An omitted path or unreadable metadata is inconclusive on older Codex builds.
pub fn hook_is_background(v: &Value) -> bool {
    match v.get("transcript_path") {
        Some(Value::Null) => true,
        Some(Value::String(path)) if path.is_empty() => true,
        Some(Value::String(path)) => rollout_metadata(path).is_some_and(|meta| {
            meta["source"].get("subagent").is_some()
                || matches!(
                    meta["thread_source"].as_str(),
                    Some(
                        "subagent"
                            | "guardian_review"
                            | "ambient_suggestions"
                            | "ambient_suggestion_safety"
                    )
                )
        }),
        _ => false,
    }
}

fn rollout_metadata(transcript_path: &str) -> Option<Value> {
    let file = File::open(transcript_path).ok()?;
    let line = BufReader::new(file).lines().next()?.ok()?;
    let mut v: Value = serde_json::from_str(&line).ok()?;
    (v["type"] == "session_meta").then(|| v["payload"].take())
}

/// Session id → thread name from Codex's `session_index.jsonl`. Codex titles
/// threads itself, so these beat anything broglow could generate. The file
/// is append-style; the last entry for an id wins.
pub fn thread_names() -> HashMap<String, String> {
    let Some(path) = codex_home().map(|h| h.join("session_index.jsonl")) else {
        return HashMap::new();
    };
    let Ok(file) = File::open(path) else {
        return HashMap::new();
    };
    parse_thread_names(BufReader::new(file).lines().map_while(Result::ok))
}

fn parse_thread_names(lines: impl Iterator<Item = String>) -> HashMap<String, String> {
    let mut names = HashMap::new();
    for line in lines {
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let (Some(id), Some(name)) = (
            v.get("id").and_then(Value::as_str),
            v.get("thread_name").and_then(Value::as_str),
        ) else {
            continue;
        };
        if !name.trim().is_empty() {
            names.insert(id.to_string(), name.to_string());
        }
    }
    names
}

/// First typed user prompt from a Codex rollout, for fallback naming when no
/// thread name exists yet. Prefers `event_msg`/`user_message` records (text
/// the user actually typed); falls back to the first user `response_item`
/// that isn't injected context (those arrive wrapped in an XML-ish `<tag>`).
/// Streams line-by-line — rollouts grow into the tens of MB.
pub fn first_user_message(transcript_path: &str) -> Option<String> {
    let file = File::open(transcript_path).ok()?;
    let mut fallback: Option<String> = None;
    for line in BufReader::new(file).lines() {
        let Ok(line) = line else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        match extract_user_text(&v) {
            Some(UserText::Typed(text)) => return Some(clip_prompt(&text)),
            Some(UserText::ResponseItem(text)) if fallback.is_none() => {
                fallback = Some(clip_prompt(&text));
            }
            _ => {}
        }
    }
    fallback
}

enum UserText {
    /// `event_msg` / `user_message`: text the user actually typed.
    Typed(String),
    /// `response_item` user message: may be injected context; fallback only.
    ResponseItem(String),
}

fn extract_user_text(v: &Value) -> Option<UserText> {
    let payload = v.get("payload")?;
    match v.get("type").and_then(Value::as_str)? {
        "event_msg" => {
            if payload.get("type").and_then(Value::as_str) != Some("user_message") {
                return None;
            }
            let text = payload.get("message").and_then(Value::as_str)?.trim();
            (!text.is_empty()).then(|| UserText::Typed(text.to_string()))
        }
        "response_item" => {
            if payload.get("type").and_then(Value::as_str) != Some("message")
                || payload.get("role").and_then(Value::as_str) != Some("user")
            {
                return None;
            }
            let text: String = payload
                .get("content")?
                .as_array()?
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("input_text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(" ");
            let text = text.trim();
            // Injected context (plugin lists, AGENTS.md, env blocks) arrives
            // as user-role messages wrapped in XML-ish tags — not a prompt.
            (!text.is_empty() && !text.starts_with('<'))
                .then(|| UserText::ResponseItem(text.to_string()))
        }
        _ => None,
    }
}

/// First line, capped to ~500 chars. Char-based, never byte-sliced (repo
/// rule: prompts are arbitrary UTF-8).
fn clip_prompt(text: &str) -> String {
    let capped: String = text.chars().take(500).collect();
    capped.lines().next().unwrap_or("").trim().to_string()
}

// ---------------------------------------------------------------------------
// Rollout metadata
// ---------------------------------------------------------------------------

/// Whether a rollout belongs to a one-shot `codex exec` run. Those finish for
/// good on their single `Stop`, so the shim maps it to `ended` instead of
/// leaving a "paused" session nobody will come back to. Reads only the first
/// line (the `session_meta` record).
pub fn rollout_is_exec(transcript_path: &str) -> bool {
    let Ok(file) = File::open(transcript_path) else {
        return false;
    };
    let mut first_line = String::new();
    if BufReader::new(file).read_line(&mut first_line).is_err() {
        return false;
    }
    session_meta_is_exec(&first_line)
}

fn session_meta_is_exec(line: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    if v.get("type").and_then(Value::as_str) != Some("session_meta") {
        return false;
    }
    let Some(payload) = v.get("payload") else {
        return false;
    };
    payload.get("source").and_then(Value::as_str) == Some("exec")
        || payload.get("originator").and_then(Value::as_str) == Some("codex_exec")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_expiry_keeps_the_newest_terminal_across_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = crate::state::HookState::default();
        for (id, timestamp) in [
            ("fresh", "2026-10-21T10:00:00Z"),
            ("old", "2026-10-21T09:00:00Z"),
        ] {
            let path = dir.path().join(id);
            std::fs::write(
                &path,
                serde_json::json!({"type":"event_msg", "timestamp":timestamp,
                "payload":{"type":"task_complete", "turn_id":"turn"}})
                .to_string()
                    + "\n",
            )
            .unwrap();
            state.sessions.insert(
                id.into(),
                serde_json::from_value(serde_json::json!({
                    "status":"needs_help", "last_updated":timestamp, "harness":"codex",
                    "notification_type":"codex_question_async:call"
                }))
                .unwrap(),
            );
            state.codex_questions.insert(
                id.into(),
                PendingQuestion {
                    turn_id: "turn".into(),
                    call_id: "call".into(),
                    transcript_path: path.to_string_lossy().into_owned(),
                    offset: 0,
                    accepted: true,
                    exec: false,
                    question_count: 1,
                    answered: Vec::new(),
                    discard_line: false,
                },
            );
        }
        assert!(reconcile_questions(&mut state));
        assert_eq!(state.question_expiry.unwrap().id, "fresh:turn:call");
    }

    #[test]
    fn question_parser_bounds_work_and_requires_exact_terminal_evidence() {
        let rollout = tempfile::NamedTempFile::new().unwrap();
        let mut pending = PendingQuestion {
            turn_id: "turn".into(),
            call_id: "call".into(),
            transcript_path: rollout.path().to_string_lossy().into_owned(),
            offset: 0,
            accepted: false,
            exec: false,
            question_count: 1,
            answered: Vec::new(),
            discard_line: false,
        };
        let mut contents = "x".repeat(300 * 1024) + "\n";
        for (turn, timestamp) in [
            ("old", Some("2026-10-21T10:00:00Z")),
            ("turn", None),
            ("turn", Some("unknown")),
        ] {
            contents += &(serde_json::json!({"type":"event_msg", "timestamp":timestamp,
                "payload":{"type":"task_complete", "turn_id":turn}})
            .to_string()
                + "\n");
        }
        std::fs::write(rollout.path(), &contents).unwrap();
        assert!(question_resolution(&mut pending).is_none());
        assert_eq!(pending.offset, 256 * 1024);
        assert!(question_resolution(&mut pending).is_none());
        assert_eq!(pending.offset, contents.len() as u64);
        contents += &(serde_json::json!({"type":"response_item", "payload":{
            "type":"function_call_output", "call_id":"call", "output":"{\"accepted\":true}"
        }})
        .to_string()
            + "\n");
        contents += &(serde_json::json!({"type":"event_msg", "timestamp":"2026-10-21T10:00:00Z",
            "payload":{"type":"task_complete", "turn_id":"turn"}})
        .to_string()
            + "\n");
        std::fs::write(rollout.path(), &contents).unwrap();
        assert!(matches!(
            question_resolution(&mut pending),
            Some(QuestionResolution::Terminal(_))
        ));
        assert!(pending.accepted);
        let answer = serde_json::json!({"questionItemId": "[\"request_user_input_async\",\"call\",0]", "answer":"yes"});
        let reply = format!(
            "<send_user_message_question_reply>{answer}</send_user_message_question_reply>"
        );
        record_question_answers(&mut pending, &format!("Example: {reply}"));
        assert!(pending.answered.is_empty());
        record_question_answers(&mut pending, &format!("  {reply}\n"));
        assert_eq!(pending.answered, vec![0]);
    }

    // Real line shapes captured from a Codex 0.144/0.145 install.
    const EVENT_USER: &str = r#"{"timestamp":"2026-07-20T13:08:52.681Z","type":"event_msg","payload":{"type":"user_message","client_id":"0bd480da","message":"how does codex differ from claude code?\n","images":[]}}"#;
    const ITEM_INJECTED: &str = r#"{"timestamp":"2026-07-20T13:08:52.675Z","type":"response_item","payload":{"type":"message","id":"msg_1","role":"user","content":[{"type":"input_text","text":"<recommended_plugins>\nplugin list here\n</recommended_plugins>"}]}}"#;
    const ITEM_TYPED: &str = r#"{"timestamp":"2026-07-20T13:08:52.681Z","type":"response_item","payload":{"type":"message","id":"msg_2","role":"user","content":[{"type":"input_text","text":"fix the flaky test in ota.rs"}]}}"#;

    #[test]
    fn rollout_names_prefer_typed_prompts_and_ignore_injected_context() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout.jsonl");
        for (lines, expected) in [
            (vec![ITEM_INJECTED, "not json"], None),
            (
                vec![ITEM_INJECTED, ITEM_TYPED],
                Some("fix the flaky test in ota.rs"),
            ),
            (
                vec![ITEM_INJECTED, ITEM_TYPED, EVENT_USER],
                Some("how does codex differ from claude code?"),
            ),
        ] {
            std::fs::write(&path, lines.join("\n")).unwrap();

            assert_eq!(
                first_user_message(path.to_str().unwrap()).as_deref(),
                expected
            );
        }
    }

    #[test]
    fn rollout_prompt_titles_use_the_first_line_with_a_unicode_safe_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout.jsonl");
        for (prompt, expected) in [
            ("fix this\nand that".into(), "fix this".into()),
            ("日".repeat(600), "日".repeat(500)),
        ] {
            std::fs::write(
                &path,
                serde_json::json!({
                    "type": "event_msg", "payload": {"type": "user_message", "message": prompt}
                })
                .to_string(),
            )
            .unwrap();

            assert_eq!(first_user_message(path.to_str().unwrap()), Some(expected));
        }
    }

    #[test]
    fn hooks_merge_preserves_foreign_groups_in_place() {
        // Real-world shape: the user's own PermissionRequest hook at index 0.
        // Ours appends after it; Codex keys hook trust by position, so the
        // foreign group must never move.
        let mut hooks = serde_json::json!({
            "PermissionRequest": [
                { "hooks": [{ "type": "command",
                              "command": "~/.claude/scripts/permission-hook.sh",
                              "timeout": 600 }] }
            ],
            "Stop": [
                { "hooks": [{ "type": "command",
                              "command": "\"/installed/path/broglow\" codex-hook" }] }
            ],

        });
        let obj = hooks.as_object_mut().unwrap();
        let group = serde_json::json!({
            "hooks": [{ "type": "command", "command": "\"/new/broglow\" codex-hook" }]
        });
        for event in HOOK_EVENTS {
            let entry = obj
                .entry(event.to_string())
                .or_insert_with(|| serde_json::json!([]));
            upsert_broglow_group(entry.as_array_mut().unwrap(), &group);
        }

        let pr = obj["PermissionRequest"].as_array().unwrap();
        assert_eq!(pr.len(), 2);
        assert_eq!(
            pr[0]["hooks"][0]["command"],
            "~/.claude/scripts/permission-hook.sh"
        );
        assert_eq!(pr[1], group);
        // An existing BroGlow registration is refreshed in place, not duplicated.
        let stop = obj["Stop"].as_array().unwrap();
        assert_eq!(stop.len(), 1);
        assert_eq!(stop[0], group);
    }

    #[test]
    fn broglow_commands_recognized_foreign_ones_not() {
        assert!(command_is_broglow(
            "\"/Users/x/.cargo/bin/broglow\" codex-hook"
        ));
        assert!(command_is_broglow("broglow hook")); // Claude hook backend
        assert!(command_is_broglow("\"C:\\bin\\broglow.exe\" codex-hook"));
        assert!(!command_is_broglow("~/.claude/scripts/permission-hook.sh"));
        assert!(!command_is_broglow("\"/usr/bin/broglow\" led"));
        // A foreign binary whose name merely *ends* in "broglow" is not
        // ours — a bare suffix match would strip someone else's hook.
        assert!(!command_is_broglow("/usr/bin/my-broglow codex-hook"));
        assert!(!command_is_broglow("\"/opt/not-broglow\" hook"));
        assert!(!command_is_broglow("notbroglow codex-hook"));
        for cmd in [
            "echo /bin/broglow hook",
            "\"/bin/broglow\"hook",
            "broglow hook && other",
            "broglow-helper hook",
        ] {
            assert!(!command_is_broglow(cmd), "{cmd}");
        }
    }

    #[test]
    fn removal_never_shifts_a_foreign_groups_position() {
        let foreign = serde_json::json!({
            "hooks": [{ "type": "command", "command": "./their-hook.sh" }]
        });
        let ours = serde_json::json!({
            "hooks": [{ "type": "command", "command": "\"/x/broglow\" codex-hook" }]
        });

        // Ours in front of a foreign group: neutralized in place, so the
        // foreign group keeps index 1 (its trust key encodes the position).
        let mut groups = vec![ours.clone(), foreign.clone()];
        assert!(remove_broglow_groups(&mut groups));
        assert_eq!(groups.len(), 2);
        assert!(group_is_placeholder(&groups[0]));
        assert_eq!(groups[1], foreign);

        // A later reinstall fills the placeholder slot instead of appending,
        // so the foreign group *still* hasn't moved.
        upsert_broglow_group(&mut groups, &ours);
        assert_eq!(groups[0], ours);
        assert_eq!(groups[1], foreign);

        // Ours on the tail: plain removal, nothing shifts.
        let mut groups = vec![foreign.clone(), ours.clone()];
        assert!(remove_broglow_groups(&mut groups));
        assert_eq!(groups, vec![foreign.clone()]);

        // Only ours: the array empties (the caller then drops the event key).
        let mut groups = vec![ours.clone()];
        assert!(remove_broglow_groups(&mut groups));
        assert!(groups.is_empty());

        // Nothing of ours: untouched, and reported as such.
        let mut groups = vec![foreign.clone()];
        assert!(!remove_broglow_groups(&mut groups));
        assert_eq!(groups, vec![foreign]);
    }
}
