//! Today's Claude Code usage — tokens and API-equivalent dollars — plus the
//! plan rate-limit percentages, feeding the tray readout and the popover's
//! usage section (designs 1a/1c in the Tray Menu Designs doc).
//!
//! Token counts come from the transcript JSONLs Claude Code writes under
//! `~/.claude/projects/<dir>/<session>.jsonl`: each assistant line carries the
//! request's token usage and model id. Scanning is incremental (a byte offset
//! per file, only files touched since local midnight), entries are deduped by
//! message/request id (resuming a session copies old lines into a new file),
//! and only lines stamped today in local time are counted.
//!
//! Plan-mode percentages (5-hour block / week) come from the same OAuth usage
//! endpoint the Claude Code `/usage` screen reads, using the credentials
//! Claude Code already stores. Everything here is best-effort: missing creds,
//! an unreachable endpoint, or an unknown response shape degrade to `None`
//! and the UI falls back to the token/$ line.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::sync::Mutex;

use chrono::{DateTime, Local, NaiveDate};
use serde::{Deserialize, Serialize};

/// Dollar cost is an estimate of what today's tokens would cost at API list
/// prices — for subscription users it is "API-equivalent", not a bill.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ModelUsage {
    /// Human name parsed from the model id, e.g. "Opus 4.5".
    pub name: String,
    /// Color family for the popover's segmented bar: opus/sonnet/haiku/other.
    pub family: &'static str,
    pub tokens: u64,
    pub cost: f64,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UsageSnapshot {
    pub today_tokens: u64,
    pub today_cost: f64,
    /// Per-model breakdown, largest cost first.
    pub models: Vec<ModelUsage>,
    /// Plan rate-limit utilization (0–100), when the OAuth endpoint is
    /// reachable with Claude Code's stored credentials.
    pub five_hour_pct: Option<f64>,
    /// Local reset time of the 5h block, preformatted ("4:00 PM").
    pub five_hour_reset: Option<String>,
    pub week_pct: Option<f64>,
    /// Weekday the weekly window resets ("Mon").
    pub week_reset: Option<String>,
}

/// Shared cache between the refresher thread and the tray/popover readers.
/// Only the tray daemon (macOS/Windows) uses it; the cross-platform `clawlight
/// usage` subcommand computes its snapshot directly, so gate it to those
/// platforms to avoid a dead-code error on Linux.
#[cfg(any(target_os = "macos", target_os = "windows"))]
static LATEST: Mutex<Option<UsageSnapshot>> = Mutex::new(None);

/// Most recent snapshot computed by the refresher thread.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn latest() -> Option<UsageSnapshot> {
    LATEST.lock().ok()?.clone()
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

