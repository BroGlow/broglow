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

use crate::state::{self, aggregate, read_hook_state, Aggregate};

const BAUD: u32 = 115_200;
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const HEARTBEAT: Duration = Duration::from_secs(2);
const RESCAN_INTERVAL: Duration = Duration::from_secs(2);

/// USB vendor IDs treated as "probably the ESP32 board":
///   0x303A  Espressif — native USB-Serial-JTAG (nanoESP32-C6 "USB" port)
///   0x1A86  WCH — CH340/CH343 USB-UART bridge (nanoESP32-C6 "UART" port)
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
/// dial-in (`tty.*`) device — `cu.*` opens without waiting for carrier.
fn find_port() -> Option<String> {
    let ports = serialport::available_ports().ok()?;

    let mut candidates: Vec<(u8, String)> = ports
        .into_iter()
        .filter_map(|p| {
            let rank = match &p.port_type {
                SerialPortType::UsbPort(usb) if KNOWN_VIDS.contains(&usb.vid) => 0,
                SerialPortType::UsbPort(_) => 1,
                _ if p.port_name.contains("usbmodem") || p.port_name.contains("usbserial") => 2,
                _ => return None,
            };
            // Skip dial-in devices when a callout twin exists.
            let cu_penalty = if p.port_name.contains("/tty.") { 1 } else { 0 };
            Some((rank * 2 + cu_penalty, p.port_name))
        })
        .collect();

    candidates.sort();
    candidates.into_iter().next().map(|(_, name)| name)
}

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
                if let Err(e) = drive(port) {
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

/// Stream state to a connected board until the serial write fails
/// (typically because the board was unplugged).
fn drive(mut port: Box<dyn SerialPort>) -> anyhow::Result<()> {
    // Native USB CDC stacks use DTR to learn that a host is listening.
    let _ = port.write_data_terminal_ready(true);

    let mut last_sent: Option<u8> = None;
    let mut last_write = Instant::now();

    loop {
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
