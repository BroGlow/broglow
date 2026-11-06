//! T3 Code's v2 projections are the source of truth; this adapter never writes
//! its database or configuration. A deferred read transaction includes WAL data.

#[path = "t3_remote.rs"]
mod remote;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::state::{HookState, SessionStatus, Status, TerminalInfo};

const POLL_INTERVAL: Duration = Duration::from_secs(2);
const MAX_THREADS: usize = 2048;
const MAX_NATIVE_SESSIONS: usize = 16384;
const PREFIX: &str = "orchestration_v2_projection_";

/// Kept independently of displayed sessions so dismissal and retention survive
/// polls and app restarts. A thread rename is deliberately not activity.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Tracking {
    #[serde(default)]
    pub source: String,
    pub activity: String,
    pub dismissed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offline_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provider_sessions: Vec<ProviderSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ProviderSession {
    pub harness: String,
    pub id: String,
}

#[derive(Clone)]
struct Source {
    home: PathBuf,
    state_dir: &'static str,
    key: String,
    bundle: &'static str,
    owner_pid: Option<u32>,
}

impl Source {
    fn new(home: PathBuf) -> Self {
        let canonical = home.canonicalize().unwrap_or_else(|_| home.clone());
        // Stable across processes and Rust upgrades, unlike DefaultHasher.
        let key = stable_hash(canonical.to_string_lossy().as_bytes());
        let bundle = if home.file_name().is_some_and(|name| name == ".t3-yandex") {
            "ru.yandex.t3code"
        } else {
            "com.t3tools.t3code"
        };
        Self {
            home,
            state_dir: "userdata",
            key: format!("t3code:{key:016x}:"),
            bundle,
            owner_pid: None,
        }
    }

    fn data(&self) -> PathBuf {
        self.home.join(self.state_dir)
    }

    fn development(mut self) -> Self {
        self.state_dir = "dev";
        let data = self.data().canonicalize().unwrap_or_else(|_| self.data());
        self.key = format!(
            "t3code:{:016x}:",
            stable_hash(data.to_string_lossy().as_bytes())
        );
        self.bundle = "com.t3tools.t3code.dev";
        self
    }
}

fn sources() -> Vec<Source> {
    let mut paths = Vec::new();
    if let Some(home) = std::env::var_os("T3CODE_HOME").filter(|h| !h.is_empty()) {
        return vec![Source::new(PathBuf::from(home))];
    }
    if let Some(home) = dirs::home_dir() {
        paths.extend([home.join(".t3-yandex"), home.join(".t3")]);
    }
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .map(Source::new)
        .flat_map(|source| [source.clone(), source.development()])
        .filter(|source| seen.insert(source.key.clone()))
        .collect()
}

pub fn detected() -> bool {
    sources()
        .iter()
        .any(|source| source.data().join("statev2.sqlite").is_file())
}

/// T3 needs no hooks, plugins, or changes to its settings.
pub fn install() -> anyhow::Result<()> {
    Ok(())
}

pub fn uninstall() {}

#[derive(Deserialize)]
struct RuntimeState {
    version: u32,
    pid: u32,
    #[serde(rename = "serverVersion")]
    server_version: Option<String>,
}

fn runtime(source: &Source) -> anyhow::Result<Option<RuntimeState>> {
    let bytes = match std::fs::read(source.data().join("server-runtime.json")) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let runtime: RuntimeState = serde_json::from_slice(&bytes)?;
    anyhow::ensure!(
        runtime.version == 1 && runtime.pid > 0,
        "unknown T3 runtime state"
    );
    Ok(crate::terminal::is_alive(runtime.pid).then_some(runtime))
}

#[derive(Clone, Default)]
struct Snapshot {
    source: String,
    online: bool,
    threads: Vec<Thread>,
    provider_sessions: Vec<ProviderSession>,
}

#[derive(Clone)]
struct Thread {
    id: String,
    activity: String,
    session: SessionStatus,
}

#[derive(Default)]
struct Cache {
    last_attempt: Option<Instant>,
    sources: HashMap<String, CachedSource>,
}

struct CachedSource {
    stamp: Vec<Option<(u64, SystemTime)>>,
    snapshot: Snapshot,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
static POLLING: AtomicBool = AtomicBool::new(false);

fn file_stamp(path: &Path) -> Option<(u64, SystemTime)> {
    let metadata = std::fs::metadata(path).ok()?;
    Some((metadata.len(), metadata.modified().ok()?))
}

/// Readers may run on several tray/LED threads. Neither the polling mutex nor
/// the shared state lock is ever waited on; skipped state writes retry later.
pub fn refresh() {
    remote::refresh();
    if POLLING.load(Ordering::Relaxed) {
        return;
    }
    let Ok(guard) = CACHE.try_lock() else {
        return;
    };
    if guard.as_ref().is_some_and(|cache| {
        cache
            .last_attempt
            .is_some_and(|last| last.elapsed() < POLL_INTERVAL)
    }) {
        return;
    }
    drop(guard);
    if POLLING
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return;
    }
    if std::thread::Builder::new()
        .name("broglow-t3".into())
        .spawn(|| {
            poll_sources();
            POLLING.store(false, Ordering::Relaxed);
        })
        .is_err()
    {
        POLLING.store(false, Ordering::Relaxed);
    }
}

fn poll_sources() {
    let Ok(mut guard) = CACHE.try_lock() else {
        return;
    };
    let cache = guard.get_or_insert_with(Cache::default);
    if cache
        .last_attempt
        .is_some_and(|last| last.elapsed() < POLL_INTERVAL)
    {
        return;
    }
    cache.last_attempt = Some(Instant::now());
    let mut snapshots = Vec::new();
    let mut queried = Vec::new();
    let selected = sources();
    let selected_keys: Vec<_> = selected.iter().map(|source| source.key.clone()).collect();
    cache.sources.retain(|key, _| selected_keys.contains(key));
    for mut source in selected {
        let runtime = match runtime(&source) {
            Ok(Some(runtime)) => runtime,
            Ok(None) => {
                snapshots.push(Snapshot {
                    source: source.key,
                    ..Default::default()
                });
                continue;
            }
            Err(_) => continue,
        };
        source.owner_pid = Some(runtime.pid);
        if source.state_dir != "dev"
            && runtime
                .server_version
                .as_deref()
                .is_some_and(|version| version.contains("yandex"))
        {
            source.bundle = "ru.yandex.t3code";
        }
        let stamp = [
            "statev2.sqlite",
            "statev2.sqlite-wal",
            "environment-id",
            "server-runtime.json",
        ]
        .map(|file| file_stamp(&source.data().join(file)))
        .to_vec();
        if let Some(cached) = cache.sources.get(&source.key) {
            if cached.stamp == stamp {
                snapshots.push(cached.snapshot.clone());
                continue;
            }
        }
        // Missing, corrupt, busy and unknown schemas leave the last good state.
        if let Ok(snapshot) = read_source(&source) {
            queried.push((source.key, stamp, snapshot.clone()));
            snapshots.push(snapshot);
        }
    }
    if snapshots
        .iter()
        .all(|snapshot| snapshot.threads.is_empty() && snapshot.provider_sessions.is_empty())
        && !std::fs::read(crate::state::state_file_path())
            .ok()
            .and_then(|bytes| serde_json::from_slice::<HookState>(&bytes).ok())
            .is_some_and(|state| !state.t3_threads.is_empty())
    {
        return;
    }
    let mut applied = false;
    let result = crate::hook::try_update_state(|state| {
        applied = true;
        let retired = retire_unselected_sources(state, &selected_keys);
        apply_snapshots(state, &snapshots) || retired
    });
    if applied && result.is_ok() {
        for (key, stamp, snapshot) in queried {
            cache.sources.insert(key, CachedSource { stamp, snapshot });
        }
    }
}

