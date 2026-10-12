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

    let mut last_sent: Option<u8> = None;
    let mut last_write = Instant::now();

    loop {
        if !keep_running() {
            let _ = port.write_all(b"N\n");
            let _ = port.flush();
            return Ok(());
        }

        // Re-read the config each poll so a yellow-mode change from the
        // popover's Settings view takes effect without reconnecting.
        let byte = status_byte(aggregate(
            &read_hook_state(),
            config::read_config().yellow_mode,
        ));
        let changed = last_sent != Some(byte);

        if changed || last_write.elapsed() >= HEARTBEAT {
            port.write_all(&[byte, b'\n'])?;
            port.flush()?;
            if let Some(cfg) = status_config {
                publish_status(cfg, "connected", "USB write succeeded");
            }
            if changed {
                println!("state -> {}", byte as char);
            }
            last_sent = Some(byte);
            last_write = Instant::now();
        }

        std::thread::sleep(POLL_INTERVAL);
    }
}

/// Target changes cancel the old transport before opening the new one.
pub(crate) fn same_target(expected: &config::Config, current: &config::Config) -> bool {
    current.led_enabled
        && expected.led_transport == current.led_transport
        && expected.ble_ids() == current.ble_ids()
        && expected.led_port == current.led_port
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct DeviceStatus {
    pub id: String,
    pub state: String,
    pub detail: String,
    pub updated: u64,
}

impl DeviceStatus {
    pub(crate) fn new(id: &str, state: &str, detail: &str) -> Self {
        Self {
            id: id.into(),
            state: state.into(),
            detail: detail.into(),
            updated: now_secs(),
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
    #[serde(default)]
    peripherals: Vec<String>,
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
        peripherals: cfg.ble_ids(),
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
    if let Some(mut record) = record {
        let ttl = if record.status.state == "connected" {
            6
        } else {
            15
        };
        if record.transport == cfg.led_transport
            && record.peripherals == cfg.ble_ids()
            && record.port == cfg.led_port
            && now >= record.updated
            && now - record.updated <= ttl
        {
            if cfg.led_transport == config::LedTransport::Ble && !record.status.devices.is_empty() {
                // Another healthy lamp must never keep a stalled lamp marked connected.
                for device in &mut record.status.devices {
                    let ttl = if device.state == "connected" { 6 } else { 15 };
                    if now < device.updated || now - device.updated > ttl {
                        device.state = "unavailable".into();
                        device.detail = "No recent update from this lamp".into();
                    }
                }
                return summarize_devices(record.status.devices);
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
            peripherals: cfg.ble_ids(),
            port: None,
        };
        let json = serde_json::to_value(&record).unwrap();
        let record = serde_json::from_value(json).unwrap();
        let status = status_for(&cfg, Some(record), 107);
        assert_eq!(status.state, "connected");
        assert_eq!(status.detail, "1/2 lamps connected");
        assert_eq!(status.devices[0].state, "unavailable");
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
            peripherals: Vec::new(),
            port: None,
        };
        assert_eq!(status_for(&cfg, Some(record()), 102).state, "connected");
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
        assert!(!same_target(&cfg, &next));
        next = cfg.clone();
        next.led_port = Some("other".into());
        assert!(!same_target(&cfg, &next));
    }
}
