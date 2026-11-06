//! Read existing T3 peer links through MCP; never pair, migrate, or write to T3.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv6Addr};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Result};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};

use super::{Snapshot, Source, Thread};
use crate::state::{SessionStatus, Status, TerminalInfo};

const INTERVAL: Duration = Duration::from_secs(10);
const BUDGET: Duration = Duration::from_secs(30);
const MAX_PEERS: usize = 8;
const MAX_RESPONSE: u64 = 2 * 1024 * 1024;
const MAX_ITEMS: usize = 16384;
const PROTOCOL: &str = "2025-06-18";

#[derive(Default)]
struct Cache {
    last_attempt: Option<Instant>,
    threads: HashMap<String, CachedThread>,
    peer_start: usize,
}

#[derive(Clone)]
struct CachedThread {
    stamp: String,
    thread: Thread,
}

static CACHE: Mutex<Option<Cache>> = Mutex::new(None);
static POLLING: AtomicBool = AtomicBool::new(false);

pub(super) fn is_remote_source(source: &str) -> bool {
    source.starts_with("t3code:") && source.contains(":remote:")
}

fn enabled() -> bool {
    crate::config::read_config_strict().is_ok_and(|cfg| cfg.t3_linked_machines_enabled)
}

/// Local SQLite polling never waits for a peer or for this worker's mutex.
pub(super) fn refresh() {
    if POLLING.load(Ordering::Relaxed) {
        return;
    }
    let Ok(guard) = CACHE.try_lock() else {
        return;
    };
    if guard
        .as_ref()
        .and_then(|cache| cache.last_attempt)
        .is_some_and(|last| last.elapsed() < INTERVAL)
    {
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
        .name("broglow-t3-peers".into())
        .spawn(|| {
            poll();
            POLLING.store(false, Ordering::Relaxed);
        })
        .is_err()
    {
        POLLING.store(false, Ordering::Relaxed);
    }
}

fn poll() {
    let Ok(mut guard) = CACHE.try_lock() else {
        return;
    };
    let cache = guard.get_or_insert_with(Cache::default);
    cache.last_attempt = Some(Instant::now());
    let deadline = Instant::now() + BUDGET;
    let mut snapshots = Vec::new();
    let mut checked_parents = Vec::new();
    if enabled() {
        let (mut peers, checked) = linked_peers(super::sources());
        checked_parents = checked;
        rotate_peers(&mut peers, &mut cache.peer_start);
        let mut peer_count = 0;
        for (source, link) in peers {
            let remote = link.source(&source);
            let snapshot = if peer_count < MAX_PEERS && Instant::now() < deadline {
                peer_count += 1;
                read_peer(&source, &remote, &link, deadline, &mut cache.threads).unwrap_or_else(
                    |_| Snapshot {
                        source: remote.key.clone(),
                        ..Default::default()
                    },
                )
            } else {
                Snapshot {
                    source: remote.key.clone(),
                    ..Default::default()
                }
            };
            snapshots.push(snapshot);
        }
    }
    // A toggle, unlink, or expiration during a slow request must not resurrect it.
    if enabled() {
        let mut still_linked = HashSet::new();
        let mut readable = Vec::new();
        for source in super::sources() {
            if let Ok(links) = links(&source) {
                readable.push(format!("{}remote:", source.key));
                still_linked.extend(links.into_iter().map(|link| link.source(&source).key));
            }
        }
        snapshots.retain(|snapshot| readable.iter().any(|p| snapshot.source.starts_with(p)));
        checked_parents.retain(|p| readable.contains(p));
        for snapshot in &mut snapshots {
            if !still_linked.contains(&snapshot.source) {
                *snapshot = Snapshot {
                    source: snapshot.source.clone(),
                    ..Default::default()
                };
            }
        }
    } else {
        snapshots.clear();
        cache.threads.clear();
    }
    if snapshots.is_empty()
        && !has_remote_tracking(&crate::state::state_file_path()).unwrap_or(false)
    {
        return;
    }
    let _ = crate::hook::try_update_state(|state| {
        let disabled = !enabled();
        if disabled {
            snapshots.clear();
        }
        let present: HashSet<String> = snapshots.iter().map(|s| s.source.clone()).collect();
        let retired: HashSet<String> = state
            .t3_threads
            .values()
            .map(|tracking| &tracking.source)
            .filter(|source| {
                is_remote_source(source)
                    && !present.contains(*source)
                    && (disabled || checked_parents.iter().any(|p| source.starts_with(p)))
            })
            .cloned()
            .collect();
        snapshots.extend(retired.into_iter().map(|source| Snapshot {
            source,
            ..Default::default()
        }));
        super::apply_snapshots(state, &snapshots)
    });
    let live: HashSet<&str> = snapshots
        .iter()
        .flat_map(|snapshot| snapshot.threads.iter().map(|thread| thread.id.as_str()))
        .collect();
    // Failed peers keep their read cache so a large first import can progress.
    if cache.threads.len() > super::MAX_THREADS * MAX_PEERS {
        cache.threads.retain(|id, _| live.contains(id.as_str()));
    }
}

fn has_remote_tracking(path: &Path) -> Result<bool> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    let state: crate::state::HookState = serde_json::from_slice(&bytes)?;
    Ok(state
        .t3_threads
        .values()
        .any(|tracking| is_remote_source(&tracking.source)))
}

struct Link {
    environment: String,
    secret_name: String,
    urls: Vec<String>,
}

fn linked_peers(sources: Vec<Source>) -> (Vec<(Source, Link)>, Vec<String>) {
    let mut seen: HashSet<String> = sources
        .iter()
        .filter(|source| super::runtime(source).is_ok_and(|runtime| runtime.is_some()))
        .filter_map(|source| std::fs::read_to_string(source.data().join("environment-id")).ok())
        .map(|environment| environment.trim().to_owned())
        .filter(|environment| super::plain_id(environment))
        .collect();
    let mut peers = Vec::new();
    let mut checked = Vec::new();
    for source in sources {
        let Ok(links) = links(&source) else {
            // A bad metadata read cannot delete the last good state.
            continue;
        };
        checked.push(format!("{}remote:", source.key));
        for link in links {
            if seen.insert(link.environment.clone()) {
                peers.push((source.clone(), link));
            }
        }
    }
    (peers, checked)
}

