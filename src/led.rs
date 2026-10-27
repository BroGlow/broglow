//! `broglow led` — daemon that mirrors aggregate session state to the
//! ESP32-C3 SuperMini lamp over USB serial, driving three status LEDs
//! (red / yellow / green).
//!
//! Protocol: one ASCII byte + newline per update.
//!   'R' = needs help   (red LED)
//!   'Y' = inactive     (yellow LED, matching the menu bar icon)
//!   'G' = active       (green LED)
//!   'N' = no sessions  (all LEDs off)
//!
//! The daemon scans Espressif USB serial devices, connects, and sends
//! the current state on every change plus a periodic heartbeat. If the
//! board is unplugged it falls back to scanning until it reappears, so it
//! can be left running unattended.

use std::io::Write;
#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serialport::{SerialPort, SerialPortType};

use crate::config;
use crate::state::{self, aggregate, read_hook_state, Aggregate};

const BAUD: u32 = 115_200;
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const HEARTBEAT: Duration = Duration::from_secs(2);
const RESCAN_INTERVAL: Duration = Duration::from_secs(2);

/// Connection-local animations; reconnect consumes the current events as its baseline.
pub(crate) struct ExpiryPulse {
    seen: Option<String>,
    seen_capacity: Option<String>,
    generation: u64,
    phase: Option<(u8, Option<Instant>, bool)>,
}

impl ExpiryPulse {
    pub(crate) fn new(
        expiry: Option<&state::QuestionExpiry>,
        capacity: Option<&state::CapacityAlert>,
    ) -> Self {
        Self {
            seen: expiry.map(|event| event.id.clone()),
            seen_capacity: capacity.map(|event| event.id.clone()),
            generation: 0,
            phase: None,
        }
    }

    pub(crate) fn sample(
        &mut self,
        expiry: Option<&state::QuestionExpiry>,
        capacity: Option<&state::CapacityAlert>,
        aggregate: u8,
        masked: bool,
        now: Instant,
        wall: chrono::DateTime<chrono::Utc>,
    ) -> (u8, Option<(u64, u8)>) {
        let fresh = |timestamp: &str, seconds| {
            chrono::DateTime::parse_from_rfc3339(timestamp).is_ok_and(|time| {
                let age = wall.signed_duration_since(time);
                age >= chrono::Duration::zero() && age <= chrono::Duration::seconds(seconds)
            })
        };
        if let Some(event) = expiry.filter(|event| self.seen.as_ref() != Some(&event.id)) {
            self.seen = Some(event.id.clone());
            if fresh(&event.occurred_at, 5)
                && aggregate != b'R'
                && !masked
                && !self.phase.is_some_and(|(_, _, capacity)| capacity)
            {
                self.generation = self.generation.wrapping_add(1);
                self.phase = Some((0, None, false));
            }
        }
        if let Some(event) = capacity.filter(|event| self.seen_capacity.as_ref() != Some(&event.id))
        {
            self.seen_capacity = Some(event.id.clone());
            // Capacity wins over an expiry effect, including when the lamp is already red.
            // Allow the daemon's five-second reconciliation tick plus transport polling.
            let fresh = fresh(&event.occurred_at, 10);
            if fresh || self.phase.is_some_and(|(_, _, capacity)| capacity) {
                self.generation = self.generation.wrapping_add(1);
                self.phase = (fresh && !masked).then_some((0, None, true));
            }
        }
        if masked
            || self.phase.is_some_and(|(_, _, is_capacity)| {
                if is_capacity {
                    capacity.is_none()
                } else {
                    aggregate == b'R'
                }
            })
        {
            self.phase = None;
        }
        if let Some((phase, Some(written), capacity)) = self.phase {
            let (duration, last) = if capacity {
                (Duration::from_millis(150), 10)
            } else {
                (Duration::from_millis(500), 5)
            };
            if now.duration_since(written) >= duration {
                self.phase = (phase < last).then_some((phase + 1, None, capacity));
            }
        }
        match self.phase {
            // Start dark so each red flash is distinct even from an existing help signal.
            Some((phase, _, _)) => (
                if phase % 2 == 0 { b'N' } else { b'R' },
                Some((self.generation, phase)),
            ),
            None => (aggregate, None),
        }
    }