fn read_source(source: &Source) -> anyhow::Result<Snapshot> {
    let path = source.data().join("statev2.sqlite");
    let mut connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(Duration::ZERO)?;
    let transaction = connection.transaction()?;
    let snapshot = read_snapshot(&transaction, source)?;
    transaction.rollback()?;
    Ok(snapshot)
}

fn read_snapshot(connection: &Connection, source: &Source) -> anyhow::Result<Snapshot> {
    let environment = std::fs::read_to_string(source.data().join("environment-id"))
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| plain_id(s));
    let mut snapshot = Snapshot {
        source: source.key.clone(),
        online: true,
        provider_sessions: native_sessions(connection)?,
        ..Default::default()
    };
    let sql = format!(
        "SELECT t.thread_id, t.title, t.created_at, p.workspace_root,
                json_extract(t.payload_json, '$.worktreePath')
         FROM {PREFIX}threads t
         JOIN projection_projects p ON p.project_id = t.project_id
         WHERE t.archived_at IS NULL AND t.deleted_at IS NULL
           AND p.archived_at IS NULL AND p.deleted_at IS NULL
           AND COALESCE(json_extract(t.payload_json, '$.lineage.relationshipToParent'), '') != 'subagent'
           AND COALESCE(json_extract(t.payload_json, '$.lineage.sideChat'), 0) != 1
         ORDER BY t.updated_at DESC LIMIT ?1"
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([(MAX_THREADS + 1) as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    for row in rows {
        anyhow::ensure!(snapshot.threads.len() < MAX_THREADS, "too many T3 threads");
        let (thread_id, title, created_at, project, worktree) = row?;
        let facts = thread_facts(connection, &thread_id)?;
        let (status, notification_type) = facts.status();
        let last_updated = facts.last_updated(&created_at);
        let terminal = Some(TerminalInfo {
            bundle_id: Some(source.bundle.to_owned()),
            // A live T3 server keeps long-running work from the 24-hour stale reap.
            owner_pid: source
                .owner_pid
                .filter(|_| matches!(status, Status::Active | Status::NeedsHelp)),
            host_session_id: environment
                .as_ref()
                .filter(|_| valid_thread_id(&thread_id))
                .map(|env| format!("{env}/{thread_id}")),
            ..Default::default()
        });
        snapshot.threads.push(Thread {
            id: format!("{}{thread_id}", source.key),
            activity: facts.activity(),
            session: SessionStatus {
                status,
                last_updated,
                project_path: worktree.filter(|s| !s.is_empty()).or(project),
                notification_type,
                name: (!title.trim().is_empty()).then_some(title),
                terminal,
                harness: Some("t3code".to_owned()),
                codex_background: false,
                t3_hosted: None,
            },
        });
    }
    Ok(snapshot)
}

fn plain_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

pub(crate) fn valid_thread_id(id: &str) -> bool {
    !id.is_empty() && id.trim() == id
}

pub(crate) fn hook_hosted(harness: Option<&str>) -> Option<bool> {
    let acp = std::env::var_os("T3_ACP_MCP_NODE").is_some_and(|id| !id.is_empty());
    match harness {
        Some("codex") => Some(
            acp || std::env::var("CODEX_INTERNAL_ORIGINATOR_OVERRIDE").as_deref() == Ok("T3 Code"),
        ),
        None | Some("claude") => {
            Some(acp || std::env::var_os("T3CODE_THREAD_ID").is_some_and(|id| !id.is_empty()))
        }
        // shortcut: OpenCode SDK exposes no hook-time owner marker; retain ID
        // deduplication until T3 supplies provenance for shared-server events.
        _ => None,
    }
}

fn native_sessions(connection: &Connection) -> anyhow::Result<Vec<ProviderSession>> {
    let sql = format!(
        "SELECT json_extract(payload_json, '$.nativeThreadRef.driver'),
                json_extract(payload_json, '$.nativeThreadRef.nativeId')
         FROM {PREFIX}provider_threads
         WHERE json_extract(payload_json, '$.nativeThreadRef.strength') = 'strong'
           AND json_extract(payload_json, '$.nativeThreadRef.nativeId') IS NOT NULL
         LIMIT ?1"
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([(MAX_NATIVE_SESSIONS + 1) as i64], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut sessions = Vec::new();
    for (count, row) in rows.enumerate() {
        anyhow::ensure!(count < MAX_NATIVE_SESSIONS, "too many T3 provider threads");
        let (driver, id) = row?;
        let harness = match driver.as_str() {
            "codex" => "codex",
            "claudeAgent" => "claude",
            "opencode" => "opencode",
            _ => continue,
        };
        if !id.is_empty() {
            sessions.push(ProviderSession {
                harness: harness.to_owned(),
                id,
            });
        }
    }
    sessions.sort_by(|a, b| (&a.harness, &a.id).cmp(&(&b.harness, &b.id)));
    sessions.dedup();
    Ok(sessions)
}

#[derive(Default, Serialize)]
struct Facts {
    latest: Option<Run>,
    active: Option<Run>,
    request: Option<Request>,
    card: Option<(String, String, String)>,
    message: Option<(String, String)>,
    item: Option<(String, String, String)>,
    background: bool,
}

#[derive(Serialize)]
struct Run {
    id: String,
    status: String,
    requested: String,
    completed: Option<String>,
    started: Option<String>,
}

#[derive(Serialize)]
struct Request {
    id: String,
    kind: String,
    status: String,
    created: String,
    resolved: Option<String>,
    capability: Option<String>,
}

fn idle_compaction_message(alias: &str) -> String {
    format!(
        "({alias}.role = 'user'
          AND lower(trim(json_extract({alias}.payload_json, '$.text'),
              char(9,10,11,12,13,32,160,5760,8192,8193,8194,8195,8196,8197,8198,8199,8200,8201,8202,8232,8233,8239,8287,12288,65279))) = '/compact'
          AND coalesce(json_array_length({alias}.payload_json, '$.attachments'), 0) = 0
          AND (json_extract({alias}.payload_json, '$.idleCompaction') IS NOT NULL
               OR (json_extract({alias}.payload_json, '$.createdBy') = 'agent'
                   AND json_extract({alias}.payload_json, '$.creationSource') = 'server')))"
    )
}

fn silent_run(alias: &str) -> String {
    let prompt = idle_compaction_message("prompt");
    format!(
        "({alias}.status != 'failed'
          AND EXISTS (SELECT 1 FROM {PREFIX}messages prompt
              WHERE prompt.message_id = json_extract({alias}.payload_json, '$.userMessageId') AND {prompt})
          AND NOT EXISTS (SELECT 1 FROM {PREFIX}turn_items compaction
              WHERE compaction.thread_id = {alias}.thread_id AND compaction.run_id = {alias}.run_id
                AND compaction.type = 'compaction' AND compaction.status = 'failed'))"
    )
}

fn read_run(connection: &Connection, thread: &str, active: bool) -> rusqlite::Result<Option<Run>> {
    let condition = if active {
        "status IN ('preparing', 'starting', 'running', 'waiting')"
    } else {
        "NOT(status = 'queued' AND COALESCE(json_extract(payload_json, '$.queueHeld'), 0) = 1)"
    };
    let silent = silent_run("r");
    connection
        .query_row(
            &format!(
                "WITH visible AS (
                    SELECT r.* FROM {PREFIX}runs r WHERE r.thread_id = ?1 AND NOT {silent}
                 ), executed AS (
                    SELECT * FROM visible WHERE status != 'queued'
                      AND NOT(status = 'cancelled' AND json_extract(payload_json, '$.startedAt') IS NULL)
                    ORDER BY (completed_at IS NULL) DESC, julianday(completed_at) DESC, ordinal DESC LIMIT 1
                 ), limited AS (
                    SELECT e.run_id FROM executed e WHERE e.status = 'failed' AND (
                      SELECT json_extract(i.payload_json, '$.failure.class') FROM {PREFIX}turn_items i WHERE i.run_id = e.run_id
                        AND i.node_id IS json_extract(e.payload_json, '$.rootNodeId')
                        AND i.type = 'error' AND i.status = 'failed'
                        ORDER BY julianday(i.updated_at) DESC, i.ordinal DESC, i.turn_item_id DESC LIMIT 1) = 'usage_limit'
                 )
                 SELECT run_id, status, requested_at, completed_at,
                        COALESCE(json_extract(payload_json, '$.workStartedAt'), json_extract(payload_json, '$.startedAt'))
                 FROM visible WHERE {condition}
                 ORDER BY (run_id IN (SELECT run_id FROM limited)) DESC, ordinal DESC LIMIT 1"
            ),
            [thread],
            |row| {
                Ok(Run {
                    id: row.get(0)?,
                    status: row.get(1)?,
                    requested: row.get(2)?,
                    completed: row.get(3)?,
                    started: row.get(4)?,
                })
            },
        )
        .optional()
}

fn thread_facts(connection: &Connection, thread: &str) -> anyhow::Result<Facts> {
    let silent = silent_run("r");
    let idle_prompt = idle_compaction_message("m");
    let latest = read_run(connection, thread, false)?;
    let active = read_run(connection, thread, true)?;
    let request = connection
        .query_row(
            &format!(
                "SELECT runtime_request_id, kind, status, created_at, resolved_at,
                        json_extract(payload_json, '$.responseCapability.type')
                 FROM {PREFIX}runtime_requests WHERE thread_id = ?1
                   AND COALESCE(json_extract(payload_json, '$.responseCapability.type'), '') != 'message'
                 ORDER BY (status = 'pending') DESC, created_at DESC LIMIT 1"
            ),
            [thread],
            |row| {
                Ok(Request {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    status: row.get(2)?,
                    created: row.get(3)?,
                    resolved: row.get(4)?,
                    capability: row.get(5)?,
                })
            },
        )
        .optional()?;
    let card = connection
        .query_row(
            &format!(
                "SELECT i.turn_item_id, i.type, COALESCE(json_extract(i.payload_json, '$.startedAt'), i.updated_at)
                 FROM {PREFIX}turn_items i WHERE i.thread_id = ?1 AND i.node_id IS NOT NULL AND i.status = 'waiting'
                   AND ((i.type = 'secret_request' AND EXISTS (
                         SELECT 1 FROM {PREFIX}runs r WHERE r.run_id = i.run_id
                           AND r.status IN ('preparing', 'starting', 'running', 'waiting')))
                     OR (i.type = 'link_request' AND NOT EXISTS (
                         SELECT 1 FROM {PREFIX}messages m WHERE m.thread_id = i.thread_id AND m.role = 'user'
                           AND json_extract(m.payload_json, '$.createdBy') = 'user'
                           AND julianday(m.updated_at) > julianday(COALESCE(json_extract(i.payload_json, '$.startedAt'), i.updated_at)))))
                 ORDER BY i.updated_at DESC LIMIT 1"
            ),
            [thread],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let message = connection
        .query_row(
            &format!(
                "SELECT m.message_id, m.updated_at FROM {PREFIX}messages m
                 WHERE m.thread_id = ?1 AND NOT COALESCE({idle_prompt}, 0)
                   AND NOT EXISTS (SELECT 1 FROM {PREFIX}runs r WHERE r.run_id = m.run_id AND {silent})
                 ORDER BY m.updated_at DESC LIMIT 1"
            ),
            [thread],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let item = connection
        .query_row(
            &format!(
                "SELECT i.turn_item_id, i.status, i.updated_at FROM {PREFIX}turn_items i
                 WHERE i.thread_id = ?1
                   AND NOT EXISTS (SELECT 1 FROM {PREFIX}runs r WHERE r.run_id = i.run_id AND {silent})
                 ORDER BY i.ordinal DESC LIMIT 1"
            ),
            [thread],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let watching_pull_request = connection.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM {PREFIX}threads t, json_each(t.payload_json, '$.pullRequests') pr
                 WHERE t.thread_id = ?1 AND json_extract(pr.value, '$.watch') IS NOT NULL
                   AND COALESCE(json_extract(pr.value, '$.source'), '') != 'stack-dismissed'
                   AND COALESCE(json_extract(pr.value, '$.snapshot.state'), '') != 'merged')"
        ),
        [thread],
        |row| row.get::<_, bool>(0),
    )?;
    let background = watching_pull_request || (latest.as_ref().is_some_and(|run| {
        matches!(run.status.as_str(), "completed" | "interrupted" | "failed" | "cancelled" | "waiting")
    }) && connection.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM {PREFIX}provider_threads pt, json_each(pt.payload_json, '$.pendingBackgroundTasks') task
                     WHERE pt.thread_id = ?1
                       AND (pt.provider_thread_id = (SELECT active_provider_thread_id FROM {PREFIX}threads WHERE thread_id = ?1)
                            OR (SELECT active_provider_thread_id FROM {PREFIX}threads WHERE thread_id = ?1) IS NULL)
                       AND (json_extract(task.value, '$.awaited') = 1 OR json_extract(task.value, '$.kind') IN ('subagent', 'background_task')))
                 OR EXISTS(SELECT 1 FROM {PREFIX}turn_items i LEFT JOIN {PREFIX}runs r ON r.run_id = i.run_id
                     WHERE i.thread_id = ?1 AND i.status IN ('pending', 'running', 'waiting') AND COALESCE(r.status, '') != 'rolled_back'
                       AND NOT COALESCE({silent}, 0)
                       AND (i.type = 'subagent' OR (i.type = 'dynamic_tool' AND COALESCE(json_extract(i.payload_json, '$.input.persistent'), 0) != 1)))"
        ),
        [thread],
        |row| row.get::<_, bool>(0),
    )?);
    Ok(Facts {
        latest,
        active,
        request,
        card,
        message,
        item,
        background,
    })
}

