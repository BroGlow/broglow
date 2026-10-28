//! Opt-in local token totals per coding agent, with independently attributed
//! subscription limits. Tokens include cached input; they are not a bill or a
//! measure of subscription utilization. Missing provider data stays unavailable.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::sync::Mutex;

use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub today_tokens: u64,
    pub agents: Vec<AgentUsage>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentUsage {
    pub harness: &'static str,
    pub label: &'static str,
    pub badge: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<&'static str>,
    pub today_tokens: u64,
    pub limits: Vec<UsageLimit>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimit {
    pub label: String,
    pub used_percent: f64,
    pub reset: Option<String>,
}

/// Shared cache between the refresher thread and the tray/popover readers.
/// Only the tray daemon (macOS/Windows) uses it; the cross-platform `broglow
/// usage` subcommand computes its snapshot directly, so gate it to those
/// platforms to avoid a dead-code error on Linux.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static LATEST: Mutex<Option<UsageSnapshot>> = Mutex::new(None);

/// Most recent snapshot computed by the refresher thread.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn latest() -> Option<UsageSnapshot> {
    LATEST.lock().ok()?.clone()
}

/// "1.2M" / "12k" / "950", matching the popover's `fmtTokens`.
#[cfg(any(test, target_os = "macos", target_os = "windows"))]
pub fn compact_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1e6)
    } else if n >= 1_000 {
        format!("{}k", (n as f64 / 1e3).round())
    } else {
        n.to_string()
    }
}

/// Background refresher for the tray daemon: rescans the transcripts every
/// minute, refetches the plan percentages every five, and calls `on_change`
/// after each snapshot so the event loop can repaint.
///
/// Gated on the opt-in `config::usage_enabled`: while usage is off (the
/// default) this does no work at all — no transcript scan, no reading of Claude
/// Code's credentials, no request to the usage endpoint — it only re-checks the
/// setting cheaply so it can start the moment the user turns it on. Turning it
/// back off clears the last snapshot so the UI drops the usage section.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn spawn_refresher(on_change: impl Fn() + Send + 'static) {
    std::thread::spawn(move || {
        const SCAN_EVERY: std::time::Duration = std::time::Duration::from_secs(60);
        // While disabled, poll the setting this often so opting in feels prompt
        // without doing any of the actual (privacy-sensitive) work.
        const IDLE_POLL: std::time::Duration = std::time::Duration::from_secs(5);
        const PLAN_EVERY_SCANS: u32 = 5;
        let mut tracker = Tracker::new();
        let mut plan = None;
        let mut scans = 0u32;
        let mut prev_enabled = false;
        loop {
            if !crate::config::read_config().usage_enabled {
                // On the enabled→disabled edge, drop the snapshot and repaint
                // once so a stale readout can't linger after opting out.
                if prev_enabled {
                    if let Ok(mut latest) = LATEST.lock() {
                        *latest = None;
                    }
                    on_change();
                }
                prev_enabled = false;
                plan = None;
                scans = 0;
                std::thread::sleep(IDLE_POLL);
                continue;
            }
            prev_enabled = true;
            tracker.scan();
            if scans.is_multiple_of(PLAN_EVERY_SCANS) {
                plan = fetch_plan_usage();
            }
            scans = scans.wrapping_add(1);
            let snap = tracker.snapshot(plan.as_ref());
            if let Ok(mut latest) = LATEST.lock() {
                *latest = Some(snap);
            }
            on_change();
            std::thread::sleep(SCAN_EVERY);
        }
    });
}

/// One-shot snapshot for the hidden `broglow usage` subcommand.
pub fn run_once() -> anyhow::Result<()> {
    let mut tracker = Tracker::new();
    tracker.scan();
    let snap = tracker.snapshot(fetch_plan_usage().as_ref());
    println!("{}", serde_json::to_string_pretty(&snap)?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Transcript scanning

#[derive(Debug, Default, Clone, Copy)]
struct FilePos {
    offset: u64,
    len: u64,
    modified: Option<std::time::SystemTime>,
}

/// Codex `token_count` events carry **cumulative** session totals, so the
/// per-file scan state must remember the last cumulative reading (usage per
/// event is the delta).
#[derive(Debug, Default, Clone)]
struct CodexFilePos {
    offset: u64,
    len: u64,
    prev: CodexCumulative,
    context: CodexContext,
    modified: Option<std::time::SystemTime>,
}

#[derive(Debug, Default, Clone)]
struct CodexContext {
    owner: String,
    session: String,
    inherited: bool,
    turn: Option<String>,
    reset: u64,
}

#[derive(Debug, Default, Clone, Copy)]
struct CodexCumulative {
    input: u64,
    output: u64,
    total: u64,
}

struct Tracker {
    day: NaiveDate,
    files: HashMap<PathBuf, FilePos>,
    /// Codex rollout files (`$CODEX_HOME/sessions/YYYY/MM/DD/*.jsonl`).
    codex_files: HashMap<PathBuf, CodexFilePos>,
    /// message id / request id already counted today, across all files.
    seen: HashMap<String, u64>,
    codex_seen: HashSet<String>,
    claude_tokens: u64,
    codex_tokens: u64,
    opencode_tokens: u64,
    codex_plan: Option<(DateTime<chrono::FixedOffset>, serde_json::Value)>,
}

/// One transcript line, reduced to the fields usage accounting needs.
/// Everything is optional: transcripts hold many line shapes (user, system,
/// summary…) and future schema growth must not break the scan.
#[derive(Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: Option<String>,
    timestamp: Option<String>,
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    message: Option<Msg>,
}

#[derive(Deserialize)]
struct Msg {
    id: Option<String>,
    model: Option<String>,
    usage: Option<RawUsage>,
}

#[derive(Deserialize)]
struct RawUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cache_creation_input_tokens: u64,
    #[serde(default)]
    cache_read_input_tokens: u64,
}