/// One-shot snapshot for the hidden `clawlight usage` subcommand.
pub fn run_once() -> anyhow::Result<()> {
    let mut tracker = Tracker::new();
    tracker.scan();
    let snap = tracker.snapshot(fetch_plan_usage().as_ref());
    println!("{}", serde_json::to_string_pretty(&snap)?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Transcript scanning

/// Token counts split the way pricing splits them.
#[derive(Debug, Default, Clone, Copy)]
struct Tokens {
    input: u64,
    output: u64,
    /// Cache writes at the 5-minute TTL (1.25× input price).
    cache_5m: u64,
    /// Cache writes at the 1-hour TTL (2× input price).
    cache_1h: u64,
    cache_read: u64,
}

impl Tokens {
    fn total(&self) -> u64 {
        self.input + self.output + self.cache_5m + self.cache_1h + self.cache_read
    }

    fn add(&mut self, other: &Tokens) {
        self.input += other.input;
        self.output += other.output;
        self.cache_5m += other.cache_5m;
        self.cache_1h += other.cache_1h;
        self.cache_read += other.cache_read;
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct FilePos {
    offset: u64,
    len: u64,
}

/// Codex `token_count` events carry **cumulative** session totals, so the
/// per-file scan state must remember the last cumulative reading (usage per
/// event is the delta) and the current model from the latest `turn_context`.
#[derive(Debug, Default, Clone)]
struct CodexFilePos {
    offset: u64,
    len: u64,
    prev: CodexCumulative,
    model: Option<String>,
}

#[derive(Debug, Default, Clone, Copy)]
struct CodexCumulative {
    input: u64,
    cached: u64,
    output: u64,
    total: u64,
}

struct Tracker {
    day: NaiveDate,
    files: HashMap<PathBuf, FilePos>,
    /// Codex rollout files (`$CODEX_HOME/sessions/YYYY/MM/DD/*.jsonl`).
    codex_files: HashMap<PathBuf, CodexFilePos>,
    /// message id / request id already counted today, across all files.
    seen: HashSet<String>,
    /// model id → today's tokens.
    tallies: HashMap<String, Tokens>,
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
    cache_creation: Option<CacheCreation>,
}

#[derive(Deserialize)]
struct CacheCreation {
    #[serde(default)]
    ephemeral_5m_input_tokens: u64,
    #[serde(default)]
    ephemeral_1h_input_tokens: u64,
}

fn projects_dir() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".claude").join("projects"))
}

/// Recursively collect `*.jsonl` under `dir`, bounded to `depth` levels —
/// Codex's fixed `YYYY/MM/DD` layout needs no walkdir dependency.
fn collect_jsonl(dir: &std::path::Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
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
            seen: HashSet::new(),
            tallies: HashMap::new(),
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
            self.tallies.clear();
        }

        let Some(root) = projects_dir() else { return };
        let Ok(projects) = std::fs::read_dir(&root) else {
            return;
        };
        // Session files last written before local midnight can't contain
        // today's entries (lines are appended as they happen).
        let midnight = today
            .and_hms_opt(0, 0, 0)
            .and_then(|t| t.and_local_timezone(Local).single());

        for project in projects.flatten() {
            let Ok(sessions) = std::fs::read_dir(project.path()) else {
                continue;
            };
            for entry in sessions.flatten() {
                let path = entry.path();
                if path.extension().is_none_or(|e| e != "jsonl") {
                    continue;
                }
                let Ok(meta) = entry.metadata() else { continue };
                if let (Some(midnight), Ok(mtime)) = (midnight, meta.modified()) {
                    if DateTime::<Local>::from(mtime) < midnight {
                        continue;
                    }
                }
                self.scan_file(path, meta.len());
            }
        }

        self.scan_codex(midnight);
    }

    /// Fold new Codex rollout lines into today's tallies. Same incremental
    /// per-file scan as the Claude side; the layout is a fixed
    /// `sessions/YYYY/MM/DD/rollout-*.jsonl` tree, walked with the same
    /// before-midnight mtime skip.
    fn scan_codex(&mut self, midnight: Option<DateTime<Local>>) {
        let Some(root) = crate::codex::codex_home().map(|h| h.join("sessions")) else {
            return;
        };
        let mut files = Vec::new();
        collect_jsonl(&root, 4, &mut files);
        for path in files {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if let (Some(midnight), Ok(mtime)) = (midnight, meta.modified()) {
                if DateTime::<Local>::from(mtime) < midnight {
                    continue;
                }
            }
            self.scan_codex_file(path, meta.len());
        }
    }

    fn scan_codex_file(&mut self, path: PathBuf, len: u64) {
        let pos = self.codex_files.entry(path.clone()).or_default();
        if len == pos.len {
            return;
        }
        if len < pos.offset {
            // Truncated/rewritten: start over with fresh counters.
            *pos = CodexFilePos::default();
        }
        // Work on locals across the read loop; written back at the end (the
        // borrow checker won't allow holding the map entry across ingest).
        let mut offset = pos.offset;
        let mut prev = pos.prev;
        let mut model = pos.model.clone();
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
                self.ingest_codex_line(text, &mut prev, &mut model);
            }
        }
        let pos = self.codex_files.entry(path).or_default();
        pos.offset = offset;
        pos.len = len;
        pos.prev = prev;
        pos.model = model;
    }

    /// Fold one rollout line into the tallies. `token_count` events carry
    /// cumulative totals: usage per event is the delta from the previous one,
    /// and a decrease means the counter reset (the current value is the new
    /// delta). `cached_input_tokens` is a subset of `input_tokens`;
    /// `output_tokens` already includes reasoning tokens.
    fn ingest_codex_line(
        &mut self,
        text: &str,
        prev: &mut CodexCumulative,
        model: &mut Option<String>,
    ) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else {
            return;
        };
        let Some(payload) = v.get("payload") else {
            return;
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("turn_context") => {
                if let Some(m) = payload.get("model").and_then(|m| m.as_str()) {
                    *model = Some(m.to_string());
                }
            }
            Some("event_msg")
                if payload.get("type").and_then(|t| t.as_str()) == Some("token_count") =>
            {
                let Some(total) = payload.get("info").and_then(|i| i.get("total_token_usage"))
                else {
                    return;
                };
                let n = |k: &str| total.get(k).and_then(|x| x.as_u64()).unwrap_or(0);
                let cur = CodexCumulative {
                    input: n("input_tokens"),
                    cached: n("cached_input_tokens"),
                    output: n("output_tokens"),
                    total: n("total_tokens"),
                };
                let delta = if cur.total < prev.total {
                    cur // counter reset: current value is the fresh delta
                } else {
                    CodexCumulative {
                        input: cur.input.saturating_sub(prev.input),
                        cached: cur.cached.saturating_sub(prev.cached),
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
                if !today {
                    return;
                }
                // No turn_context yet (shouldn't happen mid-rollout): skip
                // rather than misattribute to a guessed model.
                let Some(model) = model.clone() else { return };
                self.tallies.entry(model).or_default().add(&Tokens {
                    input: delta.input.saturating_sub(delta.cached),
                    output: delta.output,
                    cache_5m: 0,
                    cache_1h: 0,
                    cache_read: delta.cached,
                });
            }
            _ => {}
        }
    }

    fn scan_file(&mut self, path: PathBuf, len: u64) {
        let pos = self.files.entry(path.clone()).or_default();
        if len == pos.len {
            return;
        }
        if len < pos.offset {
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
        // Resumed sessions copy prior lines into the new session file — count
        // each message once. Prefer the message id (stable across copies).
        if let Some(key) = msg.id.or(line.request_id) {
            if !self.seen.insert(key) {
                return;
            }
        }

        let (cache_5m, cache_1h) = match &usage.cache_creation {
            Some(c) => (c.ephemeral_5m_input_tokens, c.ephemeral_1h_input_tokens),
            // Older transcripts have only the total; assume the default 5m TTL.
            None => (usage.cache_creation_input_tokens, 0),
        };
        self.tallies.entry(model).or_default().add(&Tokens {
            input: usage.input_tokens,
            output: usage.output_tokens,
            cache_5m,
            cache_1h,
            cache_read: usage.cache_read_input_tokens,
        });
    }

    fn snapshot(&self, plan: Option<&PlanUsage>) -> UsageSnapshot {
        // Merge model ids that render to the same display name (date-suffixed
        // ids of one family+version), costing each id at its own rates.
        let mut by_name: HashMap<String, ModelUsage> = HashMap::new();
        for (model, tokens) in &self.tallies {
            let cost = rates_for(model).cost(tokens);
            let entry = by_name
                .entry(model_display_name(model))
                .or_insert_with_key(|name| ModelUsage {
                    name: name.clone(),
                    family: model_family(model),
                    tokens: 0,
                    cost: 0.0,
                });
            entry.tokens += tokens.total();
            entry.cost += cost;
        }
        let mut models: Vec<ModelUsage> = by_name.into_values().collect();
        models.sort_by(|a, b| b.cost.total_cmp(&a.cost).then(b.tokens.cmp(&a.tokens)));

        UsageSnapshot {
            today_tokens: models.iter().map(|m| m.tokens).sum(),
            today_cost: models.iter().map(|m| m.cost).sum(),
            models,
            five_hour_pct: plan.and_then(|p| p.five_hour_pct),
            five_hour_reset: plan.and_then(|p| p.five_hour_reset.clone()),
            week_pct: plan.and_then(|p| p.week_pct),
            week_reset: plan.and_then(|p| p.week_reset.clone()),
        }
    }
}

// ---------------------------------------------------------------------------
// Pricing

/// API list prices in $ per million tokens.
struct Rates {
    input: f64,
    output: f64,
    cache_read: f64,
}

impl Rates {
    fn cost(&self, t: &Tokens) -> f64 {
        (t.input as f64 * self.input
            // Cache writes are priced off the input rate: 1.25× (5m TTL) / 2× (1h).
            + t.cache_5m as f64 * self.input * 1.25
            + t.cache_1h as f64 * self.input * 2.0
            + t.cache_read as f64 * self.cache_read
            + t.output as f64 * self.output)
            / 1_000_000.0
    }
}

const OPUS_45: Rates = Rates {
    input: 5.0,
    output: 25.0,
    cache_read: 0.50,
};
const OPUS_LEGACY: Rates = Rates {
    input: 15.0,
    output: 75.0,
    cache_read: 1.50,
};
const SONNET: Rates = Rates {
    input: 3.0,
    output: 15.0,
    cache_read: 0.30,
};
const HAIKU_45: Rates = Rates {
    input: 1.0,
    output: 5.0,
    cache_read: 0.10,
};
const HAIKU_35: Rates = Rates {
    input: 0.80,
    output: 4.0,
    cache_read: 0.08,
};
const HAIKU_3: Rates = Rates {
    input: 0.25,
    output: 1.25,
    cache_read: 0.03,
};
// OpenAI (Codex) — GPT-5.6 GA pricing; cached input is 10% of input, and
// Codex reports no separate cache-write figure (those fields stay 0).
const GPT56_SOL: Rates = Rates {
    input: 5.0,
    output: 30.0,
    cache_read: 0.50,
};
const GPT56_TERRA: Rates = Rates {
    input: 2.5,
    output: 15.0,
    cache_read: 0.25,
};
const GPT56_LUNA: Rates = Rates {
    input: 1.0,
    output: 6.0,
    cache_read: 0.10,
};
const GPT5: Rates = Rates {
    input: 1.25,
    output: 10.0,
    cache_read: 0.125,
};

/// Best-effort list-price lookup by model id. Unknown ids fall back to their
/// family's latest rates (or Sonnet's) — the readout is an estimate either
/// way, and new model releases shouldn't zero the meter.
fn rates_for(model: &str) -> Rates {
    let m = model.to_ascii_lowercase();
    if m.contains("haiku") {
        if m.contains("haiku-3-5") || m.contains("3-5-haiku") {
            HAIKU_35
        } else if m.contains("haiku-3") || m.contains("3-haiku") {
            HAIKU_3
        } else {
            HAIKU_45
        }
    } else if m.contains("opus") {
        // Opus dropped to $5/$25 at 4.5; earlier 3/4/4.1 were $15/$75.
        match opus_minor(&m) {
            Some(minor) if minor >= 5 => OPUS_45,
            Some(_) => OPUS_LEGACY,
            // Plain opus-4 (date-only suffix) and opus-3 predate the reprice.
            None if m.contains("opus-4") || m.contains("opus-3") || m.contains("3-opus") => {
                OPUS_LEGACY
            }
            // Unversioned or 5.x+: current pricing.
            None => OPUS_45,
        }
    } else if m.contains("fable") || m.contains("mythos") {
        // Claude 5 tier — no published transcript pricing pinned here yet;
        // price at the top Opus tier so the estimate errs recognizably.
        OPUS_45
    } else if m.contains("gpt") {
        // Codex models. Unknown GPT ids estimate at the older flagship rate.
        if m.contains("5.6-sol") {
            GPT56_SOL
        } else if m.contains("5.6-terra") {
            GPT56_TERRA
        } else if m.contains("5.6-luna") {
            GPT56_LUNA
        } else {
            GPT5
        }
    } else {
        SONNET
    }
}

/// The x in "opus-4-x". `None` for opus-3 ids, unversioned ids, or 5.x+ ids.
fn opus_minor(model: &str) -> Option<u32> {
    let rest = model.split("opus-4-").nth(1)?;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    // An 8-digit run is the date suffix of a plain "opus-4" id, not a minor.
    if digits.is_empty() || digits.len() >= 8 {
        return None;
    }
    digits.parse().ok()
}

/// Bar color family for a model id.
fn model_family(model: &str) -> &'static str {
    let m = model.to_ascii_lowercase();
    for family in ["opus", "sonnet", "haiku"] {
        if m.contains(family) {
            return family;
        }
    }
    if m.contains("fable") || m.contains("mythos") {
        return "opus";
    }
    "other"
}