impl Facts {
    fn status(&self) -> (Status, Option<String>) {
        if let Some(request) = &self.request {
            if request.status == "pending" && request.capability.as_deref() != Some("message") {
                return (Status::NeedsHelp, Some(format!("t3code_{}", request.kind)));
            }
        }
        if let Some((_, kind, _)) = &self.card {
            return (Status::NeedsHelp, Some(format!("t3code_{kind}")));
        }
        if self.active.is_some()
            || self.background
            || self
                .latest
                .as_ref()
                .is_some_and(|run| run.status == "queued")
        {
            (Status::Active, None)
        } else if self
            .latest
            .as_ref()
            .is_some_and(|run| run.status == "failed")
        {
            (Status::NeedsHelp, Some("t3code_error".to_owned()))
        } else {
            (Status::Inactive, None)
        }
    }

    fn activity(&self) -> String {
        // Hash only activity facts: never names, project metadata or thread.updated_at.
        format!(
            "{:016x}",
            stable_hash(serde_json::to_string(self).unwrap_or_default().as_bytes())
        )
    }

    fn last_updated(&self, created: &str) -> String {
        let mut times = vec![created];
        for run in [&self.latest, &self.active].into_iter().flatten() {
            times.push(&run.requested);
            times.extend(run.completed.as_deref());
            times.extend(run.started.as_deref());
        }
        if let Some(request) = &self.request {
            times.push(&request.created);
            times.extend(request.resolved.as_deref());
        }
        if let Some((_, _, at)) = &self.card {
            times.push(at);
        }
        if let Some((_, at)) = &self.message {
            times.push(at);
        }
        if let Some((_, _, at)) = &self.item {
            times.push(at);
        }
        times
            .into_iter()
            .filter_map(|time| DateTime::parse_from_rfc3339(time).ok())
            .max()
            .map(|time| time.with_timezone(&Utc).to_rfc3339())
            .unwrap_or_else(|| created.to_owned())
    }
}