fn rotate_peers(peers: &mut [(Source, Link)], start: &mut usize) {
    if !peers.is_empty() {
        peers.rotate_left(*start % peers.len());
        *start = (*start + 1) % peers.len();
    }
}

impl Link {
    fn source(&self, parent: &Source) -> Source {
        Source {
            key: format!("{}remote:{}:", parent.key, self.environment),
            owner_pid: None,
            ..parent.clone()
        }
    }
}

fn links(source: &Source) -> Result<Vec<Link>> {
    let path = source.data().join("statev2.sqlite");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(Duration::ZERO)?;
    let has_table: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'peer_environment_links')",
        [],
        |row| row.get(0),
    )?;
    if !has_table {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(
        "SELECT environment_id, secret_name, urls_json, expires_at FROM peer_environment_links
         WHERE fenced = 1 ORDER BY environment_id LIMIT ?1",
    )?;
    let rows = statement.query_map([(MAX_PEERS + 1) as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut links = Vec::new();
    for (count, row) in rows.enumerate() {
        let (environment, secret_name, urls, expires) = row?;
        ensure!(count < MAX_PEERS, "too many T3 peer links");
        if !DateTime::parse_from_rfc3339(&expires).is_ok_and(|at| at > Utc::now()) {
            continue;
        }
        ensure!(super::plain_id(&environment), "invalid T3 peer identity");
        ensure!(super::plain_id(&secret_name), "invalid T3 secret reference");
        let urls: Vec<String> = serde_json::from_str(&urls)?;
        ensure!(urls.len() <= 8, "too many T3 peer addresses");
        links.push(Link {
            environment,
            secret_name,
            urls,
        });
    }
    Ok(links)
}

/// Match T3's HTTPS, loopback HTTP, and encrypted Tailscale HTTP policy.
fn origin(url: &str) -> Result<String> {
    ensure!(url.len() < 2048, "invalid T3 address");
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| anyhow::anyhow!("invalid T3 address"))?;
    ensure!(
        matches!(scheme, "https" | "http"),
        "invalid T3 address scheme"
    );
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    ensure!(!authority.contains('@'), "invalid T3 address authority");
    let (host, port) = if let Some(ipv6) = authority.strip_prefix('[') {
        let (host, suffix) = ipv6
            .split_once(']')
            .ok_or_else(|| anyhow::anyhow!("invalid T3 host"))?;
        let _: Ipv6Addr = host.parse()?;
        ensure!(
            suffix.is_empty() || suffix.starts_with(':'),
            "invalid T3 host suffix"
        );
        (host, suffix.strip_prefix(':').unwrap_or_default())
    } else {
        let (host, port) = authority.split_once(':').unwrap_or((authority, ""));
        ensure!(
            !host.is_empty()
                && host
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-'),
            "invalid T3 host"
        );
        (host, port)
    };
    ensure!(
        port.is_empty() || port.parse::<u16>().is_ok_and(|p| p > 0),
        "invalid T3 port"
    );
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    let ip = host.parse::<IpAddr>().ok();
    let loopback = host == "localhost" || ip.is_some_and(|ip| ip.is_loopback());
    let tailscale = host.ends_with(".ts.net")
        || matches!(ip, Some(IpAddr::V6(ip)) if ip.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0]);
    ensure!(
        scheme == "https" || loopback || tailscale,
        "T3 requires encrypted peer transport"
    );
    Ok(format!("{scheme}://{authority}"))
}

fn credential(source: &Source, name: &str) -> Result<String> {
    ensure!(super::plain_id(name), "invalid T3 secret reference");
    let path = source.data().join("secrets").join(format!("{name}.bin"));
    let metadata = std::fs::symlink_metadata(&path)?;
    ensure!(
        metadata.is_file() && metadata.len() <= 16384,
        "invalid T3 credential file"
    );
    let token = std::fs::read_to_string(path)?;
    ensure!(
        !token.is_empty() && token.bytes().all(|b| (33..=126).contains(&b)),
        "invalid T3 credential"
    );
    Ok(token)
}

struct Response {
    status: u16,
    session: Option<String>,
    body: String,
}

fn quoted(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
    )
}