/// "claude-opus-4-5-20251101" → "Opus 4.5"; "claude-3-5-sonnet-20241022" →
/// "Sonnet 3.5"; "claude-fable-5" → "Fable 5". The family is the first
/// alphabetic segment, the version is the numeric segments around it, and an
/// 8-digit date segment ends the id.
fn model_display_name(model: &str) -> String {
    // OpenAI ids don't follow the claude segment grammar: "gpt-5.6-sol" →
    // "GPT-5.6 Sol", "gpt-5.1" → "GPT-5.1".
    if let Some(rest) = model.strip_prefix("gpt-") {
        let mut parts = rest.splitn(2, '-');
        let version = parts.next().unwrap_or(rest);
        let variant = parts.next().map(|v| {
            let mut chars = v.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        });
        return match variant {
            Some(v) if !v.is_empty() => format!("GPT-{version} {v}"),
            _ => format!("GPT-{version}"),
        };
    }

    let mut family: Option<&str> = None;
    let mut version: Vec<&str> = Vec::new();
    for seg in model.split('-') {
        if seg.eq_ignore_ascii_case("claude") {
            continue;
        }
        if seg.chars().all(|c| c.is_ascii_digit()) {
            if seg.len() >= 8 {
                break; // date suffix
            }
            version.push(seg);
        } else if family.is_none() {
            family = Some(seg);
        }
    }
    let Some(family) = family else {
        return model.to_string();
    };
    let mut chars = family.chars();
    let mut name = match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => return model.to_string(),
    };
    if !version.is_empty() {
        name.push(' ');
        name.push_str(&version.join("."));
    }
    name
}