    pub(crate) fn acknowledged(&mut self, phase: Option<(u64, u8)>, now: Instant) {
        if let Some((current, written, _)) = self.phase.as_mut() {
            if Some((self.generation, *current)) == phase && written.is_none() {
                *written = Some(now);
            }
        }
    }

    pub(crate) fn poll_interval(&self, normal: Duration, now: Instant) -> Duration {
        match self.phase {
            Some((_, Some(written), true)) => {
                normal.min(Duration::from_millis(150).saturating_sub(now.duration_since(written)))
            }
            _ => normal,
        }
    }
}

pub(crate) fn current_capacity(state: &state::HookState) -> Option<&state::CapacityAlert> {
    state.capacity_alert.as_ref().filter(|alert| {
        state
            .sessions
            .get(&alert.session_id)
            .is_some_and(|session| session.is_codex_capacity())
    })
}

/// Espressif's native USB vendor ID, used by the ESP32-C3 SuperMini.
/// This identifies candidates, not the board model or installed firmware.
const ESPRESSIF_VID: u16 = 0x303A;

pub(crate) fn status_byte(agg: Aggregate) -> u8 {
    match agg {
        Aggregate::Red => b'R',
        Aggregate::Yellow => b'Y',
        Aggregate::Green => b'G',
        Aggregate::None => b'N',
    }
}

/// List Espressif USB serial candidates, most likely first. The vendor ID does
/// not prove that a device is a BroGlow lamp. Pass `--port` to select a device.
///
/// Returns a list so callers can fall through to the next candidate when one
/// can't be opened — e.g. it's held by another app, or it's the `tty.*` twin of
/// a `cu.*` device. On macOS the callout (`cu.*`) device is preferred over its
/// dial-in (`tty.*`) twin; `cu.*` opens without waiting for carrier. (Windows
/// COM names never contain "/tty.", so the penalty is simply zero there.)
pub fn detect_boards() -> Vec<String> {
    let ports = match serialport::available_ports() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    let cu_penalty = |name: &str| if name.contains("/tty.") { 1u8 } else { 0u8 };

    let mut boards: Vec<(u8, String)> = Vec::new();
    for p in ports {
        if let SerialPortType::UsbPort(usb) = &p.port_type {
            if usb.vid == ESPRESSIF_VID {
                let penalty = cu_penalty(&p.port_name);
                boards.push((penalty, p.port_name));
            }
        }
    }

    boards.sort();
    boards.into_iter().map(|(_, name)| name).collect()
}

/// The single most likely USB candidate — used by the TUI for its "board attached?"
/// indicator and the `l` toggle, and by the foreground `led` command.
pub fn detect_board() -> Option<String> {
    detect_boards().into_iter().next()
}

/// Is an Espressif USB candidate attached? Cached with a short TTL so the tray
/// popover — which asks on every state push, potentially several times a second
/// during a busy session — doesn't re-enumerate the USB bus each time. The LED
/// daemon and the TUI call [`detect_board`] directly instead; they already scan
/// on their own slow timers.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn board_present_cached() -> bool {
    const TTL: Duration = Duration::from_millis(1500);
    static CACHE: OnceLock<Mutex<Option<(Instant, bool)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock().unwrap_or_else(|e| e.into_inner());
    let fresh = guard.is_some_and(|(t, _)| t.elapsed() < TTL);
    if !fresh {
        *guard = Some((Instant::now(), detect_board().is_some()));
    }
    guard.map(|(_, present)| present).unwrap_or(false)
}