fn retire_unselected_sources(state: &mut HookState, selected: &[String]) -> bool {
    let mut changed = false;
    for (id, tracking) in &mut state.t3_threads {
        if selected.iter().any(|key| tracking.source.starts_with(key)) {
            continue;
        }
        if !tracking.provider_sessions.is_empty() {
            tracking.provider_sessions.clear();
            changed = true;
        }
        if let Some(session) = state.sessions.get_mut(id) {
            if session.harness.as_deref() == Some("t3code") && session.status != Status::Done {
                session.status = Status::Done;
                session.notification_type = None;
                changed = true;
            }
        }
    }
    changed
}

fn apply_snapshots(state: &mut HookState, snapshots: &[Snapshot]) -> bool {
    let mut changed = false;
    for snapshot in snapshots {
        if snapshot.threads.is_empty()
            && snapshot.provider_sessions.is_empty()
            && !state
                .t3_threads
                .values()
                .any(|tracking| tracking.source == snapshot.source)
        {
            continue;
        }
        let native_id = format!("{}@providers", snapshot.source);
        let native = Tracking {
            source: snapshot.source.clone(),
            dismissed: true,
            offline_at: (!snapshot.online).then(|| {
                state
                    .t3_threads
                    .get(&native_id)
                    .and_then(|tracking| tracking.offline_at.clone())
                    .unwrap_or_else(|| Utc::now().to_rfc3339())
            }),
            provider_sessions: if snapshot.online {
                snapshot.provider_sessions.clone()
            } else {
                state
                    .t3_threads
                    .get(&native_id)
                    .map(|tracking| tracking.provider_sessions.clone())
                    .unwrap_or_default()
            },
            ..Default::default()
        };
        if state.t3_threads.get(&native_id) != Some(&native) {
            state.t3_threads.insert(native_id, native);
            changed = true;
        }
        let ids: HashSet<&str> = snapshot
            .threads
            .iter()
            .map(|thread| thread.id.as_str())
            .collect();
        for (id, tracking) in &mut state.t3_threads {
            if tracking.source == snapshot.source && !ids.contains(id.as_str()) {
                if let Some(session) = state.sessions.get_mut(id) {
                    if session.harness.as_deref() == Some("t3code")
                        && session.status != Status::Done
                    {
                        session.status = Status::Done;
                        session.notification_type = None;
                        changed = true;
                    }
                }
            }
        }
        for thread in &snapshot.threads {
            let previous = state.t3_threads.get(&thread.id);
            let new_activity = previous.is_none_or(|tracking| tracking.activity != thread.activity);
            let dismissed = previous.is_some_and(|tracking| tracking.dismissed) && !new_activity;
            let should_show =
                !dismissed && (new_activity || state.sessions.contains_key(&thread.id));
            let tracking = Tracking {
                source: snapshot.source.clone(),
                activity: thread.activity.clone(),
                dismissed,
                offline_at: None,
                provider_sessions: Vec::new(),
            };
            if previous != Some(&tracking) {
                state.t3_threads.insert(thread.id.clone(), tracking);
                changed = true;
            }
            if should_show {
                let equal = state.sessions.get(&thread.id).is_some_and(|old| {
                    old.status == thread.session.status
                        && old.last_updated == thread.session.last_updated
                        && old.project_path == thread.session.project_path
                        && old.notification_type == thread.session.notification_type
                        && old.name == thread.session.name
                        && old.terminal == thread.session.terminal
                        && old.harness == thread.session.harness
                });
                if !equal {
                    state
                        .sessions
                        .insert(thread.id.clone(), thread.session.clone());
                    changed = true;
                }
            }
        }
    }
    changed
}

fn stable_hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

pub fn dismiss(state: &mut HookState, id: &str) {
    if let Some(tracking) = state.t3_threads.get_mut(id) {
        tracking.dismissed = true;
    }
}