// ---------------------------------------------------------------------------
// Plan utilization (OAuth usage endpoint)

#[derive(Debug, Default)]
struct PlanUsage {
    five_hour_pct: Option<f64>,
    five_hour_reset: Option<String>,
    week_pct: Option<f64>,
    week_reset: Option<String>,
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
    let path = dirs::home_dir()?.join(".claude").join(".credentials.json");
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// macOS keychain fallback. The first read may show the user a keychain
/// consent prompt for the clawlight binary; if they deny it, don't ask again
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
    if let Some(w) = resp.five_hour {
        plan.five_hour_pct = w.utilization;
        plan.five_hour_reset = w
            .resets_at
            .as_deref()
            .and_then(local)
            .map(|t| t.format("%-I:%M %p").to_string());
    }
    if let Some(w) = resp.seven_day {
        plan.week_pct = w.utilization;
        plan.week_reset = w
            .resets_at
            .as_deref()
            .and_then(local)
            .map(|t| t.format("%a").to_string());
    }
    (plan.five_hour_pct.is_some() || plan.week_pct.is_some()).then_some(plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

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

        let snap = t.snapshot(None);
        assert_eq!(snap.models.len(), 1);
        assert_eq!(snap.models[0].name, "Haiku 4.5");
        assert_eq!(snap.models[0].family, "haiku");
        // 2 × (10 in + 50 out + 100 cache5m + 1000 read)
        assert_eq!(snap.today_tokens, 2 * 1160);
        // 2 × (10·1.0 + 100·1.25 + 1000·0.10 + 50·5.0) / 1e6
        let expected = 2.0 * (10.0 + 125.0 + 100.0 + 250.0) / 1e6;
        assert!((snap.today_cost - expected).abs() < 1e-12);
    }