fn projects_dir() -> Option<PathBuf> {
    Some(crate::harness::claude::projects_dir())
}

/// Recursively collect `*.jsonl` under `dir`, bounded to `depth` levels —
/// Codex's fixed `YYYY/MM/DD` layout needs no walkdir dependency.
fn collect_jsonl(dir: &std::path::Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if depth > 0 {
                collect_jsonl(&path, depth - 1, out);
            }
        } else if path.extension().is_some_and(|e| e == "jsonl") {
            out.push(path);
        }
    }
}

impl Tracker {
    fn new() -> Self {
        Self {
            day: Local::now().date_naive(),
            files: HashMap::new(),
            codex_files: HashMap::new(),
            seen: HashMap::new(),
            codex_seen: HashSet::new(),
            claude_tokens: 0,
            codex_tokens: 0,
            opencode_tokens: 0,
            codex_plan: None,
        }
    }

    /// Incrementally fold new transcript lines into today's tallies.
    fn scan(&mut self) {
        let today = Local::now().date_naive();
        if today != self.day {
            // Day rolled over: everything counted so far belongs to yesterday.
            self.day = today;
            self.files.clear();
            self.codex_files.clear();
            self.seen.clear();
            self.codex_seen.clear();
            self.claude_tokens = 0;
            self.codex_tokens = 0;
            self.codex_plan = None;
        }

        // Session files last written before local midnight can't contain
        // today's entries (lines are appended as they happen).
        let midnight = today
            .and_hms_opt(0, 0, 0)
            .and_then(|t| t.and_local_timezone(Local).single());

        self.opencode_tokens = crate::opencode_usage::today_tokens(today);
        self.scan_codex(midnight);
        let Some(root) = projects_dir() else { return };
        let mut files = Vec::new();
        // projects/<project>/<session>/subagents/*.jsonl also contains usage.
        collect_jsonl(&root, 3, &mut files);
        for path in files {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if let (Some(midnight), Ok(mtime)) = (midnight, meta.modified()) {
                if DateTime::<Local>::from(mtime) < midnight {
                    continue;
                }
            }
            self.scan_file(path, meta.len());
        }
    }

    /// Fold new Codex rollout lines into today's tallies. Same incremental
    /// per-file scan as the Claude side; the layout is a fixed
    /// `sessions/YYYY/MM/DD/rollout-*.jsonl` tree, walked with the same
    /// before-midnight mtime skip.
    fn scan_codex(&mut self, midnight: Option<DateTime<Local>>) {
        let Some(home) = crate::codex::codex_home() else {
            return;
        };
        let mut files = Vec::new();
        collect_jsonl(&home.join("sessions"), 4, &mut files);
        collect_jsonl(&home.join("archived_sessions"), 4, &mut files);
        for path in files {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if let (Some(midnight), Ok(mtime)) = (midnight, meta.modified()) {
                if DateTime::<Local>::from(mtime) < midnight {
                    self.scan_old_codex_limits(&path, &meta);
                    continue;
                }
            }
            self.scan_codex_file(path, meta.len());
        }
    }

    fn scan_old_codex_limits(&mut self, path: &std::path::Path, meta: &std::fs::Metadata) {
        let modified = meta.modified().ok();
        if self
            .codex_files
            .get(path)
            .is_some_and(|pos| pos.len == meta.len() && pos.modified == modified)
        {
            return;
        }
        if self
            .codex_plan
            .as_ref()
            .is_some_and(|(ts, _)| modified.is_some_and(|m| DateTime::<Local>::from(m) <= *ts))
        {
            return;
        }
        // shortcut: normal old files use their final 1 MiB for recent quotas;
        // increase this bound if footer records grow. Forks need ownership history.
        let Ok(mut file) = std::fs::File::open(path) else {
            return;
        };
        let mut context = CodexContext::default();
        let mut header = String::new();
        if BufReader::new(&mut file).read_line(&mut header).is_ok() {
            if let Ok(v) = serde_json::from_str(&header) {
                update_codex_context(&v, &mut context);
            }
        }
        let start = if context.inherited {
            0
        } else {
            meta.len().saturating_sub(1024 * 1024)
        };
        if file.seek(SeekFrom::Start(start)).is_err() {
            return;
        }
        let mut reader = BufReader::new(file);
        let mut buf = Vec::new();
        if start > 0 {
            let _ = reader.read_until(b'\n', &mut buf);
        }
        loop {
            buf.clear();
            let Ok(n) = reader.read_until(b'\n', &mut buf) else {
                return;
            };
            if n == 0 || buf.last() != Some(&b'\n') {
                break;
            }
            if let Ok(v) = serde_json::from_slice(&buf) {
                update_codex_context(&v, &mut context);
                if !context.inherited {
                    self.ingest_codex_limits(&v);
                }
            }
        }
        let pos = self.codex_files.entry(path.to_path_buf()).or_default();
        pos.len = meta.len();
        pos.modified = modified;
    }