fn request(
    url: &str,
    token: Option<&str>,
    session: Option<&str>,
    body: Option<&Value>,
    deadline: Instant,
    allowed: fn() -> bool,
) -> Result<Response> {
    ensure!(allowed(), "T3 linked machines disabled");
    let remaining = deadline
        .saturating_duration_since(Instant::now())
        .as_secs_f64()
        .min(3.0);
    // curl treats a timeout rounded down to zero as unlimited.
    ensure!(remaining >= 0.001, "T3 peer poll budget exhausted");
    let mut config = format!(
        "silent\ninclude\nmax-time = {remaining:.3}\nconnect-timeout = 1\nmax-redirs = 0\nproto = \"=http,https\"\nnoproxy = \"*\"\nurl = {}\nheader = \"Accept: application/json, text/event-stream\"\n",
        quoted(url)
    );
    if let Some(token) = token {
        ensure!(
            token.bytes().all(|b| (33..=126).contains(&b)),
            "invalid T3 credential"
        );
        config.push_str(&format!(
            "header = {}\n",
            quoted(&format!("Authorization: Bearer {token}"))
        ));
    }
    if let Some(session) = session {
        ensure!(
            session.len() <= 1024 && session.bytes().all(|b| (33..=126).contains(&b)),
            "invalid MCP session"
        );
        config.push_str(&format!(
            "header = {}\n",
            quoted(&format!("Mcp-Session-Id: {session}"))
        ));
    }
    if let Some(body) = body {
        config.push_str(&format!(
            "header = \"Content-Type: application/json\"\nheader = \"MCP-Protocol-Version: {PROTOCOL}\"\ndata = {}\n",
            quoted(&serde_json::to_string(body)?)
        ));
    }
    let mut command = Command::new("curl");
    // Credentials are stdin-only; ignore .curlrc, redirects and proxy settings.
    command
        .args(["--disable", "--config", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    crate::spawn::configure_detached(&mut command);
    let mut child = command.spawn()?;
    let sent = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("curl stdin unavailable"))?
        .write_all(config.as_bytes());
    if sent.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        bail!("T3 peer request failed");
    }
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("curl stdout unavailable"))?
        .take(MAX_RESPONSE + 1)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() as u64 > MAX_RESPONSE {
        let _ = child.kill();
        let _ = child.wait();
        bail!("T3 peer response exceeded limit");
    }
    ensure!(child.wait()?.success(), "T3 peer request failed");
    let output = std::str::from_utf8(&bytes)?;
    let mut rest = output;
    loop {
        let (headers, body) = rest
            .split_once("\r\n\r\n")
            .ok_or_else(|| anyhow::anyhow!("invalid T3 HTTP response"))?;
        let status = headers
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse::<u16>().ok())
            .ok_or_else(|| anyhow::anyhow!("invalid T3 HTTP status"))?;
        if (100..200).contains(&status) {
            rest = body;
            continue;
        }
        let session = headers
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(key, _)| key.eq_ignore_ascii_case("mcp-session-id"))
            .map(|(_, value)| value.trim().to_owned());
        return Ok(Response {
            status,
            session,
            body: body.to_owned(),
        });
    }
}

fn rpc_result(response: &Response, id: u64) -> Result<Value> {
    ensure!((200..300).contains(&response.status), "T3 MCP HTTP error");
    let value = serde_json::from_str::<Value>(&response.body)
        .ok()
        .filter(|v| v["id"] == id)
        .or_else(|| {
            response
                .body
                .replace("\r\n", "\n")
                .split("\n\n")
                .filter_map(|event| {
                    let data = event
                        .lines()
                        .filter_map(|line| line.strip_prefix("data:").map(str::trim_start))
                        .collect::<Vec<_>>()
                        .join("\n");
                    serde_json::from_str::<Value>(&data)
                        .ok()
                        .filter(|v| v["id"] == id)
                })
                .next()
        })
        .ok_or_else(|| anyhow::anyhow!("invalid T3 MCP response"))?;
    ensure!(value.get("error").is_none(), "T3 MCP request rejected");
    value
        .get("result")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("missing T3 MCP result"))
}

struct Client {
    origin: String,
    token: String,
    session: Option<String>,
    next_id: u64,
    deadline: Instant,
    allowed: fn() -> bool,
}

impl Client {
    fn initialize(&mut self) -> Result<()> {
        self.session = None;
        let id = self.next_id;
        self.next_id += 1;
        let response = self.send(&json!({"jsonrpc":"2.0","id":id,"method":"initialize","params":{
            "protocolVersion":PROTOCOL,"capabilities":{},"clientInfo":{"name":"broglow","version":env!("CARGO_PKG_VERSION")}
        }}))?;
        let result = rpc_result(&response, id)?;
        ensure!(
            result["protocolVersion"] == PROTOCOL,
            "unsupported T3 MCP version"
        );
        self.session = response.session;
        let response = self.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))?;
        ensure!(
            (200..300).contains(&response.status),
            "T3 MCP initialization rejected"
        );
        Ok(())
    }

    fn send(&self, body: &Value) -> Result<Response> {
        request(
            &format!("{}/mcp", self.origin),
            Some(&self.token),
            self.session.as_deref(),
            Some(body),
            self.deadline,
            self.allowed,
        )
    }

    fn call(&mut self, name: &str, arguments: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        let body = json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}});
        let mut response = self.send(&body)?;
        if response.status == 404 && self.session.is_some() {
            self.initialize()?;
            response = self.send(&body)?;
        }
        let result = rpc_result(&response, id)?;
        ensure!(result["isError"] != true, "T3 MCP tool rejected");
        if let Some(content) = result.get("structuredContent") {
            return Ok(content.clone());
        }
        result["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|content| content["type"] == "text")
            .filter_map(|content| {
                content["text"]
                    .as_str()
                    .and_then(|text| serde_json::from_str(text).ok())
            })
            .next()
            .ok_or_else(|| anyhow::anyhow!("missing T3 tool output"))
    }
}

fn connect(
    parent: &Source,
    link: &Link,
    deadline: Instant,
    allowed: fn() -> bool,
) -> Result<Client> {
    let mut answering = None;
    for url in &link.urls {
        let Ok(origin) = origin(url) else { continue };
        let Ok(response) = request(
            &format!("{origin}/.well-known/t3/environment"),
            None,
            None,
            None,
            deadline,
            allowed,
        ) else {
            continue;
        };
        if (200..300).contains(&response.status)
            && serde_json::from_str::<Value>(&response.body).is_ok_and(|v| {
                v["environmentId"] == link.environment
                    && v["capabilities"]["mcpModeLimitHeader"] == true
                    && v["capabilities"]["linkFence"] == true
            })
        {
            answering = Some(origin);
            break;
        }
    }
    let mut client = Client {
        origin: answering.ok_or_else(|| anyhow::anyhow!("T3 peer identity unavailable"))?,
        token: credential(parent, &link.secret_name)?,
        session: None,
        next_id: 1,
        deadline,
        allowed,
    };
    client.initialize()?;
    Ok(client)
}

fn read_peer(
    parent: &Source,
    source: &Source,
    link: &Link,
    deadline: Instant,
    cache: &mut HashMap<String, CachedThread>,
) -> Result<Snapshot> {
    let mut client = connect(parent, link, deadline, enabled)?;
    read_threads(source, &link.environment, &mut client, cache)
}