    #[test]
    fn merges_model_ids_by_display_name() {
        let mut t = Tracker::new();
        let ts = today_ts(&t);
        t.ingest_line(&line("claude-opus-4-5-20251101", "msg_a", &ts, 10));
        t.ingest_line(&line("claude-opus-4-5", "msg_b", &ts, 10));
        t.ingest_line(&line("claude-haiku-4-5-20251001", "msg_c", &ts, 10));
        let snap = t.snapshot(None);
        assert_eq!(snap.models.len(), 2);
        assert!(snap.models.iter().any(|m| m.name == "Opus 4.5"));
    }

    #[test]
    fn display_names() {
        assert_eq!(model_display_name("claude-opus-4-5-20251101"), "Opus 4.5");
        assert_eq!(model_display_name("claude-sonnet-4-20250514"), "Sonnet 4");
        assert_eq!(
            model_display_name("claude-3-5-sonnet-20241022"),
            "Sonnet 3.5"
        );
        assert_eq!(model_display_name("claude-haiku-4-5-20251001"), "Haiku 4.5");
        assert_eq!(model_display_name("claude-fable-5"), "Fable 5");
        assert_eq!(model_display_name("claude-opus-4-8"), "Opus 4.8");
    }

    #[test]
    fn pricing_tiers() {
        // Opus repriced at 4.5; 4/4.1 keep legacy rates, 4.8 is current.
        assert_eq!(rates_for("claude-opus-4-5-20251101").input, 5.0);
        assert_eq!(rates_for("claude-opus-4-8").input, 5.0);
        assert_eq!(rates_for("claude-opus-4-1-20250805").input, 15.0);
        assert_eq!(rates_for("claude-opus-4-20250514").input, 15.0);
        assert_eq!(rates_for("claude-3-opus-20240229").input, 15.0);
        assert_eq!(rates_for("claude-3-5-haiku-20241022").input, 0.80);
        assert_eq!(rates_for("claude-haiku-4-5-20251001").input, 1.0);
        assert_eq!(rates_for("claude-sonnet-4-5-20250929").input, 3.0);
        // Unknown ids estimate rather than zero out.
        assert_eq!(rates_for("claude-fable-5").input, 5.0);
        assert_eq!(rates_for("some-future-model").input, 3.0);
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
        let mut model = None;

        // A token_count before any turn_context is skipped, but still
        // advances the cumulative baseline.
        t.ingest_codex_line(&count(100, 0, 1, 101, &ts), &mut prev, &mut model);
        assert!(t.tallies.is_empty());

        t.ingest_codex_line(
            r#"{"type":"turn_context","payload":{"turn_id":"t1","model":"gpt-5.6-sol"}}"#,
            &mut prev,
            &mut model,
        );
        // Cumulative counters: each event contributes its delta.
        t.ingest_codex_line(&count(14300, 9984, 5, 14305, &ts), &mut prev, &mut model);
        t.ingest_codex_line(&count(20300, 14984, 505, 20805, &ts), &mut prev, &mut model);
        // Yesterday's line advances the baseline without counting.
        t.ingest_codex_line(
            &count(30300, 14984, 505, 30805, "2020-01-01T12:00:00Z"),
            &mut prev,
            &mut model,
        );
        // A reset (smaller cumulative) becomes a fresh delta, never underflow.
        t.ingest_codex_line(&count(50, 0, 10, 60, &ts), &mut prev, &mut model);

        let snap = t.snapshot(None);
        assert_eq!(snap.models.len(), 1);
        assert_eq!(snap.models[0].name, "GPT-5.6 Sol");
        assert_eq!(snap.models[0].family, "other");
        // Event 1: 14305 (delta from 101 baseline: 14204 in + 4 out… the
        // baseline event itself contributed 101 to prev, so deltas are
        // (14300-100)+(5-1) then 6000+500, then the reset's 60.
        let expected_tokens = (14300 - 100) + (5 - 1) + (6000 + 500) + 60;
        assert_eq!(snap.today_tokens, expected_tokens as u64);
        // Cost at Sol rates: uncached input ×$5, cached ×$0.50, output ×$30.
        let uncached = (14200 - 9984) as f64 + 1000.0 + 50.0;
        let cached = 9984.0 + 5000.0;
        let output = 4.0 + 500.0 + 10.0;
        let expected = (uncached * 5.0 + cached * 0.5 + output * 30.0) / 1e6;
        assert!(
            (snap.today_cost - expected).abs() < 1e-9,
            "cost {} != {expected}",
            snap.today_cost
        );
    }

