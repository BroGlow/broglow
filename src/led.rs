//! `clawlight led` — daemon that mirrors aggregate session state to a Seeed
//! XIAO ESP32-C6 over USB serial, driving three status LEDs (red / yellow /
//! green).
//!
//! Protocol: one ASCII byte + newline per update.
//!   'R' = needs help   (red LED)
//!   'Y' = inactive     (yellow LED, matching the menu bar icon)
//!   'G' = active       (green LED)
//!   'N' = no sessions  (all LEDs off)
//!
//! The daemon scans for the Seeed XIAO ESP32-C6 serial device, connects, and sends
//! the current state on every change plus a periodic heartbeat. If the
//! board is unplugged it falls back to scanning until it reappears, so it
//! can be left running unattended.

use std::io::Write;
use std::time::{Duration, Instant};

use serialport::{SerialPort, SerialPortType};

use crate::config;
use crate::state::{self, aggregate, read_hook_state, Aggregate};

const BAUD: u32 = 115_200;
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const HEARTBEAT: Duration = Duration::from_secs(2);
const RESCAN_INTERVAL: Duration = Duration::from_secs(2);

/// USB vendor ID of the Seeed XIAO ESP32-C6 — the only board clawlight
/// supports. The XIAO wires the ESP32-C6's built-in USB straight to its USB-C
/// port (no CH340/CP210x UART bridge), so it always enumerates under
/// Espressif's native USB-Serial-JTAG vendor ID.
const SEEED_C6_VID: u16 = 0x303A;

fn status_byte(agg: Aggregate) -> u8 {
    match agg {
        Aggregate::Red => b'R',
        Aggregate::Yellow => b'Y',
        Aggregate::Green => b'G',
        Aggregate::None => b'N',
    }
}

/// Detect a connected Seeed XIAO ESP32-C6 by its USB vendor ID and return the
/// serial device path. Matches only the XIAO's native USB-Serial-JTAG VID and
/// never falls back to an arbitrary serial device — so the always-on daemon can
/// scan safely without risking writing status bytes to an unrelated board (an
/// Arduino, GPS, printer, ...). Pass `--port` to force a specific device.
///
/// On macOS the callout (`cu.*`) device is preferred over its dial-in (`tty.*`)
/// twin — `cu.*` opens without waiting for carrier.
pub fn detect_board() -> Option<String> {
    let ports = serialport::available_ports().ok()?;

    let mut candidates: Vec<(u8, String)> = ports
        .into_iter()
        .filter_map(|p| match &p.port_type {
            SerialPortType::UsbPort(usb) if usb.vid == SEEED_C6_VID => {
                // Prefer the callout (cu.*) device over its dial-in (tty.*) twin.
                let cu_penalty = if p.port_name.contains("/tty.") { 1 } else { 0 };
                Some((cu_penalty, p.port_name))
            }
            _ => None,
        })
        .collect();

    candidates.sort();
    candidates.into_iter().next().map(|(_, name)| name)
}

/// Foreground command (`clawlight led`): mirror state to the Seeed XIAO
/// ESP32-C6 until killed. Kept for debugging and for users who'd rather run it
/// standalone than via the menu bar daemon.
pub fn run(port_override: Option<String>) -> anyhow::Result<()> {
    println!(
        "clawlight led — mirroring {} to the Seeed XIAO ESP32-C6 over serial",
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
                if let Err(e) = drive(port, || true) {
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
/// while the LED setting is off; when on, connects to a known board (or the
/// configured `led_port`) and mirrors state, reconnecting on replug. Returns
/// promptly when the user turns the setting off. Never exits.
pub fn run_daemon() -> ! {
    loop {
        let cfg = config::read_config();
        if !cfg.led_enabled {
            std::thread::sleep(RESCAN_INTERVAL);
            continue;
        }

        let Some(path) = cfg.led_port.clone().or_else(detect_board) else {
            std::thread::sleep(RESCAN_INTERVAL);
            continue;
        };

        if let Ok(port) = serialport::new(&path, BAUD)
            .timeout(Duration::from_millis(500))
            .open()
        {
            println!("LED: connected to {path}");
            // Stay connected until the write fails (unplug) or LED is disabled.
            let _ = drive(port, || config::read_config().led_enabled);
        }

        std::thread::sleep(RESCAN_INTERVAL);
    }
}

/// Stream state to a connected board until the serial write fails (typically
/// because the board was unplugged) or `keep_running` returns false (the user
/// disabled the LED), whichever comes first.
fn drive(
    mut port: Box<dyn SerialPort>,
    keep_running: impl Fn() -> bool,
) -> anyhow::Result<()> {
    // Native USB CDC stacks use DTR to learn that a host is listening.
    let _ = port.write_data_terminal_ready(true);

    let mut last_sent: Option<u8> = None;
    let mut last_write = Instant::now();

    loop {
        if !keep_running() {
            return Ok(());
        }

        let byte = status_byte(aggregate(&read_hook_state()));
        let changed = last_sent != Some(byte);

        if changed || last_write.elapsed() >= HEARTBEAT {
            port.write_all(&[byte, b'\n'])?;
            port.flush()?;
            if changed {
                println!("state -> {}", byte as char);
            }
            last_sent = Some(byte);
            last_write = Instant::now();
        }

        std::thread::sleep(POLL_INTERVAL);
    }
}