fn pages(client: &mut Client, tool: &str, key: &str, mut arguments: Value) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    let mut cursor = 0;
    loop {
        arguments["cursor"] = json!(cursor);
        arguments["limit"] = json!(100);
        let page = client.call(tool, arguments.clone())?;
        let values = page[key]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("invalid T3 list page"))?;
        ensure!(
            rows.len() + values.len() <= super::MAX_THREADS,
            "too many T3 peer records"
        );
        rows.extend(values.iter().cloned());
        let next = page
            .get("nextCursor")
            .ok_or_else(|| anyhow::anyhow!("missing T3 list cursor"))?;
        if next.is_null() {
            break;
        }
        let next = next
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("invalid T3 list cursor"))?;
        ensure!(
            next > cursor && !values.is_empty(),
            "nonadvancing T3 list cursor"
        );
        cursor = next;
    }
    Ok(rows)
}

fn read_threads(
    source: &Source,
    environment: &str,
    client: &mut Client,
    cache: &mut HashMap<String, CachedThread>,
) -> Result<Snapshot> {
    let projects = pages(client, "t3_project_list", "projects", json!({}))?;
    let projects: HashMap<&str, &Value> = projects
        .iter()
        .filter_map(|p| p["id"].as_str().map(|id| (id, p)))
        .collect();
    let listed = pages(
        client,
        "t3_thread_list",
        "threads",
        json!({"allProjects":true,"includeSubagents":true}),
    )?;
    let mut snapshot = Snapshot {
        source: source.key.clone(),
        online: true,
        ..Default::default()
    };
    let mut seen = HashSet::new();
    for item in listed {
        if item["relationshipToParent"] == "subagent"
            || item["sideChat"] == true
            || item["archived"] == true
            || !item["deletedAt"].is_null()
        {
            continue;
        }
        let project = item["projectId"].as_str().and_then(|id| projects.get(id));
        if project.is_some_and(|p| !p["deletedAt"].is_null() || !p["archivedAt"].is_null()) {
            continue;
        }
        let id = item["threadId"]
            .as_str()
            .filter(|id| super::valid_thread_id(id))
            .ok_or_else(|| anyhow::anyhow!("invalid T3 thread identity"))?;
        ensure!(seen.insert(id.to_owned()), "duplicate T3 peer thread");
        let full_id = format!("{}{id}", source.key);
        let stamp = serde_json::to_string(&json!([
            item["updatedAt"],
            item["latestRunId"],
            item["status"],
            item["itemCount"]
        ]))?;
        let project = project.and_then(|p| p["workspaceRoot"].as_str());
        let mut thread =
            if let Some(cached) = cache.get(&full_id).filter(|cached| cached.stamp == stamp) {
                cached.thread.clone()
            } else {
                let thread = read_thread(source, environment, id, client)?;
                cache.insert(
                    full_id,
                    CachedThread {
                        stamp,
                        thread: thread.clone(),
                    },
                );
                thread
            };
        if thread.session.status == Status::Done {
            continue;
        }
        if matches!(
            item["status"].as_str(),
            Some("preparing" | "starting" | "running" | "waiting")
        ) && thread.session.status == Status::Inactive
        {
            thread.session.status = Status::Active;
            thread.activity.push_str(":active");
        }
        thread.session.name = item["title"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_owned);
        if thread.session.project_path.is_none() {
            thread.session.project_path = project.map(str::to_owned);
        }
        snapshot.threads.push(thread);
    }
    Ok(snapshot)
}

fn read_thread(
    source: &Source,
    environment: &str,
    id: &str,
    client: &mut Client,
) -> Result<Thread> {
    let mut after = None;
    let mut items = Vec::new();
    let (detail, runs) = loop {
        let mut arguments =
            json!({"threadId":id,"view":"activity","limit":100,"runLimit":50,"maxCharsPerItem":1});
        if let Some(after) = after {
            arguments["afterPosition"] = json!(after);
        }
        let page = client.call("t3_thread_read", arguments)?;
        ensure!(
            page["thread"]["threadId"] == id,
            "T3 thread identity changed"
        );
        let detail = page["thread"].clone();
        let runs = page["recentRuns"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("invalid T3 run records"))?
            .clone();
        let values = page["items"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("invalid T3 activity page"))?;
        ensure!(
            items.len() + values.len() <= MAX_ITEMS,
            "too much T3 thread activity"
        );
        items.extend(values.iter().cloned());
        if page["hasMore"] == false {
            break (detail, runs);
        }
        ensure!(page["hasMore"] == true, "invalid T3 activity pagination");
        let next = page["nextPosition"]
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("invalid T3 activity cursor"))?;
        ensure!(
            after.is_none_or(|old| next > old) && !values.is_empty(),
            "nonadvancing T3 activity cursor"
        );
        after = Some(next);
    };
    project_thread(source, environment, id, &detail, &runs, &items)
}

