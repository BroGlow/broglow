//! `clawlight led` — daemon that mirrors aggregate session state to an ESP32
//! over USB serial, driving three status LEDs (red / yellow / green).
//!
//! Protocol: one ASCII byte + newline per update.
//!   'R' = needs help   (red LED)
//!   'Y' = inactive     (yellow LED, matching the menu bar icon)
//!   'G' = active       (green LED)
//!   'N' = no sessions  (all LEDs off)
//!
//! The daemon scans for a likely ESP32 serial device, connects, and sends
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

/// USB vendor IDs treated as "probably the ESP32 board":
///   0x303A  Espressif — native USB-Serial-JTAG (ESP32-C3/C6/S3 native USB)
///   0x1A86  WCH — CH340/CH343 USB-UART bridge
///   0x10C4  Silicon Labs — CP210x bridges on many classic devkits
const KNOWN_VIDS: [u16; 3] = [0x303A, 0x1A86, 0x10C4];

fn status_byte(agg: Aggregate) -> u8 {
    match agg {
        Aggregate::Red => b'R',
        Aggregate::Yellow => b'Y',
        Aggregate::Green => b'G',
        Aggregate::None => b'N',
    }
}

/// Pick the most likely ESP32 serial device. Prefers USB devices with a
/// known vendor ID, then falls back to anything that looks like a USB
/// serial port. On macOS the callout (`cu.*`) device is preferred over the
/// dial-in (`tty.*`) device — `cu.*` opens without waiting for carrier. On
/// Windows ports are named `COM<n>`, which the USB-type checks below already
/// cover (with a name-based fallback for drivers that don't report a VID).
fn find_port() -> Option<String> {
    let ports = serialport::available_ports().ok()?;

    let mut candidates: Vec<(u8, String)> = ports
        .into_iter()
        .filter_map(|p| {
            let rank = match &p.port_type {
                SerialPortType::UsbPort(usb) if KNOWN_VIDS.contains(&usb.vid) => 0,
                SerialPortType::UsbPort(_) => 1,
                _ if p.port_name.contains("usbmodem")
                    || p.port_name.contains("usbserial")
                    || p.port_name.starts_with("COM") =>
                {
                    2
                }
                _ => return None,
            };
            // Skip dial-in devices when a callout twin exists (macOS only;
            // Windows COM names never contain "/tty.").
            let cu_penalty = if p.port_name.contains("/tty.") { 1 } else { 0 };
            Some((rank * 2 + cu_penalty, p.port_name))
        })
        .collect();

    candidates.sort();
    candidates.into_iter().next().map(|(_, name)| name)
}

/// Strict detection: every currently-present USB device whose vendor ID is a
/// known ESP32 / USB-UART chip, sorted (callout `cu.*` devices preferred over
/// their dial-in `tty.*` twins on macOS). Unlike [`find_port`], this never
/// falls back to an arbitrary serial device — so the always-on daemon can scan
/// safely without writing status bytes to an unrelated board (an Arduino, GPS,
/// printer, ...). Returns a list so callers can fall through to the next board
/// when one can't be opened (e.g. it's held by another app).
pub fn detect_boards() -> Vec<String> {
    let ports = match serialport::available_ports() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    let mut candidates: Vec<(u8, String)> = ports
        .into_iter()
        .filter_map(|p| match &p.port_type {
            SerialPortType::UsbPort(usb) if KNOWN_VIDS.contains(&usb.vid) => {
                let cu_penalty = if p.port_name.contains("/tty.") { 1 } else { 0 };
                Some((cu_penalty, p.port_name))
            }
            _ => None,
        })
        .collect();

    candidates.sort();
    candidates.into_iter().map(|(_, name)| name).collect()
}

/// The single most likely board — used by the TUI for its "board attached?"
/// indicator and the `l` toggle.
pub fn detect_board() -> Option<String> {
    detect_boards().into_iter().next()
}

/// Foreground command (`clawlight led`): mirror state to a board until killed,
/// using the loose [`find_port`] detection. Kept for debugging and for users
/// who'd rather run it standalone than via the menu bar daemon.
pub fn run(port_override: Option<String>) -> anyhow::Result<()> {
    println!(
        "clawlight led — mirroring {} to ESP32 over serial",
        state::state_file_path().display()
    );

    loop {
        let path = match port_override.clone().or_else(find_port) {
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
                    let _ = drive(port, || config::read_config().led_enabled);
                    break;
                }
                Err(_) => continue,
            }
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