    fn ingest_codex_limits(&mut self, v: &serde_json::Value) {
        let payload = &v["payload"];
        if v["type"] != "event_msg" || payload["type"] != "token_count" {
            return;
        }
        let timestamp = v
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(|v| DateTime::parse_from_rfc3339(v).ok());
        if let (Some(ts), Some(limits)) = (timestamp, payload.get("rate_limits")) {
            let is_codex = limits
                .get("limit_id")
                .and_then(|v| v.as_str())
                .is_none_or(|id| id == "codex");
            // API sessions have no quota observation; they cannot erase a
            // concurrently used subscription account's last known limits.
            if limits.is_object()
                && is_codex
                && self.codex_plan.as_ref().is_none_or(|(old, _)| ts >= *old)
            {
                self.codex_plan = Some((ts, limits.clone()));
            }
        }
    }

    fn scan_codex_file(&mut self, path: PathBuf, len: u64) {
        let pos = self.codex_files.entry(path.clone()).or_default();
        let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if len == pos.len && modified == pos.modified {
            return;
        }
        if len <= pos.len {
            // Truncated/rewritten: start over with fresh counters.
            *pos = CodexFilePos::default();
        }
        // Work on locals across the read loop; written back at the end (the
        // borrow checker won't allow holding the map entry across ingest).
        let mut offset = pos.offset;
        let mut prev = pos.prev;
        let mut context = pos.context.clone();
        if context.session.is_empty() {
            context.session = path.to_string_lossy().into_owned();
        }
        let Ok(mut file) = std::fs::File::open(&path) else {
            return;
        };
        if file.seek(SeekFrom::Start(offset)).is_err() {
            return;
        }
        let mut reader = BufReader::new(file);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let Ok(n) = reader.read_until(b'\n', &mut buf) else {
                break;
            };
            if n == 0 || buf.last() != Some(&b'\n') {
                break; // EOF or a line still being appended
            }
            offset += n as u64;
            if let Ok(text) = std::str::from_utf8(&buf) {
                self.ingest_codex_record(text, &mut prev, &mut context);
            }
        }
        let pos = self.codex_files.entry(path).or_default();
        pos.offset = offset;
        pos.len = len;
        pos.prev = prev;
        pos.context = context;
        pos.modified = modified;
    }

    /// Fold one rollout line into the tallies. `token_count` events carry
    /// cumulative totals: usage per event is the delta from the previous one,
    /// and a decrease means the counter reset (the current value is the new
    /// delta). Cached input is a subset of `input_tokens`; `output_tokens`
    /// already includes reasoning tokens.
    #[cfg(test)]
    fn ingest_codex_line(&mut self, text: &str, prev: &mut CodexCumulative) {
        self.ingest_codex_record(text, prev, &mut CodexContext::default());
    }

    fn ingest_codex_record(
        &mut self,
        text: &str,
        prev: &mut CodexCumulative,
        context: &mut CodexContext,
    ) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
            return;
        };
        let Some(payload) = v.get("payload") else {
            return;
        };
        update_codex_context(&v, context);
        match v.get("type").and_then(|t| t.as_str()) {
            Some("event_msg")
                if payload.get("type").and_then(|t| t.as_str()) == Some("token_count") =>
            {
                if !context.inherited {
                    self.ingest_codex_limits(&v);
                }
                let Some(total) = payload.get("info").and_then(|i| i.get("total_token_usage"))
                else {
                    return;
                };
                let (Some(input), Some(output)) = (
                    total["input_tokens"].as_u64(),
                    total["output_tokens"].as_u64(),
                ) else {
                    return;
                };
                let cur = CodexCumulative {
                    input,
                    output,
                    total: total
                        .get("total_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or_else(|| input.saturating_add(output)),
                };
                let delta = if cur.total < prev.total {
                    context.reset += 1;
                    cur // counter reset: current value is the fresh delta
                } else {
                    CodexCumulative {
                        input: cur.input.saturating_sub(prev.input),
                        output: cur.output.saturating_sub(prev.output),
                        total: cur.total.saturating_sub(prev.total),
                    }
                };
                *prev = cur;

                // Old lines must still advance `prev` (above) even when they
                // don't count toward today.
                let today = v
                    .get("timestamp")
                    .and_then(|t| t.as_str())
                    .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                    .is_some_and(|t| t.with_timezone(&Local).date_naive() == self.day);
                if !today || context.inherited {
                    return;
                }
                // Forks retain original turn/session metadata but rewrite outer
                // timestamps. Scope identical counters to that verified origin.
                let key = serde_json::json!([
                    context.session,
                    context.turn,
                    context.reset,
                    payload.get("info")
                ])
                .to_string();
                if self.codex_seen.insert(key) {
                    self.codex_tokens = self
                        .codex_tokens
                        .saturating_add(delta.input.saturating_add(delta.output));
                }
            }
            _ => {}
        }
    }

    fn scan_file(&mut self, path: PathBuf, len: u64) {
        let pos = self.files.entry(path.clone()).or_default();
        let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if len == pos.len && modified == pos.modified {
            return;
        }
        if len <= pos.len {
            // Truncated/rewritten: start over. Lines already counted stay in
            // `seen`, so a rewrite can't double-count.
            pos.offset = 0;
        }
        let Ok(mut file) = std::fs::File::open(&path) else {
            return;
        };
        if file.seek(SeekFrom::Start(pos.offset)).is_err() {
            return;
        }
        let mut offset = pos.offset;
        let mut reader = BufReader::new(file);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            let Ok(n) = reader.read_until(b'\n', &mut buf) else {
                break;
            };
            if n == 0 || buf.last() != Some(&b'\n') {
                // EOF, or a line still being appended — leave it for the next
                // scan so a half-written JSON object is never half-counted.
                break;
            }
            offset += n as u64;
            if let Ok(text) = std::str::from_utf8(&buf) {
                self.ingest_line(text);
            }
        }
        let pos = self.files.entry(path).or_default();
        pos.offset = offset;
        pos.len = len;
        pos.modified = modified;
    }

    /// Fold one transcript line into the tallies, if it is a deduplicated
    /// assistant message from today carrying usage.
    fn ingest_line(&mut self, text: &str) {
        let Ok(line) = serde_json::from_str::<Line>(text) else {
            return;
        };
        if line.kind.as_deref() != Some("assistant") {
            return;
        }
        let Some(msg) = line.message else { return };
        let Some(usage) = msg.usage else { return };
        let Some(model) = msg.model else { return };
        // API-error placeholder rows carry no real usage.
        if model == "<synthetic>" {
            return;
        }
        // Local-day filter: the timestamp is RFC 3339 (UTC).
        let today = line
            .timestamp
            .as_deref()
            .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
            .is_some_and(|t| t.with_timezone(&Local).date_naive() == self.day);
        if !today {
            return;
        }
        let total = usage
            .input_tokens
            .saturating_add(usage.output_tokens)
            .saturating_add(usage.cache_creation_input_tokens)
            .saturating_add(usage.cache_read_input_tokens);
        // Streaming rows can update the same message. Reconcile the maximum
        // observed count, so replayed early chunks cannot lower the total.
        let delta = if let Some(key) = msg.id.or(line.request_id) {
            let previous = self.seen.entry(key).or_default();
            let delta = total.saturating_sub(*previous);
            *previous = (*previous).max(total);
            delta
        } else {
            total
        };
        self.claude_tokens = self.claude_tokens.saturating_add(delta);
    }

    fn snapshot(&self, plan: Option<&PlanUsage>) -> UsageSnapshot {
        let codex_limits = self
            .codex_plan
            .as_ref()
            .map(|(_, v)| codex_limits(v, chrono::Utc::now().timestamp()))
            .unwrap_or_default();
        let mut agents = Vec::new();
        for (harness, tokens, limits) in [
            (
                "claude",
                self.claude_tokens,
                plan.map(|p| p.windows.clone()).unwrap_or_default(),
            ),
            ("codex", self.codex_tokens, codex_limits),
            ("opencode", self.opencode_tokens, Vec::new()),
        ] {
            if tokens == 0 && limits.is_empty() {
                continue;
            }
            let adapter = crate::harness::ADAPTERS.iter().find(|a| a.name == harness);
            agents.push(AgentUsage {
                harness,
                label: adapter
                    .map(|a| a.label)
                    .unwrap_or(crate::harness::CLAUDE_LABEL),
                badge: adapter
                    .map(|a| a.badge)
                    .unwrap_or(crate::harness::CLAUDE_BADGE),
                icon: Some(
                    adapter
                        .map(|a| a.icon)
                        .unwrap_or(crate::harness::CLAUDE_ICON),
                ),
                today_tokens: tokens,
                limits,
            });
        }
        UsageSnapshot {
            today_tokens: agents
                .iter()
                .fold(0u64, |sum, a| sum.saturating_add(a.today_tokens)),
            agents,
        }
    }
}