/// Display-only: hooks retain their own state, but never duplicate their host
/// T3 thread, including descendants and threads the user dismissed or archived.
pub fn hide_provider_sessions(state: &mut HookState) {
    let include_remote =
        crate::config::read_config_strict().is_ok_and(|cfg| cfg.t3_linked_machines_enabled);
    let mut native = HashMap::<(&str, &str), Option<DateTime<Utc>>>::new();
    for tracking in state.t3_threads.values() {
        let cutoff = tracking
            .offline_at
            .as_deref()
            .and_then(|at| at.parse().ok());
        for session in &tracking.provider_sessions {
            native
                .entry((session.harness.as_str(), session.id.as_str()))
                .and_modify(|at| {
                    *at = match (*at, cutoff) {
                        (Some(a), Some(b)) => Some(a.max(b)),
                        _ => None,
                    };
                })
                .or_insert(cutoff);
        }
    }
    state.sessions.retain(|id, session| {
        if !include_remote
            && state
                .t3_threads
                .get(id)
                .is_some_and(|tracking| remote::is_remote_source(&tracking.source))
        {
            return false;
        }
        if session.t3_hosted == Some(false) {
            return true;
        }
        match native.get(&(session.harness.as_deref().unwrap_or("claude"), id.as_str())) {
            None => true,
            Some(None) => false,
            // A new native prompt after T3 exits belongs to the resumed CLI session.
            Some(Some(cutoff)) => session
                .last_updated
                .parse::<DateTime<Utc>>()
                .is_ok_and(|at| at > *cutoff),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use serde_json::json;

    const AT: &str = "2026-11-06T10:00:00Z";
    const LATER: &str = "2026-11-06T10:01:00Z";

    fn fixture() -> (tempfile::TempDir, Source, Connection) {
        let directory = tempfile::tempdir().unwrap();
        let source = Source::new(directory.path().to_owned());
        std::fs::create_dir_all(source.data()).unwrap();
        std::fs::write(source.data().join("environment-id"), "environment-1\n").unwrap();
        let connection = Connection::open(source.data().join("statev2.sqlite")).unwrap();
        connection.execute_batch(
            "CREATE TABLE projection_projects (project_id TEXT PRIMARY KEY, workspace_root TEXT, archived_at TEXT, deleted_at TEXT);
             INSERT INTO projection_projects VALUES ('project', '/work/project', NULL, NULL);
             CREATE TABLE orchestration_v2_projection_threads (thread_id TEXT PRIMARY KEY, project_id TEXT, title TEXT,
               created_at TEXT, updated_at TEXT, archived_at TEXT, deleted_at TEXT, active_provider_thread_id TEXT, payload_json TEXT);
             CREATE TABLE orchestration_v2_projection_runs (run_id TEXT PRIMARY KEY, thread_id TEXT, ordinal INTEGER,
               status TEXT, requested_at TEXT, completed_at TEXT, payload_json TEXT);
             CREATE TABLE orchestration_v2_projection_runtime_requests (runtime_request_id TEXT PRIMARY KEY, thread_id TEXT,
               kind TEXT, status TEXT, created_at TEXT, resolved_at TEXT, payload_json TEXT);
             CREATE TABLE orchestration_v2_projection_messages (message_id TEXT PRIMARY KEY, thread_id TEXT, role TEXT,
               updated_at TEXT, payload_json TEXT, run_id TEXT);
             CREATE TABLE orchestration_v2_projection_turn_items (turn_item_id TEXT PRIMARY KEY, thread_id TEXT,
               run_id TEXT, node_id TEXT, type TEXT, ordinal INTEGER, status TEXT, updated_at TEXT, payload_json TEXT);
             CREATE TABLE orchestration_v2_projection_provider_threads (provider_thread_id TEXT PRIMARY KEY, thread_id TEXT,
               payload_json TEXT);",
        ).unwrap();
        add_thread(
            &connection,
            "thread",
            json!({"worktreePath":"/work/project-feature"}),
        );
        (directory, source, connection)
    }

    fn add_thread(connection: &Connection, id: &str, payload: serde_json::Value) {
        connection.execute(
            "INSERT INTO orchestration_v2_projection_threads VALUES (?1, 'project', 'T3 title', ?2, ?2, NULL, NULL, NULL, ?3)",
            params![id, AT, payload.to_string()],
        ).unwrap();
    }

    fn add_run(
        connection: &Connection,
        id: &str,
        ordinal: i64,
        status: &str,
        payload: serde_json::Value,
    ) {
        connection.execute(
            "INSERT INTO orchestration_v2_projection_runs VALUES (?1, 'thread', ?2, ?3, ?4, NULL, ?5)",
            params![id, ordinal, status, AT, payload.to_string()],
        ).unwrap();
    }

    fn status(connection: &Connection, source: &Source) -> Status {
        read_snapshot(connection, source).unwrap().threads[0]
            .session
            .status
            .clone()
    }

    #[test]
    fn lifecycle_and_held_queues_follow_real_work() {
        let (_directory, source, connection) = fixture();
        assert_eq!(status(&connection, &source), Status::Inactive);
        add_run(&connection, "run", 1, "running", json!({}));
        for (run_status, expected) in [
            ("preparing", Status::Active),
            ("starting", Status::Active),
            ("running", Status::Active),
            ("waiting", Status::Active),
            ("completed", Status::Inactive),
            ("interrupted", Status::Inactive),
            ("cancelled", Status::Inactive),
            ("failed", Status::NeedsHelp),
        ] {
            connection
                .execute(
                    "UPDATE orchestration_v2_projection_runs SET status = ?1 WHERE run_id = 'run'",
                    [run_status],
                )
                .unwrap();
            assert_eq!(status(&connection, &source), expected, "{run_status}");
        }
        connection
            .execute(
                "UPDATE orchestration_v2_projection_runs SET status = 'completed'",
                [],
            )
            .unwrap();
        add_run(&connection, "held", 2, "queued", json!({"queueHeld":true}));
        assert_eq!(status(&connection, &source), Status::Inactive);
        connection.execute("UPDATE orchestration_v2_projection_runs SET payload_json = '{}' WHERE run_id = 'held'", []).unwrap();
        assert_eq!(status(&connection, &source), Status::Active);
        connection.execute("UPDATE orchestration_v2_projection_runs SET status = 'running' WHERE run_id = 'run'", []).unwrap();
        connection.execute("UPDATE orchestration_v2_projection_runs SET status = 'cancelled' WHERE run_id = 'held'", []).unwrap();
        assert_eq!(status(&connection, &source), Status::Active);
    }

    #[test]
    fn pending_input_and_secret_link_cards_clear_on_authoritative_evidence() {
        let (_directory, source, connection) = fixture();
        add_run(&connection, "run", 1, "running", json!({}));
        connection.execute(
            "INSERT INTO orchestration_v2_projection_runtime_requests VALUES ('request', 'thread', 'user_input', 'pending', ?1, NULL, ?2)",
            params![AT, json!({"responseCapability":{"type":"request"}}).to_string()],
        ).unwrap();
        assert_eq!(status(&connection, &source), Status::NeedsHelp);
        connection.execute("UPDATE orchestration_v2_projection_runtime_requests SET payload_json = '{\"responseCapability\":{\"type\":\"message\"}}'", []).unwrap();
        assert_eq!(status(&connection, &source), Status::Active);
        connection.execute(
            "INSERT INTO orchestration_v2_projection_runtime_requests VALUES ('older-approval', 'thread', 'command', 'pending', '2026-11-06T09:59:00Z', NULL, '{}')",
            [],
        ).unwrap();
        assert_eq!(
            status(&connection, &source),
            Status::NeedsHelp,
            "an async question must not hide a live approval"
        );
        connection.execute("UPDATE orchestration_v2_projection_runtime_requests SET status = 'resolved' WHERE runtime_request_id = 'older-approval'", []).unwrap();
        connection.execute(
            "INSERT INTO orchestration_v2_projection_turn_items VALUES ('card', 'thread', 'run', 'node', 'secret_request', 1, 'waiting', ?1, '{}')",
            [AT],
        ).unwrap();
        assert_eq!(status(&connection, &source), Status::NeedsHelp);
        connection
            .execute(
                "UPDATE orchestration_v2_projection_runs SET status = 'completed'",
                [],
            )
            .unwrap();
        assert_eq!(status(&connection, &source), Status::Inactive);
        connection
            .execute(
                "UPDATE orchestration_v2_projection_turn_items SET type = 'link_request'",
                [],
            )
            .unwrap();
        assert_eq!(status(&connection, &source), Status::NeedsHelp);
        connection.execute(
            "INSERT INTO orchestration_v2_projection_messages (message_id, thread_id, role, updated_at, payload_json) VALUES ('reply', 'thread', 'user', ?1, '{\"createdBy\":\"agent\"}')",
            [LATER],
        ).unwrap();
        assert_eq!(status(&connection, &source), Status::NeedsHelp);
        connection.execute("UPDATE orchestration_v2_projection_messages SET payload_json = '{\"createdBy\":\"user\"}'", []).unwrap();
        assert_eq!(status(&connection, &source), Status::Inactive);
    }

    #[test]
    fn background_waits_hold_active_but_persistent_tools_do_not() {
        let (_directory, source, connection) = fixture();
        add_run(&connection, "run", 1, "completed", json!({}));
        connection.execute(
            "INSERT INTO orchestration_v2_projection_provider_threads VALUES ('provider', 'thread', ?1)",
            [json!({"pendingBackgroundTasks":[{"taskId":"a","kind":"subagent"}]}).to_string()],
        ).unwrap();
        assert_eq!(status(&connection, &source), Status::Active);
        connection.execute("UPDATE orchestration_v2_projection_provider_threads SET payload_json = '{\"pendingBackgroundTasks\":[{\"taskId\":\"a\",\"kind\":\"command\"}]}'", []).unwrap();
        assert_eq!(status(&connection, &source), Status::Inactive);
        connection.execute(
            "INSERT INTO orchestration_v2_projection_turn_items VALUES ('background', 'thread', 'run', 'node', 'dynamic_tool', 1, 'running', ?1, '{}')",
            [AT],
        ).unwrap();
        assert_eq!(status(&connection, &source), Status::Active);
        connection.execute("UPDATE orchestration_v2_projection_turn_items SET payload_json = '{\"input\":{\"persistent\":true}}'", []).unwrap();
        assert_eq!(status(&connection, &source), Status::Inactive);
    }

    #[test]
    fn pending_child_work_keeps_a_completed_parent_active() {
        for kind in ["subagent", "dynamic_tool"] {
            let (_directory, source, connection) = fixture();
            add_run(&connection, "run", 1, "completed", json!({}));
            connection.execute(
                "INSERT INTO orchestration_v2_projection_turn_items VALUES ('child', 'thread', 'run', 'node', ?1, 1, 'pending', ?2, '{}')",
                params![kind, AT],
            ).unwrap();
            assert_eq!(status(&connection, &source), Status::Active, "{kind}");
        }
    }

    #[test]
    fn silent_compaction_does_not_restore_dismissed_threads() {
        for (automatic, run_status, item_status, visible) in [
            (true, "running", "running", false),
            (false, "running", "running", true),
            (true, "failed", "failed", true),
            (true, "completed", "failed", true),
        ] {
            let (_directory, source, connection) = fixture();
            add_run(&connection, "run", 1, "completed", json!({}));
            let snapshot = read_snapshot(&connection, &source).unwrap();
            let id = snapshot.threads[0].id.clone();
            let mut state = HookState::default();
            apply_snapshots(&mut state, &[snapshot]);
            dismiss(&mut state, &id);
            state.sessions.remove(&id);
            let prompt = if automatic {
                json!({"text":"/compact", "createdBy":"agent", "creationSource":"server"})
            } else {
                json!({"text":"/compact", "createdBy":"user"})
            };
            add_run(
                &connection,
                "compact",
                2,
                run_status,
                json!({"userMessageId":"compact-prompt"}),
            );
            connection.execute(
                "INSERT INTO orchestration_v2_projection_messages VALUES ('compact-prompt', 'thread', 'user', ?1, ?2, 'compact')",
                params![LATER, prompt.to_string()],
            ).unwrap();
            connection.execute(
                "INSERT INTO orchestration_v2_projection_messages VALUES ('compact-result', 'thread', 'assistant', ?1, '{}', 'compact')",
                [LATER],
            ).unwrap();
            connection.execute(
                "INSERT INTO orchestration_v2_projection_turn_items VALUES ('compaction', 'thread', 'compact', 'root', 'compaction', 1, ?1, ?2, '{}')",
                params![item_status, LATER],
            ).unwrap();
            apply_snapshots(&mut state, &[read_snapshot(&connection, &source).unwrap()]);
            assert_eq!(
                state.sessions.contains_key(&id),
                visible,
                "automatic={automatic}, run={run_status}, item={item_status}"
            );
        }
    }

    #[test]
    fn cancelled_unstarted_queue_preserves_the_last_usage_limit_failure() {
        let (_directory, source, connection) = fixture();
        add_run(
            &connection,
            "limited",
            1,
            "failed",
            json!({"rootNodeId":"root"}),
        );
        connection.execute(
            "INSERT INTO orchestration_v2_projection_turn_items VALUES ('failure', 'thread', 'limited', 'root', 'error', 1, 'failed', ?1, ?2)",
            params![AT, json!({"failure":{"class":"usage_limit","message":"Usage limit reached"}}).to_string()],
        ).unwrap();
        add_run(
            &connection,
            "cancelled",
            2,
            "cancelled",
            json!({"startedAt":null}),
        );
        assert_eq!(status(&connection, &source), Status::NeedsHelp);
        connection.execute(
            "INSERT INTO orchestration_v2_projection_turn_items VALUES ('newer-error', 'thread', 'limited', 'root', 'error', 2, 'failed', ?1, ?2)",
            params![LATER, json!({"failure":{"class":"provider_error"}}).to_string()],
        ).unwrap();
        assert_eq!(status(&connection, &source), Status::Inactive);
        add_run(
            &connection,
            "executed",
            3,
            "completed",
            json!({"startedAt":LATER}),
        );
        assert_eq!(status(&connection, &source), Status::Inactive);
    }

    #[test]
    fn thread_metadata_filters_namespace_and_provider_dedup() {
        let (_directory, source, connection) = fixture();
        add_thread(
            &connection,
            "child",
            json!({"lineage":{"relationshipToParent":"subagent"}}),
        );
        add_thread(&connection, "side", json!({"lineage":{"sideChat":true}}));
        add_thread(&connection, "archived", json!({}));
        connection.execute("UPDATE orchestration_v2_projection_threads SET archived_at = ?1 WHERE thread_id = 'archived'", [AT]).unwrap();
        for (provider, thread, driver, native) in [
            ("codex-provider", "thread", "codex", "codex-native"),
            ("claude-provider", "child", "claudeAgent", "claude-native"),
            ("opencode-provider", "thread", "opencode", "opencode-native"),
        ] {
            connection.execute(
                "INSERT INTO orchestration_v2_projection_provider_threads VALUES (?1, ?2, ?3)",
                params![provider, thread, json!({"nativeThreadRef":{"driver":driver,"nativeId":native,"strength":"strong"}}).to_string()],
            ).unwrap();
        }
        let snapshot = read_snapshot(&connection, &source).unwrap();
        assert_eq!(snapshot.threads.len(), 1);
        let thread = &snapshot.threads[0];
        assert_eq!(thread.session.name.as_deref(), Some("T3 title"));
        assert_eq!(
            thread.session.project_path.as_deref(),
            Some("/work/project-feature")
        );
        let terminal = thread.session.terminal.as_ref().unwrap();
        assert_eq!(
            terminal.host_session_id.as_deref(),
            Some("environment-1/thread")
        );
        assert_eq!(terminal.bundle_id.as_deref(), Some("com.t3tools.t3code"));
        assert_eq!(terminal.owner_pid, None);
        assert_ne!(source.key, Source::new(source.home.join("other")).key);
        let mut state = HookState::default();
        assert!(apply_snapshots(&mut state, std::slice::from_ref(&snapshot)));
        let mut codex = thread.session.clone();
        codex.harness = Some("codex".to_owned());
        state.sessions.insert("codex-native".to_owned(), codex);
        let mut claude = thread.session.clone();
        claude.harness = None;
        state.sessions.insert("claude-native".to_owned(), claude);
        let mut opencode = thread.session.clone();
        opencode.harness = Some("opencode".to_owned());
        state
            .sessions
            .insert("opencode-native".to_owned(), opencode);
        state
            .sessions
            .insert("unrelated".to_owned(), thread.session.clone());
        hide_provider_sessions(&mut state);
        assert!(!state.sessions.contains_key("codex-native"));
        assert!(!state.sessions.contains_key("claude-native"));
        assert!(!state.sessions.contains_key("opencode-native"));
        assert!(state.sessions.contains_key("unrelated"));
        for (id, harness) in [("codex-native", Some("codex")), ("claude-native", None)] {
            for status in [Status::Active, Status::NeedsHelp] {
                for hosted in [true, false] {
                    let mut resumed = thread.session.clone();
                    resumed.harness = harness.map(str::to_owned);
                    resumed.status = status.clone();
                    resumed.t3_hosted = Some(hosted);
                    state.sessions.insert(id.to_owned(), resumed);
                    hide_provider_sessions(&mut state);
                    assert_eq!(
                        state.sessions.contains_key(id),
                        !hosted,
                        "independent native {harness:?} {status:?} must be visible with T3 online"
                    );
                }
            }
        }
        connection
            .execute("UPDATE projection_projects SET deleted_at = ?1", [AT])
            .unwrap();
        assert!(read_snapshot(&connection, &source)
            .unwrap()
            .threads
            .is_empty());
    }

    #[test]
    fn pull_request_watches_hold_active_without_a_run_until_merged_or_dismissed() {
        let (_directory, source, connection) = fixture();
        for (link, expected) in [
            (
                json!({"watch":{},"source":"created","snapshot":{"state":"open"}}),
                Status::Active,
            ),
            (
                json!({"watch":{},"source":"created","snapshot":{"state":"merged"}}),
                Status::Inactive,
            ),
            (
                json!({"watch":{},"source":"stack-dismissed","snapshot":{"state":"open"}}),
                Status::Inactive,
            ),
            (
                json!({"source":"created","snapshot":{"state":"open"}}),
                Status::Inactive,
            ),
        ] {
            connection
                .execute(
                    "UPDATE orchestration_v2_projection_threads SET payload_json = ?1",
                    [json!({"pullRequests":[link]}).to_string()],
                )
                .unwrap();
            assert_eq!(status(&connection, &source), expected);
        }
    }

    #[test]
    fn dismissal_survives_serialization_rename_and_retention_until_activity() {
        let (_directory, source, connection) = fixture();
        add_run(&connection, "run", 1, "completed", json!({}));
        let snapshot = read_snapshot(&connection, &source).unwrap();
        let id = snapshot.threads[0].id.clone();
        let mut state = HookState::default();
        assert!(apply_snapshots(&mut state, std::slice::from_ref(&snapshot)));
        assert!(!apply_snapshots(&mut state, &[snapshot]));
        dismiss(&mut state, &id);
        state.sessions.remove(&id);
        let mut state: HookState =
            serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
        connection
            .execute(
                "UPDATE orchestration_v2_projection_threads SET title = 'Renamed', updated_at = ?1",
                [LATER],
            )
            .unwrap();
        let renamed = read_snapshot(&connection, &source).unwrap();
        apply_snapshots(&mut state, &[renamed]);
        assert!(!state.sessions.contains_key(&id));
        assert!(state.t3_threads[&id].dismissed);
        add_run(&connection, "new-run", 2, "running", json!({}));
        let updated = read_snapshot(&connection, &source).unwrap();
        apply_snapshots(&mut state, std::slice::from_ref(&updated));
        assert_eq!(state.sessions[&id].status, Status::Active);
        assert_eq!(state.sessions[&id].name.as_deref(), Some("Renamed"));
        assert!(!state.t3_threads[&id].dismissed);
        state.sessions.remove(&id);
        apply_snapshots(&mut state, &[updated]);
        assert!(
            !state.sessions.contains_key(&id),
            "retention must not cause re-import"
        );
    }

    #[test]
    fn offline_and_archived_sources_end_only_their_own_sessions() {
        let (_directory, source, connection) = fixture();
        add_run(&connection, "run", 1, "running", json!({}));
        connection.execute(
            "INSERT INTO orchestration_v2_projection_provider_threads VALUES ('provider', 'thread', ?1)",
            [json!({"nativeThreadRef":{"driver":"codex","nativeId":"native","strength":"strong"}}).to_string()],
        ).unwrap();
        let snapshot = read_snapshot(&connection, &source).unwrap();
        let id = snapshot.threads[0].id.clone();
        let mut state = HookState::default();
        assert!(!apply_snapshots(
            &mut state,
            &[Snapshot {
                source: "absent".into(),
                ..Default::default()
            }]
        ));
        apply_snapshots(&mut state, &[snapshot]);
        state
            .sessions
            .insert("another".into(), state.sessions[&id].clone());
        apply_snapshots(
            &mut state,
            &[Snapshot {
                source: source.key.clone(),
                ..Default::default()
            }],
        );
        assert_eq!(state.sessions[&id].status, Status::Done);
        assert_eq!(state.sessions["another"].status, Status::Active);
        let mut native = state.sessions["another"].clone();
        native.harness = Some("codex".into());
        let native_id = format!("{}@providers", source.key);
        let offline_at = state.t3_threads[&native_id].offline_at.clone().unwrap();
        let offline = Snapshot {
            source: source.key.clone(),
            ..Default::default()
        };
        assert!(!apply_snapshots(&mut state, &[offline]));
        assert_eq!(
            state.t3_threads[&native_id].offline_at.as_deref(),
            Some(offline_at.as_str())
        );
        state.sessions.insert("native".into(), native.clone());
        hide_provider_sessions(&mut state);
        assert!(
            !state.sessions.contains_key("native"),
            "server shutdown must not expose stale native-hook rows"
        );
        native.last_updated = (offline_at.parse::<DateTime<Utc>>().unwrap()
            + chrono::Duration::seconds(1))
        .to_rfc3339();
        state.sessions.insert("native".into(), native);
        hide_provider_sessions(&mut state);
        assert!(
            state.sessions.contains_key("native"),
            "a native CLI resumed after T3 exits must remain visible"
        );
    }

    #[test]
    fn database_reads_include_wal_and_fail_without_creating_or_mutating_database() {
        let (_directory, source, mut connection) = fixture();
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .unwrap();
        add_run(&connection, "run", 1, "running", json!({}));
        let database = source.data().join("statev2.sqlite");
        let wal = source.data().join("statev2.sqlite-wal");
        let before = (
            std::fs::read(&database).unwrap(),
            std::fs::read(&wal).unwrap(),
        );
        assert_eq!(
            read_source(&source).unwrap().threads[0].session.status,
            Status::Active
        );
        assert_eq!(
            (
                std::fs::read(&database).unwrap(),
                std::fs::read(&wal).unwrap()
            ),
            before
        );
        let uncommitted = connection.transaction().unwrap();
        uncommitted
            .execute(
                "UPDATE orchestration_v2_projection_runs SET status = 'failed'",
                [],
            )
            .unwrap();
        assert_eq!(
            read_source(&source).unwrap().threads[0].session.status,
            Status::Active
        );
        uncommitted.rollback().unwrap();
        connection
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
        connection.execute_batch("BEGIN EXCLUSIVE").unwrap();
        assert!(read_source(&source).is_err());
        connection.execute_batch("ROLLBACK").unwrap();
        connection
            .execute("DROP TABLE orchestration_v2_projection_runs", [])
            .unwrap();
        assert!(read_source(&source).is_err());
        drop(connection);
        std::fs::write(&database, b"not a database").unwrap();
        assert!(read_source(&source).is_err());
        std::fs::remove_file(&database).unwrap();
        assert!(read_source(&source).is_err());
        assert!(!database.exists());
    }

    #[test]
    fn refresh_never_waits_for_an_inflight_source_scan() {
        // The worker holds CACHE through disk I/O; reader calls must return.
        let _source_scan = CACHE.lock().unwrap();
        let started = Instant::now();
        refresh();
        assert!(started.elapsed() < Duration::from_millis(100));
    }

    #[cfg(unix)]
    #[test]
    fn development_instances_are_discovered_separately() {
        if std::env::var_os("BROGLOW_TEST_T3_DEV").is_none() {
            let home = tempfile::tempdir().unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "t3::tests::development_instances_are_discovered_separately",
                ])
                .env("HOME", home.path())
                .env("BROGLOW_HOME", home.path().join(".broglow"))
                .env_remove("T3CODE_HOME")
                .env("BROGLOW_TEST_T3_DEV", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let (_directory, fixture_source, connection) = fixture();
        add_run(&connection, "run", 1, "running", json!({}));
        drop(connection);
        let base = dirs::home_dir().unwrap().join(".t3");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::rename(fixture_source.data(), base.join("dev")).unwrap();
        std::fs::write(
            base.join("dev/server-runtime.json"),
            json!({"version":1,"pid":std::process::id()}).to_string(),
        )
        .unwrap();
        let found: Vec<_> = sources()
            .into_iter()
            .filter(|source| source.data().join("statev2.sqlite").exists())
            .collect();
        assert_eq!(found.len(), 1, "development source must be found");
        let snapshot = read_source(&found[0]).unwrap();
        assert_eq!(
            snapshot.threads[0]
                .session
                .terminal
                .as_ref()
                .unwrap()
                .bundle_id
                .as_deref(),
            Some("com.t3tools.t3code.dev")
        );
        assert_ne!(found[0].key, Source::new(base).key);
    }

    #[cfg(unix)]
    #[test]
    fn switching_home_retires_the_previous_source() {
        if std::env::var_os("BROGLOW_TEST_T3_SWITCH").is_some() {
            poll_sources();
            let state: HookState =
                serde_json::from_slice(&std::fs::read(crate::state::state_file_path()).unwrap())
                    .unwrap();
            assert_eq!(
                state
                    .sessions
                    .values()
                    .filter(|s| s.status == Status::Active)
                    .count(),
                1,
                "only the selected home may retain live T3 rows"
            );
            return;
        }
        let home = tempfile::tempdir().unwrap();
        let (_first_dir, first, first_connection) = fixture();
        let (_second_dir, second, second_connection) = fixture();
        for (source, connection) in [(&first, &first_connection), (&second, &second_connection)] {
            add_run(connection, "run", 1, "running", json!({}));
            std::fs::write(
                source.data().join("server-runtime.json"),
                json!({"version":1,"pid":std::process::id()}).to_string(),
            )
            .unwrap();
        }
        for source in [&first, &second] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "t3::tests::switching_home_retires_the_previous_source",
                ])
                .env("HOME", home.path())
                .env("BROGLOW_HOME", home.path().join(".broglow"))
                .env("T3CODE_HOME", &source.home)
                .env("BROGLOW_TEST_T3_SWITCH", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    #[test]
    #[ignore = "opt-in read-only acceptance against a locally running T3 Code"]
    fn installed_v2_source_readonly_smoke() {
        let mut count = 0;
        for source in sources() {
            if runtime(&source).unwrap().is_some() {
                let started = Instant::now();
                let snapshot = read_source(&source).unwrap_or_else(|error| {
                    panic!("T3 source query failed: {error:#}");
                });
                eprintln!(
                    "T3 source: {} visible threads, {} native provider sessions, read {} ms",
                    snapshot.threads.len(),
                    snapshot.provider_sessions.len(),
                    started.elapsed().as_millis()
                );
                count += snapshot.threads.len();
            }
        }
        assert!(count > 0, "no live T3 threads available");
    }

    #[cfg(unix)]
    #[test]
    fn polling_pipeline_is_isolated_readonly_and_retries_after_state_lock_contention() {
        if std::env::var_os("BROGLOW_TEST_T3_PIPELINE").is_none() {
            let home = tempfile::tempdir().unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "t3::tests::polling_pipeline_is_isolated_readonly_and_retries_after_state_lock_contention", "--nocapture"])
                .env("HOME", home.path())
                .env("BROGLOW_HOME", home.path().join(".broglow"))
                .env("T3CODE_HOME", home.path().join("t3-override"))
                .env("BROGLOW_TEST_T3_PIPELINE", "1")
                .output().unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        fn allow_refresh() {
            if let Some(cache) = CACHE.lock().unwrap().as_mut() {
                cache.last_attempt = None;
            }
        }
        poll_sources();
        assert!(
            !crate::config::data_dir().exists(),
            "absent T3 must preserve first-run setup"
        );
        let (_directory, mut source, connection) = fixture();
        drop(connection);
        let target = sources()[0].home.clone();
        std::fs::rename(&source.home, &target).unwrap();
        source = Source::new(target);
        let connection = Connection::open(source.data().join("statev2.sqlite")).unwrap();
        add_run(&connection, "run", 1, "running", json!({}));
        std::fs::write(
            source.data().join("server-runtime.json"),
            json!({"version":1,"pid":std::process::id(),"serverVersion":"0.1.7-yandex.2"})
                .to_string(),
        )
        .unwrap();
        assert_eq!(
            sources().len(),
            1,
            "explicit override must isolate defaults"
        );
        allow_refresh();
        poll_sources();
        let raw = || -> HookState {
            serde_json::from_slice(&std::fs::read(crate::state::state_file_path()).unwrap())
                .unwrap()
        };
        let id = format!("{}thread", source.key);
        assert_eq!(raw().sessions[&id].status, Status::Active);
        assert_eq!(
            raw().sessions[&id]
                .terminal
                .as_ref()
                .unwrap()
                .bundle_id
                .as_deref(),
            Some("ru.yandex.t3code")
        );
        assert_eq!(
            raw().sessions[&id].terminal.as_ref().unwrap().owner_pid,
            Some(std::process::id()),
            "authoritative server liveness keeps long-running work from stale reaping"
        );
        let lock = crate::state::acquire_state_lock().unwrap();
        connection
            .execute(
                "UPDATE orchestration_v2_projection_runs SET status = 'failed'",
                [],
            )
            .unwrap();
        allow_refresh();
        poll_sources();
        assert_eq!(raw().sessions[&id].status, Status::Active);
        drop(lock);
        allow_refresh();
        poll_sources();
        assert_eq!(raw().sessions[&id].status, Status::NeedsHelp);
        std::fs::write(source.data().join("server-runtime.json"), "broken").unwrap();
        allow_refresh();
        poll_sources();
        assert_eq!(
            raw().sessions[&id].status,
            Status::NeedsHelp,
            "bad runtime data preserves last good state"
        );
        std::fs::remove_file(source.data().join("server-runtime.json")).unwrap();
        allow_refresh();
        poll_sources();
        assert_eq!(raw().sessions[&id].status, Status::Done);
    }
}