    #[test]
    fn gpt_display_names_and_rates() {
        assert_eq!(model_display_name("gpt-5.6-sol"), "GPT-5.6 Sol");
        assert_eq!(model_display_name("gpt-5.6-terra"), "GPT-5.6 Terra");
        assert_eq!(model_display_name("gpt-5.1"), "GPT-5.1");
        assert_eq!(model_family("gpt-5.6-sol"), "other");
        assert_eq!(rates_for("gpt-5.6-sol").input, 5.0);
        assert_eq!(rates_for("gpt-5.6-sol").output, 30.0);
        assert_eq!(rates_for("gpt-5.6-luna").input, 1.0);
        // Unknown GPT ids estimate at the older flagship rate, not Sonnet's.
        assert_eq!(rates_for("gpt-5.2-codex").input, 1.25);
    }

    #[test]
    fn oauth_response_parses_defensively() {
        let resp: OauthUsageResp = serde_json::from_str(
            r#"{"five_hour":{"utilization":62,"resets_at":"2026-07-04T23:00:00Z"},"seven_day":{"utilization":31,"resets_at":"2026-07-06T07:00:00Z"},"unknown_window":{"foo":1}}"#,
        )
        .unwrap();
        assert_eq!(resp.five_hour.unwrap().utilization, Some(62.0));
        assert_eq!(resp.seven_day.unwrap().utilization, Some(31.0));
        // Shape drift must degrade, not error, at the callsite (Option).
        let odd: OauthUsageResp = serde_json::from_str(r#"{"five_hour":{}}"#).unwrap();
        assert_eq!(odd.five_hour.unwrap().utilization, None);
    }
}