fn update_codex_context(v: &serde_json::Value, context: &mut CodexContext) {
    let payload = &v["payload"];
    match v["type"].as_str() {
        Some("session_meta") => {
            if let Some(id) = payload
                .get("id")
                .or_else(|| payload.get("session_id"))
                .and_then(|v| v.as_str())
            {
                if context.owner.is_empty() {
                    context.owner = id.to_owned();
                }
                context.session = id.to_owned();
                context.turn = None;
                context.inherited = id != context.owner || payload["forked_from_id"].is_string();
            }
        }
        Some("turn_context") => {
            context.turn = payload["turn_id"].as_str().map(str::to_owned);
        }
        Some("event_msg")
            if payload["type"] == "thread_settings_applied"
                && payload["thread_id"].as_str() == Some(context.owner.as_str()) =>
        {
            context.session.clone_from(&context.owner);
            context.inherited = false;
            context.turn = None;
        }
        _ => {}
    }
}

fn codex_limits(value: &serde_json::Value, now: i64) -> Vec<UsageLimit> {
    // Newer rollouts may include separate non-Codex limit buckets.
    if value
        .get("limit_id")
        .and_then(|v| v.as_str())
        .is_some_and(|id| id != "codex")
    {
        return Vec::new();
    }
    ["primary", "secondary"]
        .iter()
        .filter_map(|key| {
            let window = value.get(key)?;
            let pct = window.get("used_percent")?.as_f64()?;
            let minutes = window.get("window_minutes")?.as_u64()?;
            let resets_at = window.get("resets_at")?.as_i64()?;
            if !pct.is_finite() || !(0.0..=100.0).contains(&pct) || resets_at <= now {
                return None;
            }
            let reset = DateTime::from_timestamp(resets_at, 0)?.with_timezone(&Local);
            let label = match minutes {
                300 => "5h block".into(),
                10080 => "Week".into(),
                0 => return None,
                n => format!("{n}m window"),
            };
            Some(UsageLimit {
                label,
                used_percent: pct,
                reset: Some(
                    reset
                        .format(if minutes >= 1440 {
                            "%a %-I:%M %p"
                        } else {
                            "%-I:%M %p"
                        })
                        .to_string(),
                ),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Plan utilization (OAuth usage endpoint)

#[derive(Debug, Default)]
struct PlanUsage {
    windows: Vec<UsageLimit>,
}

#[derive(Deserialize)]
struct Credentials {
    #[serde(rename = "claudeAiOauth")]
    oauth: Option<OauthCreds>,
}

#[derive(Deserialize)]
struct OauthCreds {
    #[serde(rename = "accessToken")]
    access_token: String,
    /// Milliseconds since epoch.
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
}

#[derive(Deserialize)]
struct OauthUsageResp {
    five_hour: Option<OauthWindow>,
    seven_day: Option<OauthWindow>,
}

#[derive(Deserialize)]
struct OauthWindow {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

/// Claude Code's OAuth access token, unexpired, from wherever this install
/// keeps it: the credentials file, or the macOS keychain.
fn read_access_token() -> Option<String> {
    let creds = read_credentials_file().or_else(read_keychain_credentials)?;
    let oauth = creds.oauth?;
    if let Some(expires_at) = oauth.expires_at {
        // Treat "about to expire" as expired; never refresh the token
        // ourselves — rotating it would log Claude Code out.
        if expires_at <= (chrono::Utc::now().timestamp_millis() + 60_000) {
            return None;
        }
    }
    Some(oauth.access_token)
}

fn read_credentials_file() -> Option<Credentials> {
    let path = crate::harness::claude::credentials_path();
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// macOS keychain fallback. The first read may show the user a keychain
/// consent prompt for the broglow binary; if they deny it, don't ask again
/// for the lifetime of this process.
#[cfg(target_os = "macos")]
fn read_keychain_credentials() -> Option<Credentials> {
    use std::sync::atomic::{AtomicBool, Ordering};
    static DENIED: AtomicBool = AtomicBool::new(false);
    if DENIED.load(Ordering::Relaxed) {
        return None;
    }
    let out = Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        DENIED.store(true, Ordering::Relaxed);
        return None;
    }
    serde_json::from_slice(&out.stdout).ok()
}

#[cfg(not(target_os = "macos"))]
fn read_keychain_credentials() -> Option<Credentials> {
    None
}

/// GET the OAuth usage endpoint via the system curl (present on macOS and
/// Windows 10+; no HTTP stack worth adding a dependency tree for). The
/// Authorization header goes in over stdin so the token never appears in the
/// process list.
fn fetch_plan_usage() -> Option<PlanUsage> {
    let token = read_access_token()?;
    let mut cmd = Command::new("curl");
    cmd.args([
        "-sf",
        "--max-time",
        "10",
        "-H",
        "@-",
        "-H",
        "anthropic-beta: oauth-2025-04-20",
        "https://api.anthropic.com/api/oauth/usage",
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
    crate::spawn::configure_detached(&mut cmd);
    let mut child = cmd.spawn().ok()?;
    child
        .stdin
        .take()?
        .write_all(format!("Authorization: Bearer {token}\n").as_bytes())
        .ok()?;
    let out = child.wait_with_output().ok()?;
    if !out.status.success() {
        return None;
    }
    let resp: OauthUsageResp = serde_json::from_slice(&out.stdout).ok()?;
    let local = |iso: &str| {
        DateTime::parse_from_rfc3339(iso)
            .ok()
            .map(|t| t.with_timezone(&Local))
    };
    let mut plan = PlanUsage::default();
    for (label, window) in [("5h block", resp.five_hour), ("Week", resp.seven_day)] {
        let Some(w) = window else { continue };
        let Some(pct) = w
            .utilization
            .filter(|p| p.is_finite() && (0.0..=100.0).contains(p))
        else {
            continue;
        };
        let reset = w.resets_at.as_deref().and_then(local);
        if reset.is_some_and(|r| r <= Local::now()) {
            continue;
        }
        plan.windows.push(UsageLimit {
            label: label.into(),
            used_percent: pct,
            reset: reset.map(|r| {
                r.format(if label == "Week" {
                    "%a %-I:%M %p"
                } else {
                    "%-I:%M %p"
                })
                .to_string()
            }),
        });
    }
    (!plan.windows.is_empty()).then_some(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[cfg(unix)]
    #[test]
    fn scans_codex_without_claude_projects() {
        if std::env::var_os("BROGLOW_TEST_CODEX_USAGE").is_none() {
            let home = tempfile::tempdir().unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "usage::tests::scans_codex_without_claude_projects",
                ])
                .env("HOME", home.path())
                .env(crate::codex::HOME_ENV, home.path().join("codex"))
                .env("BROGLOW_TEST_CODEX_USAGE", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            return;
        }
        assert!(!projects_dir().unwrap().exists());
        let sessions = crate::codex::codex_home().unwrap().join("sessions");
        std::fs::create_dir_all(&sessions).unwrap();
        let mut tracker = Tracker::new();
        let context =
            serde_json::json!({"type": "turn_context", "payload": {"model": "gpt-5.6-sol"}});
        let usage = serde_json::json!({
            "timestamp": today_ts(&tracker), "type": "event_msg",
            "payload": {"type": "token_count", "info": {"total_token_usage": {
                "input_tokens": 100, "cached_input_tokens": 25,
                "output_tokens": 10, "total_tokens": 110
            }}}
        });
        std::fs::write(
            sessions.join("rollout.jsonl"),
            format!("{context}\n{usage}\n"),
        )
        .unwrap();
        tracker.scan();
        assert_eq!(tracker.snapshot(None).today_tokens, 110);
        tracker.scan();
        assert_eq!(tracker.snapshot(None).today_tokens, 110);
    }

    #[cfg(unix)]
    #[test]
    fn scan_includes_archives_and_subagents_and_resets_on_new_day() {
        if std::env::var_os("BROGLOW_TEST_MIXED_USAGE").is_none() {
            let home = tempfile::tempdir().unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "usage::tests::scan_includes_archives_and_subagents_and_resets_on_new_day",
                ])
                .env("HOME", home.path())
                .env(crate::codex::HOME_ENV, home.path().join("codex"))
                .env("BROGLOW_TEST_MIXED_USAGE", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
            return;
        }
        let mut t = Tracker::new();
        let root = projects_dir().unwrap().join("project");
        let sub = root.join("session/subagents");
        std::fs::create_dir_all(&sub).unwrap();
        let ts = today_ts(&t);
        std::fs::write(
            root.join("session.jsonl"),
            line("claude", "main", &ts, 50) + "\n",
        )
        .unwrap();
        std::fs::write(
            sub.join("subagent.jsonl"),
            line("claude", "subagent", &ts, 50) + "\n",
        )
        .unwrap();
        let archive = crate::codex::codex_home()
            .unwrap()
            .join("archived_sessions");
        std::fs::create_dir_all(&archive).unwrap();
        let event = serde_json::json!({"timestamp":ts,"type":"event_msg",
        "payload":{"type":"token_count","info":{"total_token_usage":{
            "input_tokens":100,"output_tokens":10,"total_tokens":110
        }}}})
        .to_string()
            + "\n";
        std::fs::write(archive.join("rollout.jsonl"), event).unwrap();
        let yesterday = Local::now() - chrono::Duration::days(1);
        let quota = serde_json::json!({"timestamp":yesterday.to_rfc3339(), "type":"event_msg",
        "payload":{"type":"token_count","info":null,"rate_limits":{"secondary":{
            "used_percent":53,"window_minutes":10080,"resets_at":chrono::Utc::now().timestamp() + 200000
        }}}});
        let old_path = archive.join("yesterday.jsonl");
        std::fs::write(&old_path, format!("{quota}\n")).unwrap();
        std::fs::File::open(&old_path)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new().set_modified(std::time::SystemTime::from(yesterday)),
            )
            .unwrap();
        t.scan();
        assert_eq!(t.snapshot(None).agents[1].limits[0].used_percent, 53.0);
        assert_eq!(t.snapshot(None).today_tokens, 2430);
        assert_eq!(t.snapshot(None).agents[0].today_tokens, 2320);
        assert_eq!(t.snapshot(None).agents[1].today_tokens, 110);
        t.scan();
        assert_eq!(t.snapshot(None).today_tokens, 2430);
        // Simulate the cached tracker being carried across local midnight.
        t.day = t.day.pred_opt().unwrap();
        t.scan();
        assert_eq!(t.snapshot(None).today_tokens, 2430);
    }

    fn line(model: &str, msg_id: &str, ts: &str, output: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{ts}","requestId":"req_{msg_id}","message":{{"id":"{msg_id}","model":"{model}","usage":{{"input_tokens":10,"output_tokens":{output},"cache_creation_input_tokens":100,"cache_read_input_tokens":1000,"cache_creation":{{"ephemeral_5m_input_tokens":100,"ephemeral_1h_input_tokens":0}}}}}}}}"#
        )
    }

    /// A timestamp guaranteed to be "today" for the tracker under test.
    fn today_ts(tracker: &Tracker) -> String {
        let noon = tracker.day.and_hms_opt(12, 0, 0).unwrap();
        Local
            .from_local_datetime(&noon)
            .single()
            .unwrap()
            .to_rfc3339()
    }

    #[test]
    fn counts_todays_assistant_lines_and_dedupes() {
        let mut t = Tracker::new();
        let ts = today_ts(&t);
        t.ingest_line(&line("claude-haiku-4-5-20251001", "msg_1", &ts, 50));
        // Same message copied into a resumed session's file: ignored.
        t.ingest_line(&line("claude-haiku-4-5-20251001", "msg_1", &ts, 50));
        t.ingest_line(&line("claude-haiku-4-5-20251001", "msg_2", &ts, 50));
        // Yesterday's line: ignored.
        t.ingest_line(&line(
            "claude-haiku-4-5-20251001",
            "msg_3",
            "2020-01-01T12:00:00Z",
            50,
        ));
        // Non-assistant and synthetic lines: ignored.
        t.ingest_line(r#"{"type":"user","message":{"role":"user"}}"#);
        t.ingest_line(&line("<synthetic>", "msg_4", &ts, 50));
        // Garbage must never panic or count.
        t.ingest_line("not json at all\n");

        // 2 × (10 in + 50 out + 100 cache write + 1000 cache read)
        assert_eq!(t.snapshot(None).today_tokens, 2 * 1160);
    }

    #[test]
    fn codex_rollouts_count_as_cumulative_deltas() {
        let mut t = Tracker::new();
        let ts = today_ts(&t);
        let count = |input: u64, cached: u64, output: u64, total: u64, ts: &str| {
            format!(
                r#"{{"timestamp":"{ts}","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":{input},"cached_input_tokens":{cached},"output_tokens":{output},"total_tokens":{total}}}}}}}}}"#
            )
        };
        let mut prev = CodexCumulative::default();

        t.ingest_codex_line(&count(100, 0, 1, 101, &ts), &mut prev);
        // Non-usage lines are ignored.
        t.ingest_codex_line(
            r#"{"type":"turn_context","payload":{"turn_id":"t1","model":"gpt-5.6-sol"}}"#,
            &mut prev,
        );
        // Cumulative counters: each event contributes its delta.
        t.ingest_codex_line(&count(14300, 9984, 5, 14305, &ts), &mut prev);
        t.ingest_codex_line(&count(20300, 14984, 505, 20805, &ts), &mut prev);
        // Yesterday's line advances the baseline without counting.
        t.ingest_codex_line(
            &count(30300, 14984, 505, 30805, "2020-01-01T12:00:00Z"),
            &mut prev,
        );
        // A reset (smaller cumulative) becomes a fresh delta, never underflow.
        t.ingest_codex_line(&count(50, 0, 10, 60, &ts), &mut prev);

        // 101, then (14300-100)+(5-1), then 6000+500, then the reset's 60.
        let expected_tokens = 101 + (14300 - 100) + (5 - 1) + (6000 + 500) + 60;
        assert_eq!(t.snapshot(None).today_tokens, expected_tokens);
    }

    #[test]
    fn updated_claude_message_reconciles_usage_instead_of_keeping_first_chunk() {
        let mut t = Tracker::new();
        let ts = today_ts(&t);
        t.ingest_line(&line("claude", "streamed", &ts, 1));
        t.ingest_line(&line("claude", "streamed", &ts, 50));
        t.ingest_line(&line("claude", "streamed", &ts, 1));
        assert_eq!(t.snapshot(None).today_tokens, 1160);
    }

    #[test]
    fn fork_inherited_history_is_not_new_usage_even_when_retimestamped_today() {
        let mut t = Tracker::new();
        let ts = today_ts(&t);
        let mut prev = CodexCumulative::default();
        let mut context = CodexContext::default();
        let count = |input, pct| {
            serde_json::json!({"timestamp":ts,"type":"event_msg",
            "payload":{"type":"token_count","info":{"total_token_usage":{
                "input_tokens":input,"output_tokens":10,"total_tokens":input+10
            }},"rate_limits":{"primary":{"used_percent":pct,"window_minutes":300,
                "resets_at":chrono::Utc::now().timestamp()+3600}}}})
            .to_string()
        };
        for row in [
            serde_json::json!({"type":"session_meta","payload":{"id":"child","forked_from_id":"parent"}}).to_string(),
            serde_json::json!({"type":"session_meta","payload":{"id":"parent"}}).to_string(),
            count(100, 99),
        ] { t.ingest_codex_record(&row, &mut prev, &mut context); }
        assert_eq!(t.snapshot(None).today_tokens, 0);
        assert!(t.snapshot(None).agents.is_empty());
        t.ingest_codex_record(
            &serde_json::json!({"type":"event_msg","payload":{
            "type":"thread_settings_applied","thread_id":"child"}})
            .to_string(),
            &mut prev,
            &mut context,
        );
        t.ingest_codex_record(&count(150, 20), &mut prev, &mut context);
        assert_eq!(t.snapshot(None).today_tokens, 50);
        assert_eq!(t.snapshot(None).agents[0].limits[0].used_percent, 20.0);
    }

    #[test]
    fn old_fork_limits_survive_an_ownership_marker_outside_the_tail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fork.jsonl");
        let mut t = Tracker::new();
        let yesterday = Local::now() - chrono::Duration::days(1);
        let meta = serde_json::json!({"type":"session_meta","payload":{"id":"child","forked_from_id":"parent"}});
        let owned = serde_json::json!({"type":"event_msg","payload":{"type":"thread_settings_applied","thread_id":"child"}});
        let large =
            serde_json::json!({"type":"response_item","payload":{"text":"x".repeat(1_100_000)}});
        let quota = serde_json::json!({"timestamp":yesterday.to_rfc3339(),"type":"event_msg","payload":{
            "type":"token_count","info":null,"rate_limits":{"secondary":{
                "used_percent":53,"window_minutes":10080,"resets_at":chrono::Utc::now().timestamp()+200000}}}});
        std::fs::write(&path, format!("{meta}\n{owned}\n{large}\n{quota}\n")).unwrap();
        t.scan_old_codex_limits(&path, &std::fs::metadata(&path).unwrap());
        assert_eq!(t.snapshot(None).agents[0].limits[0].used_percent, 53.0);
        assert_eq!(t.snapshot(None).today_tokens, 0);
    }

    #[test]
    fn independent_codex_sessions_with_identical_usage_are_both_counted() {
        let dir = tempfile::tempdir().unwrap();
        let mut t = Tracker::new();
        let event = serde_json::json!({"timestamp":today_ts(&t),"type":"event_msg",
        "payload":{"type":"token_count","info":{"total_token_usage":{
            "input_tokens":100,"output_tokens":10,"total_tokens":110
        }}}})
        .to_string();
        for id in ["first", "second"] {
            let path = dir.path().join(format!("{id}.jsonl"));
            let meta = serde_json::json!({"type":"session_meta","payload":{"id":id}});
            std::fs::write(&path, format!("{meta}\n{event}\n")).unwrap();
            t.scan_codex_file(path.clone(), std::fs::metadata(path).unwrap().len());
        }
        assert_eq!(t.snapshot(None).today_tokens, 220);
    }

    #[test]
    fn copied_and_truncated_codex_rollouts_do_not_double_count() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rollout.jsonl");
        let copy = dir.path().join("copy.jsonl");
        let mut t = Tracker::new();
        let event = serde_json::json!({"timestamp":today_ts(&t),"type":"event_msg",
        "payload":{"type":"token_count","info":{"total_token_usage":{
            "input_tokens":100,"output_tokens":10,"total_tokens":110
        }}}})
        .to_string()
            + "\n";
        let meta = "{\"type\":\"session_meta\",\"payload\":{\"id\":\"same-session\"}}\n";
        let event = format!("{meta}{event}");
        std::fs::write(&path, format!("{event}{{}}\n")).unwrap();
        t.scan_codex_file(path.clone(), std::fs::metadata(&path).unwrap().len());
        std::fs::write(&copy, &event).unwrap();
        t.scan_codex_file(copy.clone(), std::fs::metadata(&copy).unwrap().len());
        assert_eq!(t.snapshot(None).today_tokens, 110, "copied history");
        std::fs::write(&path, &event).unwrap();
        t.scan_codex_file(path.clone(), std::fs::metadata(&path).unwrap().len());
        assert_eq!(t.snapshot(None).today_tokens, 110, "truncated history");
    }

    #[test]
    fn mixed_agents_keep_tokens_and_limits_independent() {
        let mut t = Tracker::new();
        let ts = today_ts(&t);
        t.ingest_line(&line("claude", "mixed", &ts, 50));
        t.codex_tokens = 220;
        t.opencode_tokens = 330;
        let now = chrono::Utc::now().timestamp();
        let limits = serde_json::json!({"limit_id":"codex", "primary":{
            "used_percent":70,"window_minutes":300,"resets_at":now + 3600}});
        t.ingest_codex_line(
            &serde_json::json!({"timestamp":ts,"type":"event_msg",
            "payload":{"type":"token_count","info":null,"rate_limits":limits}})
            .to_string(),
            &mut CodexCumulative::default(),
        );
        let plan = PlanUsage {
            windows: vec![UsageLimit {
                label: "5h block".into(),
                used_percent: 10.0,
                reset: None,
            }],
        };
        let snap = t.snapshot(Some(&plan));
        assert_eq!(snap.today_tokens, 1710);
        assert_eq!(
            snap.agents.iter().map(|a| a.harness).collect::<Vec<_>>(),
            vec!["claude", "codex", "opencode"]
        );
        for agent in &snap.agents {
            let expected =
                crate::harness::agent((agent.harness != "claude").then_some(agent.harness));
            assert_eq!(agent.icon, expected.icon);
            assert_eq!(
                serde_json::to_value(agent).unwrap()["icon"].as_str(),
                expected.icon
            );
        }
        assert_eq!(snap.agents[0].today_tokens, 1160);
        assert_eq!(snap.agents[0].limits[0].used_percent, 10.0);
        assert_eq!(snap.agents[1].limits[0].used_percent, 70.0);
        assert!(snap.agents[2].limits.is_empty());
        assert_eq!(codex_limits(&limits, now + 3600).len(), 0);
        assert_eq!(codex_limits(&serde_json::Value::Null, now).len(), 0);
        assert_eq!(
            codex_limits(
                &serde_json::json!({"limit_id":"review", "primary":
            limits["primary"]}),
                now
            )
            .len(),
            0
        );
        assert_eq!(
            codex_limits(
                &serde_json::json!({"secondary":{
            "used_percent":15,"window_minutes":10080,"resets_at":now + 10000}}),
                now
            )[0]
            .label,
            "Week"
        );
    }

    #[test]
    fn codex_latest_limits_win_and_separate_api_sessions_preserve_them() {
        let mut t = Tracker::new();
        let now = chrono::Utc::now();
        let event = |ts: chrono::DateTime<chrono::Utc>, limits: serde_json::Value| {
            serde_json::json!({"timestamp":ts.to_rfc3339(), "type":"event_msg",
                "payload":{"type":"token_count","info":null,"rate_limits":limits}})
            .to_string()
        };
        let limits = |pct| {
            serde_json::json!({"primary":{
            "used_percent":pct,"window_minutes":300,"resets_at":now.timestamp() + 3600}})
        };
        let mut prev = CodexCumulative::default();
        t.ingest_codex_line(&event(now, limits(60)), &mut prev);
        t.ingest_codex_line(
            &event(now - chrono::Duration::seconds(1), limits(10)),
            &mut prev,
        );
        assert_eq!(t.snapshot(None).agents[0].limits[0].used_percent, 60.0);
        t.ingest_codex_line(
            &event(
                now + chrono::Duration::seconds(1),
                serde_json::json!({
            "limit_id":"review","primary":limits(90)["primary"]}),
            ),
            &mut prev,
        );
        assert_eq!(t.snapshot(None).agents[0].limits[0].used_percent, 60.0);
        t.ingest_codex_line(
            &event(now + chrono::Duration::seconds(2), serde_json::Value::Null),
            &mut prev,
        );
        assert_eq!(t.snapshot(None).agents[0].limits[0].used_percent, 60.0);
        let mut api_only = Tracker::new();
        api_only.ingest_codex_line(
            &event(now, serde_json::Value::Null),
            &mut CodexCumulative::default(),
        );
        assert!(api_only.snapshot(None).agents.is_empty());
    }

    #[test]
    fn scanners_retry_partial_lines_and_keep_independent_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("claude.jsonl");
        let mut t = Tracker::new();
        let row = line("claude", "partial", &today_ts(&t), 50);
        std::fs::write(&path, &row).unwrap();
        t.scan_file(path.clone(), row.len() as u64);
        assert_eq!(t.snapshot(None).today_tokens, 0);
        std::fs::write(&path, format!("{row}\n")).unwrap();
        t.scan_file(path.clone(), row.len() as u64 + 1);
        assert_eq!(t.snapshot(None).today_tokens, 1160);
        t.scan_file(path, row.len() as u64 + 1);
        assert_eq!(t.snapshot(None).today_tokens, 1160);
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn compact_token_counts() {
        assert_eq!(compact_tokens(950), "950");
        assert_eq!(compact_tokens(12_400), "12k");
        assert_eq!(compact_tokens(1_240_000), "1.2M");
    }
}