fn project_thread(
    source: &Source,
    environment: &str,
    id: &str,
    detail: &Value,
    runs: &[Value],
    items: &[Value],
) -> Result<Thread> {
    let items: Vec<&Value> = items
        .iter()
        .filter(|item| {
            item["sourceThreadId"]
                .as_str()
                .is_none_or(|source| source == id)
        })
        .collect();
    let imported_history_only = detail["runCount"] == 0
        && detail["latestRunId"].is_null()
        && detail["activeRunId"].is_null()
        && runs.is_empty()
        && items.iter().all(|item| item["runId"].is_null())
        && ((id.starts_with("import:")
            && detail["createdBy"] == "system"
            && detail["creationSource"] == "server")
            || items.iter().any(|item| {
                item["itemId"].as_str().is_some_and(|id| {
                    id.starts_with("agent-session-import:v2:turn-item:")
                        || id.starts_with("migration:v1:turn-item:")
                })
            }));
    let status = detail["status"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing T3 thread status"))?;
    let pending = detail["pendingRequestCount"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("missing T3 pending request count"))?;
    let active = matches!(
        status,
        "preparing" | "starting" | "running" | "waiting" | "queued"
    ) || !detail["activeRunId"].is_null();
    let last_user = items
        .iter()
        .filter(|item| item["type"] == "user_message" && item["createdBy"] == "user")
        .filter_map(|item| item["updatedAt"].as_str())
        .max();
    let blocking_card = items.iter().any(|item| {
        item["status"] == "waiting"
            && ((item["type"] == "secret_request" && active)
                || (item["type"] == "link_request"
                    && item["updatedAt"]
                        .as_str()
                        .is_some_and(|at| last_user.is_none_or(|user| at >= user))))
    });
    let rolled_back: HashSet<&str> = runs
        .iter()
        .filter(|run| run["status"] == "rolled_back")
        .filter_map(|run| run["runId"].as_str())
        .collect();
    // MCP omits persistent-tool flags and provider background tasks.
    let background = matches!(
        status,
        "completed" | "interrupted" | "failed" | "cancelled" | "waiting"
    ) && items.iter().any(|item| {
        item["type"] == "subagent"
            && matches!(
                item["status"].as_str(),
                Some("pending" | "running" | "waiting")
            )
            && !item["runId"]
                .as_str()
                .is_some_and(|run| rolled_back.contains(run))
    });
    let (status, notification_type) =
        if detail["archived"] == true || detail["relationshipToParent"] == "subagent" {
            (Status::Done, None)
        } else if pending > 0 || blocking_card {
            (Status::NeedsHelp, Some("t3code_request".to_owned()))
        } else if active || background {
            (Status::Active, None)
        } else if status == "failed" {
            (Status::NeedsHelp, Some("t3code_error".to_owned()))
        } else if imported_history_only {
            (Status::Done, None)
        } else {
            (Status::Inactive, None)
        };
    let activity_items: Vec<Value> = items
        .iter()
        .map(|item| json!([item["itemId"], item["status"], item["updatedAt"]]))
        .collect();
    let activity_runs: Vec<Value> = runs
        .iter()
        .map(|run| {
            json!([
                run["runId"],
                run["status"],
                run["requestedAt"],
                run["startedAt"],
                run["completedAt"]
            ])
        })
        .collect();
    let activity = format!(
        "{:016x}",
        super::stable_hash(
            serde_json::to_string(&json!([
                activity_runs,
                activity_items,
                pending,
                detail["activeRunId"],
                background
            ]))?
            .as_bytes()
        )
    );
    let last_updated = std::iter::once(&detail["createdAt"])
        .chain(
            runs.iter()
                .flat_map(|run| [&run["requestedAt"], &run["startedAt"], &run["completedAt"]]),
        )
        .chain(items.iter().map(|item| &item["updatedAt"]))
        .filter_map(|v| {
            v.as_str()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        })
        .max()
        .ok_or_else(|| anyhow::anyhow!("invalid T3 activity time"))?
        .with_timezone(&Utc)
        .to_rfc3339();
    Ok(Thread {
        id: format!("{}{id}", source.key),
        activity,
        session: SessionStatus {
            status,
            notification_type,
            last_updated,
            name: detail["title"]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .map(str::to_owned),
            project_path: detail["worktreePath"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned),
            terminal: Some(TerminalInfo {
                bundle_id: Some(source.bundle.to_owned()),
                host_session_id: Some(format!("{environment}/{id}")),
                ..Default::default()
            }),
            harness: Some("t3code".to_owned()),
            codex_background: false,
            t3_hosted: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    const AT: &str = "2026-11-06T10:00:00Z";
    const LATER: &str = "2026-11-06T10:01:00Z";

    struct Reply {
        path: &'static str,
        auth: bool,
        status: u16,
        body: String,
        session: bool,
    }

    fn server(replies: Vec<Reply>) -> (String, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(8);
            for reply in replies {
                let mut socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        _ => panic!("mock T3 peer did not receive expected request"),
                    }
                };
                socket.set_nonblocking(false).unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                let headers = loop {
                    let mut chunk = [0; 4096];
                    let count = socket.read(&mut chunk).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&chunk[..count]);
                    if let Some(end) = request.windows(4).position(|b| b == b"\r\n\r\n") {
                        break end + 4;
                    }
                };
                let header_text = std::str::from_utf8(&request[..headers]).unwrap();
                assert_eq!(
                    header_text
                        .lines()
                        .next()
                        .unwrap()
                        .split_whitespace()
                        .nth(1),
                    Some(reply.path)
                );
                assert_eq!(
                    header_text
                        .lines()
                        .any(|line| line.to_ascii_lowercase().starts_with("authorization:")),
                    reply.auth
                );
                let length = header_text
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, value)| value.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                while request.len() < headers + length {
                    let mut chunk = [0; 4096];
                    let count = socket.read(&mut chunk).unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&chunk[..count]);
                }
                let session = if reply.session {
                    "Mcp-Session-Id: mock-session\r\n"
                } else {
                    ""
                };
                write!(socket, "HTTP/1.1 {} Mock\r\n{session}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", reply.status, reply.body.len(), reply.body).unwrap();
            }
        });
        (url, worker)
    }

    fn reply(id: u64, result: Value) -> Reply {
        Reply {
            path: "/mcp",
            auth: true,
            status: 200,
            session: false,
            body: json!({"jsonrpc":"2.0","id":id,"result":result}).to_string(),
        }
    }

    fn client(url: String) -> Client {
        Client {
            origin: url,
            token: "test-token".into(),
            session: None,
            next_id: 1,
            deadline: Instant::now() + Duration::from_secs(6),
            allowed: || true,
        }
    }

    fn fixture() -> (tempfile::TempDir, Source) {
        let directory = tempfile::tempdir().unwrap();
        let source = Source::new(directory.path().to_owned());
        std::fs::create_dir_all(source.data().join("secrets")).unwrap();
        (directory, source)
    }

    fn detail() -> Value {
        json!({"threadId":"thread","status":"completed","activeRunId":null,"pendingRequestCount":0,"createdAt":AT,"title":"Thread","worktreePath":null})
    }

    #[test]
    fn credentials_require_verified_identity_and_optout_never_connects() {
        let (_directory, parent) = fixture();
        std::fs::write(parent.data().join("secrets/peer.bin"), "test-token").unwrap();
        let (url, worker) = server(vec![Reply {
            path: "/.well-known/t3/environment",
            auth: false,
            status: 200,
            session: false,
            body: json!({"environmentId":"somebody-else"}).to_string(),
        }]);
        let link = Link {
            environment: "expected".into(),
            secret_name: "peer".into(),
            urls: vec![url.clone()],
        };
        assert!(connect(
            &parent,
            &link,
            Instant::now() + Duration::from_secs(3),
            || true
        )
        .is_err());
        worker.join().unwrap();
        let error = request(
            &url,
            Some("test-token"),
            None,
            None,
            Instant::now() + Duration::from_secs(3),
            || false,
        )
        .err()
        .unwrap();
        assert_eq!(error.to_string(), "T3 linked machines disabled");
        assert!(!format!("{error:?}").contains("test-token"));
    }

    #[test]
    fn verifies_identity_initializes_mcp_and_pages_structured_and_sse_lists() {
        let (_directory, parent) = fixture();
        std::fs::write(parent.data().join("secrets/peer.bin"), "test-token").unwrap();
        let mut initialized = reply(1, json!({"protocolVersion":PROTOCOL}));
        initialized.session = true;
        let sse = format!(
            "event: message\ndata: {}\n\n",
            json!({"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":json!({"projects":[{"id":"second"}],"nextCursor":null}).to_string()}]}})
        );
        let (url, worker) = server(vec![
            Reply {
                path: "/.well-known/t3/environment",
                auth: false,
                status: 200,
                session: false,
                body: json!({"environmentId":"expected","capabilities":{"mcpModeLimitHeader":true,"linkFence":true}}).to_string(),
            },
            initialized,
            Reply {
                path: "/mcp",
                auth: true,
                status: 202,
                session: false,
                body: String::new(),
            },
            reply(
                2,
                json!({"structuredContent":{"projects":[{"id":"first"}],"nextCursor":1}}),
            ),
            Reply {
                path: "/mcp",
                auth: true,
                status: 200,
                session: false,
                body: sse,
            },
        ]);
        let link = Link {
            environment: "expected".into(),
            secret_name: "peer".into(),
            urls: vec![url],
        };
        let mut client = connect(
            &parent,
            &link,
            Instant::now() + Duration::from_secs(6),
            || true,
        )
        .unwrap();
        let projects = pages(&mut client, "t3_project_list", "projects", json!({})).unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[1]["id"], "second");
        worker.join().unwrap();
    }

    #[test]
    fn expired_session_reinitializes_once_but_revocation_and_tool_errors_stop() {
        let mut initialized = reply(2, json!({"protocolVersion":PROTOCOL}));
        initialized.session = true;
        let (url, worker) = server(vec![
            Reply {
                path: "/mcp",
                auth: true,
                status: 404,
                session: false,
                body: String::new(),
            },
            initialized,
            Reply {
                path: "/mcp",
                auth: true,
                status: 202,
                session: false,
                body: String::new(),
            },
            reply(
                1,
                json!({"structuredContent":{"threads":[],"nextCursor":null}}),
            ),
            Reply {
                path: "/mcp",
                auth: true,
                status: 403,
                session: false,
                body: "test-token must not reach an error".into(),
            },
            reply(
                4,
                json!({"isError":true,"content":[{"type":"text","text":"test-token"}]}),
            ),
        ]);
        let mut client = client(url);
        client.session = Some("old".into());
        assert!(client.call("t3_thread_list", json!({})).is_ok());
        let error = client.call("t3_thread_list", json!({})).unwrap_err();
        assert!(!format!("{error:?}").contains("test-token"));
        assert!(client.call("t3_thread_list", json!({})).is_err());
        worker.join().unwrap();
    }

    #[test]
    fn activity_pagination_preserves_dismissal_through_renames_and_status_transitions() {
        let (_directory, parent) = fixture();
        let link = Link {
            environment: "expected".into(),
            secret_name: "peer".into(),
            urls: vec![],
        };
        let source = link.source(&parent);
        let first_item =
            json!({"itemId":"first","type":"user_message","status":"completed","updatedAt":AT});
        let next_item = json!({"itemId":"next","type":"assistant_message","status":"completed","updatedAt":LATER});
        let (url, worker) = server(vec![
            reply(
                1,
                json!({"structuredContent":{"thread":detail(),"recentRuns":[],"items":[first_item],"nextPosition":0,"hasMore":true}}),
            ),
            reply(
                2,
                json!({"structuredContent":{"thread":detail(),"recentRuns":[],"items":[next_item],"nextPosition":1,"hasMore":false}}),
            ),
        ]);
        let thread = read_thread(&source, "expected", "thread", &mut client(url)).unwrap();
        assert_eq!(thread.session.status, Status::Inactive);
        assert_eq!(thread.session.last_updated, "2026-11-06T10:01:00+00:00");
        assert_eq!(thread.session.terminal.as_ref().unwrap().owner_pid, None);
        let mut renamed = detail();
        renamed["title"] = json!("Renamed");
        renamed["updatedAt"] = json!("2026-11-06T12:00:00Z");
        let items = [first_item, next_item];
        let projected =
            project_thread(&source, "expected", "thread", &renamed, &[], &items).unwrap();
        assert_eq!(thread.activity, projected.activity);
        renamed["pendingRequestCount"] = json!(1);
        let waiting = project_thread(&source, "expected", "thread", &renamed, &[], &items).unwrap();
        assert_eq!(waiting.session.status, Status::NeedsHelp);
        assert_ne!(thread.activity, waiting.activity);
        renamed["pendingRequestCount"] = json!(0);
        renamed["status"] = json!("running");
        assert_eq!(
            project_thread(&source, "expected", "thread", &renamed, &[], &items)
                .unwrap()
                .session
                .status,
            Status::Active
        );
        worker.join().unwrap();
    }

    #[test]
    fn peer_snapshot_accepts_mcp_ids_and_skips_hidden_or_archived_projects() {
        let (_directory, parent) = fixture();
        let id = "thread:mcp:client%3Aid:request:0";
        let listed = |id: &str, project: &str| json!({"threadId":id,"projectId":project,"title":"Remote thread","updatedAt":AT,"status":"completed","itemCount":0});
        let mut child = listed("child", "project");
        child["relationshipToParent"] = json!("subagent");
        let mut side_chat = listed("side", "project");
        side_chat["sideChat"] = json!(true);
        let mut metadata = detail();
        metadata["threadId"] = json!(id);
        metadata["pendingRequestCount"] = json!(1);
        let (url, worker) = server(vec![
            reply(
                1,
                json!({"structuredContent":{"projects":[{"id":"project","workspaceRoot":"/remote/project"},{"id":"archived","archivedAt":AT}],"nextCursor":null}}),
            ),
            reply(
                2,
                json!({"structuredContent":{"threads":[listed(id,"project"),child,side_chat,listed("archived-project-thread","archived")],"nextCursor":null}}),
            ),
            reply(
                3,
                json!({"structuredContent":{"thread":metadata,"recentRuns":[],"items":[],"nextPosition":null,"hasMore":false}}),
            ),
        ]);
        let link = Link {
            environment: "remote".into(),
            secret_name: "peer".into(),
            urls: vec![],
        };
        let snapshot = read_threads(
            &link.source(&parent),
            "remote",
            &mut client(url),
            &mut HashMap::new(),
        )
        .unwrap();
        assert_eq!(snapshot.threads.len(), 1);
        assert_eq!(snapshot.threads[0].session.status, Status::NeedsHelp);
        assert_eq!(
            snapshot.threads[0].session.project_path.as_deref(),
            Some("/remote/project")
        );
        assert!(snapshot.threads[0].id.ends_with(id));
        worker.join().unwrap();
    }

    #[test]
    fn pending_subagent_work_ignores_rolled_back_and_inherited_items() {
        let (_directory, source) = fixture();
        let item = json!({"itemId":"task","sourceThreadId":"thread","runId":"run","type":"subagent","status":"pending","updatedAt":LATER});
        let active = project_thread(
            &source,
            "remote",
            "thread",
            &detail(),
            &[],
            std::slice::from_ref(&item),
        )
        .unwrap();
        assert_eq!(active.session.status, Status::Active);
        let rolled = json!({"runId":"run","status":"rolled_back"});
        let idle = project_thread(
            &source,
            "remote",
            "thread",
            &detail(),
            &[rolled],
            std::slice::from_ref(&item),
        )
        .unwrap();
        assert_eq!(idle.session.status, Status::Inactive);
        let mut inherited = item;
        inherited["sourceThreadId"] = json!("ancestor");
        assert_eq!(
            project_thread(&source, "remote", "thread", &detail(), &[], &[inherited])
                .unwrap()
                .session
                .status,
            Status::Inactive
        );
    }

    #[test]
    fn imported_history_stays_hidden_until_a_real_t3_run_without_hiding_new_threads() {
        let (_directory, source) = fixture();
        let mut metadata = detail();
        metadata["status"] = json!("idle");
        metadata["runCount"] = json!(0);
        metadata["latestRunId"] = Value::Null;
        metadata["createdBy"] = json!("system");
        metadata["creationSource"] = json!("server");
        let imported_id = "import:codex:session-id";
        assert_eq!(
            project_thread(&source, "remote", imported_id, &metadata, &[], &[])
                .unwrap()
                .session
                .status,
            Status::Done
        );
        metadata["pendingRequestCount"] = json!(1);
        assert_eq!(
            project_thread(&source, "remote", imported_id, &metadata, &[], &[])
                .unwrap()
                .session
                .status,
            Status::NeedsHelp
        );
        metadata["pendingRequestCount"] = json!(0);
        metadata["status"] = json!("queued");
        assert_eq!(
            project_thread(&source, "remote", imported_id, &metadata, &[], &[])
                .unwrap()
                .session
                .status,
            Status::Active
        );
        metadata["status"] = json!("idle");
        for prefix in [
            "agent-session-import:v2:turn-item:",
            "migration:v1:turn-item:",
        ] {
            let item = json!({"itemId":format!("{prefix}message"),"sourceThreadId":"thread","runId":null,"type":"user_message","status":"completed","updatedAt":AT});
            assert_eq!(
                project_thread(&source, "remote", "thread", &metadata, &[], &[item])
                    .unwrap()
                    .session
                    .status,
                Status::Done
            );
        }
        assert_eq!(
            project_thread(&source, "remote", "thread", &metadata, &[], &[])
                .unwrap()
                .session
                .status,
            Status::Inactive
        );
        let inherited = json!({"itemId":"migration:v1:turn-item:message","sourceThreadId":"ancestor","runId":null,"type":"user_message","status":"completed","updatedAt":AT});
        assert_eq!(
            project_thread(&source, "remote", "thread", &metadata, &[], &[inherited])
                .unwrap()
                .session
                .status,
            Status::Inactive
        );
        metadata["runCount"] = json!(1);
        metadata["latestRunId"] = json!("run");
        metadata["activeRunId"] = json!("run");
        metadata["status"] = json!("running");
        let run = json!({"runId":"run","status":"running","requestedAt":LATER,"startedAt":LATER,"completedAt":null});
        assert_eq!(
            project_thread(&source, "remote", imported_id, &metadata, &[run], &[])
                .unwrap()
                .session
                .status,
            Status::Active
        );
    }

    #[test]
    fn unsafe_origins_and_secret_paths_are_rejected() {
        for url in [
            "http://example.com",
            "http://100.64.1.2",
            "file:///tmp/test",
            "https://user:pass@example.com",
            "http://[::1]evil",
        ] {
            assert!(origin(url).is_err(), "{url}");
        }
        for url in [
            "https://example.com/path",
            "http://127.0.0.1:123",
            "http://localhost:123",
            "http://[::1]:123",
            "http://machine.tail.ts.net",
            "http://[fd7a:115c:a1e0::1]",
        ] {
            assert!(origin(url).is_ok(), "{url}");
        }
        let (_directory, source) = fixture();
        assert!(credential(&source, "../escape").is_err());
        std::fs::write(
            source.data().join("secrets/peer.bin"),
            "token\r\nheader: injected",
        )
        .unwrap();
        assert!(credential(&source, "peer").is_err());
        let error = request(
            "http://127.0.0.1:9",
            None,
            None,
            None,
            Instant::now() + Duration::from_micros(100),
            || true,
        )
        .err()
        .unwrap();
        assert_eq!(error.to_string(), "T3 peer poll budget exhausted");
    }

    #[test]
    fn metadata_ignores_expired_and_unfenced_links_without_writing_database() {
        let (_directory, source) = fixture();
        let connection = Connection::open(source.data().join("statev2.sqlite")).unwrap();
        connection.execute_batch("CREATE TABLE peer_environment_links(environment_id TEXT, secret_name TEXT, urls_json TEXT, expires_at TEXT, fenced INTEGER);
            INSERT INTO peer_environment_links VALUES ('valid','peer','[\"https://example.com\"]','2999-01-01T00:00:00Z',1);
            INSERT INTO peer_environment_links VALUES ('expired','peer','[]','2000-01-01T00:00:00Z',1);
            INSERT INTO peer_environment_links VALUES ('unfenced','peer','[]','2999-01-01T00:00:00Z',0);").unwrap();
        let links = links(&source).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].environment, "valid");
        assert!(is_remote_source(&links[0].source(&source).key));
        assert!(!is_remote_source(&source.key));
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM peer_environment_links", [], |row| row
                    .get::<_, u32>(0))
                .unwrap(),
            3
        );
        let alternate = Source::new(_directory.path().join("alternate"));
        std::fs::create_dir_all(alternate.data()).unwrap();
        std::fs::copy(
            source.data().join("statev2.sqlite"),
            alternate.data().join("statev2.sqlite"),
        )
        .unwrap();
        let (peers, checked) = linked_peers(vec![source.clone(), alternate]);
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].0.key, source.key);
        assert_eq!(checked.len(), 2);
    }

    #[test]
    fn each_peer_eventually_gets_first_poll_budget() {
        let (_directory, source) = fixture();
        let mut next = 0;
        for expected in ["first", "second", "third", "first"] {
            let mut peers: Vec<_> = ["first", "second", "third"]
                .into_iter()
                .map(|environment| {
                    (
                        source.clone(),
                        Link {
                            environment: environment.into(),
                            secret_name: "peer".into(),
                            urls: vec![],
                        },
                    )
                })
                .collect();
            rotate_peers(&mut peers, &mut next);
            assert_eq!(peers[0].1.environment, expected);
        }
    }

    #[test]
    fn active_local_environment_is_not_also_monitored_as_peer() {
        let (_directory, source) = fixture();
        let connection = Connection::open(source.data().join("statev2.sqlite")).unwrap();
        connection.execute_batch("CREATE TABLE peer_environment_links(environment_id TEXT, secret_name TEXT, urls_json TEXT, expires_at TEXT, fenced INTEGER);
            INSERT INTO peer_environment_links VALUES ('local','peer','[]','2999-01-01T00:00:00Z',1);").unwrap();
        std::fs::write(source.data().join("environment-id"), "local").unwrap();
        std::fs::write(
            source.data().join("server-runtime.json"),
            json!({"version":1,"pid":std::process::id()}).to_string(),
        )
        .unwrap();
        assert!(linked_peers(vec![source.clone()]).0.is_empty());
        std::fs::remove_file(source.data().join("server-runtime.json")).unwrap();
        assert_eq!(linked_peers(vec![source]).0.len(), 1);
    }

    #[test]
    fn empty_remote_poll_never_opens_state_lock() {
        if std::env::var_os("BROGLOW_TEST_EMPTY_REMOTE_POLL").is_none() {
            let home = tempfile::tempdir().unwrap();
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "t3::remote::tests::empty_remote_poll_never_opens_state_lock",
                    "--nocapture",
                ])
                .env("HOME", home.path())
                .env("T3CODE_HOME", home.path().join(".t3"))
                .env("BROGLOW_TEST_EMPTY_REMOTE_POLL", "1")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        assert!(!crate::state::state_file_path().exists());
        poll();
        assert!(!crate::config::data_dir().join(".state.lock").exists());
        assert!(!crate::state::state_file_path().exists());
    }

    #[test]
    fn empty_poll_guard_preserves_remote_retirement_and_unreadable_state() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("state.json");
        assert!(!has_remote_tracking(&path).unwrap());
        std::fs::write(&path, r#"{"sessions":{}}"#).unwrap();
        assert!(!has_remote_tracking(&path).unwrap());
        std::fs::write(&path, r#"{"sessions":{},"t3_threads":{"remote":{"source":"t3code:home:remote:environment:","activity":"activity","dismissed":false}}}"#).unwrap();
        assert!(has_remote_tracking(&path).unwrap());
        std::fs::write(&path, "unreadable state").unwrap();
        assert!(has_remote_tracking(&path).is_err());
    }
}