/// Foreground command (`broglow led`): mirror state to the ESP32-C3 SuperMini
/// lamp until killed. Kept for debugging and for users who'd rather run it
/// standalone than via the menu bar daemon.
pub fn run(port_override: Option<String>) -> anyhow::Result<()> {
    println!(
        "broglow led — mirroring {} to the ESP32-C3 SuperMini lamp over serial",
        state::state_file_path().display()
    );

    loop {
        let path = match port_override.clone().or_else(detect_board) {
            Some(p) => p,
            None => {
                std::thread::sleep(RESCAN_INTERVAL);
                continue;
            }
        };

        match serialport::new(&path, BAUD)
            .timeout(Duration::from_millis(500))
            .open()
        {
            Ok(port) => {
                println!("Connected to {path}");
                if let Err(e) = drive(port, || true, None) {
                    eprintln!("Serial connection lost ({e}); rescanning...");
                }
            }
            Err(e) => {
                eprintln!("Failed to open {path}: {e}; rescanning...");
            }
        }

        std::thread::sleep(RESCAN_INTERVAL);
    }
}

/// Background driver for the menu bar daemon. Idles — touching no serial port —
/// while the LED setting is off; when on, connects to a USB candidate (or the
/// configured `led_port`) and mirrors state, reconnecting on replug. Returns
/// promptly when the user turns the setting off. Never exits.
pub fn run_daemon() -> ! {
    loop {
        let cfg = config::read_config();
        if !cfg.led_enabled {
            std::thread::sleep(RESCAN_INTERVAL);
            continue;
        }
        // USB-assisted pairing pauses this same worker and waits for its lease,
        // so it never opens the cable while the serial/BLE driver still owns it.
        #[cfg(target_os = "macos")]
        let _lease = match worker_lease() {
            Ok(lease) => lease,
            Err(_) => {
                std::thread::sleep(RESCAN_INTERVAL);
                continue;
            }
        };

        if cfg.led_transport == config::LedTransport::Ble {
            if let Err(e) = crate::ble::run(&cfg) {
                publish_status(&cfg, "error", &format!("{e:#}"));
            }
            std::thread::sleep(RESCAN_INTERVAL);
            continue;
        }

        publish_status(&cfg, "searching", "Looking for USB lamp");

        // An explicit pin wins; otherwise try every known board in turn so a
        // busy/unopenable port (e.g. one held by another app, or a non-board
        // device that merely shares a known vendor ID) doesn't block a second
        // board that is actually free.
        let candidates = match cfg.led_port.clone() {
            Some(p) => vec![p],
            None => detect_boards(),
        };

        for path in candidates {
            match serialport::new(&path, BAUD)
                .timeout(Duration::from_millis(500))
                .open()
            {
                Ok(port) => {
                    println!("LED: connected to {path}");
                    // Stay connected until the write fails (unplug) or LED is disabled.
                    if let Err(e) = drive(
                        port,
                        || same_target(&cfg, &config::read_config()),
                        Some(&cfg),
                    ) {
                        publish_status(&cfg, "error", &format!("USB: {e}"));
                    }
                    break;
                }
                Err(_) => continue,
            }
        }

        std::thread::sleep(RESCAN_INTERVAL);
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn worker_lease() -> anyhow::Result<std::fs::File> {
    let path = config::config_file_path().with_file_name(".lamp.lock");
    std::fs::create_dir_all(path.parent().unwrap())?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    file.try_lock()
        .map_err(|e| anyhow::anyhow!("Lamp worker is still stopping; try again shortly: {e}"))?;
    Ok(file)
}

/// Stream state to a connected board until the serial write fails (typically
/// because the board was unplugged) or `keep_running` returns false (the user
/// disabled the LED), whichever comes first.
fn drive(
    mut port: Box<dyn SerialPort>,
    keep_running: impl Fn() -> bool,
    status_config: Option<&config::Config>,
) -> anyhow::Result<()> {
    // Native USB CDC stacks use DTR to learn that a host is listening.
    let _ = port.write_data_terminal_ready(true);

    let mut state = read_hook_state();
    let mut cfg = config::read_config();
    let mut sampled = Instant::now();
    let mut pulse = ExpiryPulse::new(state.question_expiry.as_ref(), current_capacity(&state));
    let mut last_sent = None;
    let mut last_write = Instant::now();

    loop {
        // Keep disk reads at the regular cadence while the lamp blinks faster.
        if last_sent.is_none() || sampled.elapsed() >= POLL_INTERVAL {
            if !keep_running() {
                let _ = port.write_all(b"N\n");
                let _ = port.flush();
                return Ok(());
            }
            state = read_hook_state();
            cfg = config::read_config();
            sampled = Instant::now();
        }
        let output = pulse.sample(
            state.question_expiry.as_ref(),
            current_capacity(&state),
            status_byte(aggregate(&state, cfg.yellow_mode)),
            false,
            Instant::now(),
            chrono::Utc::now(),
        );
        let (byte, phase) = output;
        let changed = last_sent != Some(output);

        if changed || last_write.elapsed() >= HEARTBEAT {
            port.write_all(&[byte, b'\n'])?;
            port.flush()?;
            pulse.acknowledged(phase, Instant::now());
            if let Some(cfg) = status_config {
                publish_status(cfg, "connected", "USB write succeeded");
            }
            if changed {
                println!("state -> {}", byte as char);
            }
            last_sent = Some(output);
            last_write = Instant::now();
        }

        std::thread::sleep(
            pulse
                .poll_interval(POLL_INTERVAL, Instant::now())
                .min(POLL_INTERVAL.saturating_sub(sampled.elapsed())),
        );
    }
}

/// Target changes cancel the old transport before opening the new one. The
/// BLE worker follows saved-lamp changes itself, so adding or forgetting one
/// lamp never reconnects the others.
pub(crate) fn same_target(expected: &config::Config, current: &config::Config) -> bool {
    current.led_enabled
        && expected.led_transport == current.led_transport
        && expected.led_port == current.led_port
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceStatus {
    pub id: String,
    pub state: String,
    pub detail: String,
    pub updated: u64,
    /// The lamp's firmware version read over BLE; absent until read, or when
    /// the firmware predates the version characteristic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firmware: Option<String>,
}

impl DeviceStatus {
    pub(crate) fn new(id: &str, state: &str, detail: &str) -> Self {
        Self {
            id: id.into(),
            state: state.into(),
            detail: detail.into(),
            updated: now_secs(),
            firmware: None,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ConnectionStatus {
    pub state: String,
    pub detail: String,
    #[serde(default)]
    pub devices: Vec<DeviceStatus>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StatusRecord {
    status: ConnectionStatus,
    updated: u64,
    transport: config::LedTransport,
    port: Option<String>,
}

fn status_path() -> std::path::PathBuf {
    config::config_file_path().with_file_name("lamp-status.json")
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn publish_status(cfg: &config::Config, state: &str, detail: &str) {
    publish(cfg, uniform_status(cfg, state, detail));
}

#[cfg(target_os = "macos")]
pub(crate) fn publish_ble_status(cfg: &config::Config, devices: Vec<DeviceStatus>) {
    publish(cfg, summarize_devices(devices));
}

fn summarize_devices(devices: Vec<DeviceStatus>) -> ConnectionStatus {
    let connected = devices.iter().filter(|d| d.state == "connected").count();
    let state = if connected > 0 {
        "connected"
    } else if devices.iter().any(|d| d.state == "error") {
        "error"
    } else if devices.iter().any(|d| d.state == "connecting") {
        "connecting"
    } else if devices.iter().any(|d| d.state == "searching") {
        "searching"
    } else {
        "unavailable"
    };
    ConnectionStatus {
        state: state.into(),
        detail: format!("{connected}/{} lamps connected", devices.len()),
        devices,
    }
}

fn uniform_status(cfg: &config::Config, state: &str, detail: &str) -> ConnectionStatus {
    ConnectionStatus {
        state: state.into(),
        detail: detail.into(),
        devices: if cfg.led_transport == config::LedTransport::Ble {
            cfg.ble_ids()
                .iter()
                .map(|id| DeviceStatus::new(id, state, detail))
                .collect()
        } else {
            Vec::new()
        },
    }
}

fn publish(cfg: &config::Config, status: ConnectionStatus) {
    let record = StatusRecord {
        status,
        updated: now_secs(),
        transport: cfg.led_transport,
        port: cfg.led_port.clone(),
    };
    let path = status_path();
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    if let Ok(json) = serde_json::to_vec(&record) {
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(tmp, path);
        }
    }
}

/// Shared by the tray and separate TUI process; stale daemon state is never green.
pub fn connection_status() -> ConnectionStatus {
    let cfg = config::read_config();
    if !cfg.led_enabled {
        return uniform_status(&cfg, "disabled", "Lamps disabled");
    }
    let record = std::fs::read(status_path())
        .ok()
        .and_then(|s| serde_json::from_slice::<StatusRecord>(&s).ok());
    status_for(&cfg, record, now_secs())
}

fn status_for(cfg: &config::Config, record: Option<StatusRecord>, now: u64) -> ConnectionStatus {
    if let Some(record) = record {
        let ttl = if record.status.state == "connected" {
            6
        } else {
            15
        };
        if record.transport == cfg.led_transport
            && record.port == cfg.led_port
            && now >= record.updated
            && now - record.updated <= ttl
        {
            if cfg.led_transport == config::LedTransport::Ble && !record.status.devices.is_empty() {
                // The worker follows lamp list edits without restarting: show the
                // saved lamps, keeping the others' rows across a Forget or Add.
                let mut devices: Vec<_> = cfg
                    .ble_ids()
                    .iter()
                    .map(|id| {
                        let found = record.status.devices.iter().find(|d| &d.id == id);
                        found.cloned().unwrap_or_else(|| DeviceStatus {
                            updated: record.updated,
                            ..DeviceStatus::new(id, "searching", "Waiting for the lamp worker")
                        })
                    })
                    .collect();
                // Another healthy lamp must never keep a stalled lamp marked connected.
                for device in &mut devices {
                    let ttl = if device.state == "connected" { 6 } else { 15 };
                    if now < device.updated || now - device.updated > ttl {
                        device.state = "unavailable".into();
                        device.detail = "No recent update from this lamp".into();
                    }
                }
                return summarize_devices(devices);
            }
            return record.status;
        }
    }
    uniform_status(
        cfg,
        "unavailable",
        "No recent lamp-worker status; start/reinstall the tray service",
    )
}

#[cfg(test)]
mod transport_tests {
    #[test]
    fn expiry_pulse_preserves_every_phase_until_acknowledged() {
        use super::*;
        let wall = chrono::Utc::now();
        let event = state::QuestionExpiry {
            id: "new".into(),
            occurred_at: wall.to_rfc3339(),
        };
        let mut pulse = ExpiryPulse::new(None, None);
        let start = Instant::now();
        let mut now = start;
        let mut writes = Vec::new();
        for phase in 0..6 {
            let expected = (if phase % 2 == 0 { b'N' } else { b'R' }, Some((1, phase)));
            let output = pulse.sample(Some(&event), None, b'G', false, now, wall);
            assert_eq!(output, expected);
            // A failed/pending write must never advance the animation, even after seconds.
            now += Duration::from_secs(3);
            assert_eq!(
                pulse.sample(Some(&event), None, b'Y', false, now, wall),
                expected
            );
            writes.push(output.0);
            pulse.acknowledged(output.1, now);
            now += Duration::from_millis(499);
            assert_eq!(
                pulse.sample(Some(&event), None, b'Y', false, now, wall),
                expected
            );
            // Duplicate ACK/heartbeat must not extend the phase.
            pulse.acknowledged(output.1, now);
            now += Duration::from_millis(1);
        }
        assert_eq!(writes, b"NRNRNR");
        assert_eq!(
            pulse.sample(Some(&event), None, b'Y', false, now, wall),
            (b'Y', None)
        );
        assert_eq!(
            pulse.sample(Some(&event), None, b'G', false, now, wall),
            (b'G', None)
        );
    }

    #[test]
    fn expiry_pulse_consumes_startup_stale_and_suppressed_events() {
        use super::*;
        let wall = chrono::Utc::now();
        let now = Instant::now();
        let event = state::QuestionExpiry {
            id: "new".into(),
            occurred_at: wall.to_rfc3339(),
        };
        let mut startup = ExpiryPulse::new(Some(&event), None);
        assert_eq!(
            startup.sample(Some(&event), None, b'G', false, now, wall),
            (b'G', None)
        );
        for timestamp in [
            (wall - chrono::Duration::seconds(6)).to_rfc3339(),
            (wall + chrono::Duration::seconds(1)).to_rfc3339(),
            "invalid".into(),
        ] {
            let stale = state::QuestionExpiry {
                occurred_at: timestamp,
                ..event.clone()
            };
            let mut pulse = ExpiryPulse::new(None, None);
            assert_eq!(
                pulse.sample(Some(&stale), None, b'G', false, now, wall),
                (b'G', None)
            );
        }
        for (aggregate, masked) in [(b'R', false), (b'G', true)] {
            let mut pulse = ExpiryPulse::new(None, None);
            assert_eq!(
                pulse.sample(Some(&event), None, aggregate, masked, now, wall),
                (aggregate, None)
            );
            assert_eq!(
                pulse.sample(Some(&event), None, b'G', false, now, wall),
                (b'G', None)
            );
            let second = state::QuestionExpiry {
                id: "second".into(),
                ..event.clone()
            };
            assert_eq!(
                pulse.sample(Some(&second), None, b'G', false, now, wall),
                (b'N', Some((1, 0)))
            );
            pulse.acknowledged(Some((1, 0)), now);
            assert_eq!(
                pulse.sample(Some(&second), None, aggregate, masked, now, wall),
                (aggregate, None)
            );
            assert_eq!(
                pulse.sample(Some(&second), None, b'G', false, now, wall),
                (b'G', None)
            );
        }
    }

    #[test]
    fn capacity_pulse_has_five_acknowledged_150ms_flashes_then_current_aggregate() {
        let wall = chrono::Utc::now();
        let event = state::CapacityAlert {
            id: "session:turn".into(),
            session_id: "session".into(),
            occurred_at: wall.to_rfc3339(),
        };
        for aggregate in *b"RY" {
            let mut pulse = ExpiryPulse::new(None, None);
            let mut now = Instant::now();
            let mut writes = Vec::new();
            for phase in 0..11 {
                let expected = (if phase % 2 == 0 { b'N' } else { b'R' }, Some((1, phase)));
                assert_eq!(
                    pulse.sample(None, Some(&event), aggregate, false, now, wall),
                    expected
                );
                // A slow transport holds every phase until its write succeeds.
                now += Duration::from_secs(3);
                assert_eq!(
                    pulse.sample(None, Some(&event), aggregate, false, now, wall),
                    expected
                );
                writes.push(expected.0);
                pulse.acknowledged(expected.1, now);
                for poll in [POLL_INTERVAL, Duration::from_millis(250)] {
                    assert_eq!(pulse.poll_interval(poll, now), Duration::from_millis(150));
                }
                now += Duration::from_millis(149);
                assert_eq!(
                    pulse.sample(None, Some(&event), aggregate, false, now, wall),
                    expected
                );
                assert_eq!(
                    pulse.poll_interval(POLL_INTERVAL, now),
                    Duration::from_millis(1)
                );
                pulse.acknowledged(expected.1, now);
                now += Duration::from_millis(1);
                assert_eq!(pulse.poll_interval(POLL_INTERVAL, now), Duration::ZERO);
            }
            assert_eq!(writes, b"NRNRNRNRNRN");
            assert_eq!(
                pulse.sample(None, Some(&event), aggregate, false, now, wall),
                (aggregate, None)
            );
            assert_eq!(pulse.poll_interval(POLL_INTERVAL, now), POLL_INTERVAL);
            assert_eq!(
                pulse.sample(None, Some(&event), b'G', false, now, wall),
                (b'G', None)
            );
        }
    }

    #[test]
    fn capacity_pulse_consumes_startup_reconnect_stale_masked_and_cancelled_alerts() {
        let wall = chrono::Utc::now();
        let now = Instant::now();
        let event = state::CapacityAlert {
            id: "session:turn".into(),
            session_id: "session".into(),
            occurred_at: wall.to_rfc3339(),
        };
        let mut startup = ExpiryPulse::new(None, Some(&event));
        assert_eq!(
            startup.sample(None, Some(&event), b'R', false, now, wall),
            (b'R', None)
        );
        for timestamp in [
            (wall - chrono::Duration::seconds(11)).to_rfc3339(),
            (wall + chrono::Duration::seconds(1)).to_rfc3339(),
            "invalid".into(),
        ] {
            let stale = state::CapacityAlert {
                occurred_at: timestamp,
                ..event.clone()
            };
            let mut pulse = ExpiryPulse::new(None, None);
            assert_eq!(
                pulse.sample(None, Some(&stale), b'R', false, now, wall),
                (b'R', None)
            );
        }
        let boundary = state::CapacityAlert {
            occurred_at: (wall - chrono::Duration::seconds(10)).to_rfc3339(),
            ..event.clone()
        };
        let mut delayed = ExpiryPulse::new(None, None);
        assert_eq!(
            delayed.sample(None, Some(&boundary), b'R', false, now, wall),
            (b'N', Some((1, 0)))
        );
        let mut masked = ExpiryPulse::new(None, None);
        assert_eq!(
            masked.sample(None, Some(&event), b'R', true, now, wall),
            (b'R', None)
        );
        assert_eq!(
            masked.sample(None, Some(&event), b'R', false, now, wall),
            (b'R', None)
        );
        for preview in [false, true] {
            let mut pulse = ExpiryPulse::new(None, None);
            assert_eq!(
                pulse.sample(None, Some(&event), b'R', false, now, wall),
                (b'N', Some((1, 0)))
            );
            pulse.acknowledged(Some((1, 0)), now);
            let current = preview.then_some(&event);
            assert_eq!(
                pulse.sample(None, current, b'R', preview, now, wall),
                (b'R', None)
            );
            assert_eq!(
                pulse.sample(None, Some(&event), b'R', false, now, wall),
                (b'R', None)
            );
            let next = state::CapacityAlert {
                id: "session:next".into(),
                ..event.clone()
            };
            assert_eq!(
                pulse.sample(None, Some(&next), b'R', false, now, wall),
                (b'N', Some((2, 0)))
            );
            let mut reconnect = ExpiryPulse::new(None, Some(&next));
            assert_eq!(
                reconnect.sample(None, Some(&next), b'R', false, now, wall),
                (b'R', None)
            );
        }
    }

    #[test]
    fn capacity_takes_priority_over_expiry_without_replaying_either_effect() {
        let wall = chrono::Utc::now();
        let now = Instant::now();
        let expiry = state::QuestionExpiry {
            id: "expiry".into(),
            occurred_at: wall.to_rfc3339(),
        };
        let capacity = state::CapacityAlert {
            id: "session:turn".into(),
            session_id: "session".into(),
            occurred_at: wall.to_rfc3339(),
        };
        let mut pulse = ExpiryPulse::new(None, None);
        assert_eq!(
            pulse.sample(Some(&expiry), None, b'G', false, now, wall),
            (b'N', Some((1, 0)))
        );
        pulse.acknowledged(Some((1, 0)), now);
        assert_eq!(
            pulse.sample(Some(&expiry), Some(&capacity), b'R', false, now, wall),
            (b'N', Some((2, 0)))
        );
        pulse.acknowledged(Some((2, 0)), now);
        let second_expiry = state::QuestionExpiry {
            id: "second".into(),
            ..expiry
        };
        assert_eq!(
            pulse.sample(
                Some(&second_expiry),
                Some(&capacity),
                b'G',
                false,
                now + Duration::from_millis(150),
                wall
            ),
            (b'R', Some((2, 1)))
        );
        assert_eq!(
            pulse.sample(Some(&second_expiry), None, b'G', false, now, wall),
            (b'G', None)
        );
        assert_eq!(
            pulse.sample(
                Some(&second_expiry),
                Some(&capacity),
                b'G',
                false,
                now,
                wall
            ),
            (b'G', None)
        );
    }

    #[test]
    fn capacity_alert_requires_its_session_to_still_need_help() {
        let mut state = state::HookState {
            capacity_alert: Some(state::CapacityAlert {
                id: "session:turn".into(),
                session_id: "session".into(),
                occurred_at: chrono::Utc::now().to_rfc3339(),
            }),
            ..Default::default()
        };
        assert!(current_capacity(&state).is_none());
        state.sessions.insert("session".into(), serde_json::from_value(serde_json::json!({
            "status": "needs_help", "last_updated": "", "harness": "codex", "notification_type": "codex_capacity"
        })).unwrap());
        assert!(current_capacity(&state).is_some());
        state.sessions.get_mut("session").unwrap().status = state::Status::Inactive;
        assert!(current_capacity(&state).is_none());
        state.sessions.get_mut("session").unwrap().status = state::Status::Done;
        assert!(current_capacity(&state).is_none());
    }

    use super::*;

    #[test]
    fn healthy_lamp_does_not_keep_a_stalled_peer_connected() {
        let cfg = config::Config {
            led_transport: config::LedTransport::Ble,
            ble_peripherals: vec!["home".into(), "work".into()],
            ..Default::default()
        };
        let mut devices = vec![
            DeviceStatus::new("home", "connected", "ACK"),
            DeviceStatus::new("work", "connected", "ACK"),
        ];
        devices[0].updated = 100;
        devices[1].updated = 107;
        let record = StatusRecord {
            status: summarize_devices(devices),
            updated: 107,
            transport: cfg.led_transport,
            port: None,
        };
        let json = serde_json::to_value(&record).unwrap();
        let record = serde_json::from_value(json.clone()).unwrap();
        let status = status_for(&cfg, Some(record), 107);
        assert_eq!(status.state, "connected");
        assert_eq!(status.detail, "1/2 lamps connected");
        assert_eq!(status.devices[0].state, "unavailable");
        assert_eq!(status.devices[1].state, "connected");

        // Forget/Add: rows follow the saved list before the worker's next write.
        let record: StatusRecord = serde_json::from_value(json).unwrap();
        let forgot = config::Config {
            ble_peripherals: vec!["work".into(), "desk".into()],
            ..cfg
        };
        let status = status_for(&forgot, Some(record), 107);
        assert_eq!(status.detail, "1/2 lamps connected");
        assert_eq!(status.devices[0].id, "desk");
        assert_eq!(status.devices[0].state, "searching");
        assert_eq!(status.devices[1].id, "work");
        assert_eq!(status.devices[1].state, "connected");
    }

    #[test]
    fn stale_or_wrong_target_is_never_connected() {
        let cfg = config::Config::default();
        let record = || StatusRecord {
            status: ConnectionStatus {
                state: "connected".into(),
                detail: "ACK".into(),
                devices: Vec::new(),
            },
            updated: 100,
            transport: config::LedTransport::Usb,
            port: None,
        };
        assert_eq!(status_for(&cfg, Some(record()), 102).state, "connected");
        // Saved Bluetooth lamps don't matter to the USB worker.
        let forgot = config::Config {
            ble_peripherals: vec!["home".into()],
            ..cfg.clone()
        };
        assert_eq!(status_for(&forgot, Some(record()), 102).state, "connected");
        assert_eq!(status_for(&cfg, Some(record()), 107).state, "unavailable");
        assert_eq!(status_for(&cfg, Some(record()), 99).state, "unavailable");
        let next = config::Config {
            led_transport: config::LedTransport::Ble,
            ..cfg
        };
        assert_eq!(status_for(&next, Some(record()), 102).state, "unavailable");
    }

    #[test]
    fn mapping_and_target_changes() {
        assert_eq!(
            [
                Aggregate::Red,
                Aggregate::Yellow,
                Aggregate::Green,
                Aggregate::None
            ]
            .map(status_byte),
            *b"RYGN"
        );
        let cfg = config::Config {
            led_enabled: true,
            ..Default::default()
        };
        assert!(same_target(&cfg, &cfg));
        let mut next = cfg.clone();
        next.led_enabled = false;
        assert!(!same_target(&cfg, &next));
        next = cfg.clone();
        next.led_transport = config::LedTransport::Ble;
        assert!(!same_target(&cfg, &next));
        next = cfg.clone();
        next.ble_peripherals = vec!["other".into()];
        assert!(same_target(&cfg, &next), "a lamp change keeps the worker");
        next = cfg.clone();
        next.led_port = Some("other".into());
        assert!(!same_target(&cfg, &next));
    }
}
