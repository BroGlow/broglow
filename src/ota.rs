//! `clawlight update <firmware>` — push new ESP32 firmware over the same
//! USB-Serial-JTAG link the LED daemon uses, so the board never needs a cable
//! reflash after its first bootstrap.
//!
//! This is the host side of the serial-OTA protocol defined by the firmware
//! (clawlight-firmware `docs/esp32-serial-ota.md`). Stop-and-wait, one ack per
//! block, so the host never outruns the board's flash writes:
//!
//! ```text
//!   host  OTA:<len>:<crc32>\n     trigger (image length + zlib CRC-32)
//!   board K\n                     ready: inactive slot found and big enough
//!   host  <=4096 bytes>           one block …
//!   board K\n                     … written; send the next
//!   board D\n  (then reboots)     whole-image CRC verified → slot activated
//!   board E\n                     abort at any step; running image untouched
//! ```
//!
//! The menu bar daemon holds the serial port whenever LEDs are enabled, so this
//! flips `led_enabled` off to make it let go, runs the transfer, then restores
//! the setting. The board reboots into the new image on its own and the daemon
//! reconnects on its next scan — no cable, no port juggling.
//!
//! The `<firmware>` argument is an **espflash image** (the output of
//! `espflash save-image`), i.e. the exact bytes that belong in an app
//! partition — not a raw ELF.

use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use serialport::{ClearBuffer, SerialPort};

use crate::config;
use crate::led;

const BAUD: u32 = 115_200;
const BLOCK: usize = 4096;
/// Per-reply read timeout. Generous vs. a sector erase+write (tens of ms), but
/// short enough to notice a board that stalled or rebooted early.
const ACK_TIMEOUT: Duration = Duration::from_secs(5);
/// How long to wait for the daemon to release the port after we disable LEDs.
const PORT_FREE_TIMEOUT: Duration = Duration::from_secs(8);

/// Restores `led_enabled` when the update finishes or errors out, so a failed
/// or interrupted push can't leave the user's LEDs silently disabled.
struct LedSettingGuard {
    restore_to: bool,
}

impl Drop for LedSettingGuard {
    fn drop(&mut self) {
        let mut cfg = config::read_config();
        if cfg.led_enabled != self.restore_to {
            cfg.led_enabled = self.restore_to;
            let _ = config::write_config(&cfg);
        }
    }
}

pub fn run(firmware: String, port_override: Option<String>) -> anyhow::Result<()> {
    let path = Path::new(&firmware);
    let image = std::fs::read(path)
        .with_context(|| format!("Reading firmware image {}", path.display()))?;
    if image.is_empty() {
        bail!("Firmware image {} is empty", path.display());
    }
    let crc = crc32fast::hash(&image);
    println!(
        "clawlight update — {} ({} bytes, crc32 {:08x})",
        path.display(),
        image.len(),
        crc
    );

    // Pick the serial device the same way the daemon does: explicit override,
    // then the configured port, then auto-detect by USB vendor ID.
    let cfg = config::read_config();
    let dev = port_override
        .or_else(|| cfg.led_port.clone())
        .or_else(led::detect_board)
        .context(
            "No Seeed XIAO ESP32-C6 found. Plug it into your Mac's USB port, or \
             pass --port /dev/cu.usbmodemXXX",
        )?;

    // If the daemon is currently driving LEDs it holds the port — flip the
    // setting off so it releases, and arm the guard to turn it back on.
    let _guard = if cfg.led_enabled {
        println!("Pausing LED daemon to free {dev}…");
        let mut off = cfg.clone();
        off.led_enabled = false;
        config::write_config(&off).context("Disabling LEDs to free the serial port")?;
        Some(LedSettingGuard { restore_to: true })
    } else {
        None
    };

    let mut port = open_when_free(&dev)?;
    println!("Connected to {dev}");
    // Native USB CDC wants DTR to know a host is attached; drop any stale bytes
    // buffered from the LED stream before we start the protocol.
    let _ = port.write_data_terminal_ready(true);
    let _ = port.clear(ClearBuffer::All);

    transfer(&mut *port, &image, crc)?;

    println!("\nUpdate complete — the board is rebooting into the new firmware.");
    if _guard.is_some() {
        println!("Re-enabling LEDs; the daemon will reconnect on its next scan.");
    }
    Ok(())
}

/// Open the port, retrying while the daemon finishes letting go of it.
fn open_when_free(dev: &str) -> anyhow::Result<Box<dyn SerialPort>> {
    let deadline = Instant::now() + PORT_FREE_TIMEOUT;
    loop {
        match serialport::new(dev, BAUD).timeout(ACK_TIMEOUT).open() {
            Ok(port) => return Ok(port),
            Err(e) if Instant::now() < deadline => {
                // Almost always "busy" while the daemon is still releasing it.
                let _ = e;
                std::thread::sleep(Duration::from_millis(400));
            }
            Err(e) => {
                return Err(anyhow::Error::new(e).context(format!(
                    "Could not open {dev}. Another process may be holding it — stop a \
                     foreground `clawlight led`, or disable LEDs in the dashboard, then retry."
                )));
            }
        }
    }
}

/// Run the stop-and-wait transfer to completion (board reboots on success).
fn transfer(port: &mut dyn SerialPort, image: &[u8], crc: u32) -> anyhow::Result<()> {
    // Lead with a newline so the trigger always starts a fresh line on the
    // board, even if the LED daemon left a partial byte in flight when it
    // released the port. (An empty line just resolves to "all off" there.)
    let trigger = format!("\nOTA:{}:{:08x}\n", image.len(), crc);
    port.write_all(trigger.as_bytes())?;
    port.flush()?;

    match read_ack(port).context("waiting for the board to accept the update")? {
        b'K' => {}
        b'E' => bail!(
            "Board rejected the update. Either it isn't running the OTA-capable firmware \
             (bootstrap it once over USB), or the image is too big for the slot."
        ),
        other => bail!("Unexpected reply {:?} to the update trigger", other as char),
    }

    let total = image.len().div_ceil(BLOCK);
    for (i, chunk) in image.chunks(BLOCK).enumerate() {
        port.write_all(chunk)?;
        port.flush()?;
        match read_ack(port).with_context(|| format!("waiting for ack on block {}/{total}", i + 1))?
        {
            b'K' => {}
            b'E' => bail!("Board reported a flash-write error on block {}/{total}", i + 1),
            other => bail!("Unexpected reply {:?} on block {}/{total}", other as char, i + 1),
        }
        print_progress(i + 1, total, image.len());
    }

    match read_ack(port).context("waiting for the board's final verdict")? {
        b'D' => Ok(()),
        b'E' => bail!(
            "Image transferred but failed the board's CRC check — nothing was activated, the \
             running firmware is untouched. Retry the update."
        ),
        other => bail!("Unexpected final reply {:?}", other as char),
    }
}

/// Read one newline-terminated reply and return its significant byte (`K`/`D`/
/// `E`). Errors on the port's read timeout — i.e. the board went quiet.
fn read_ack(port: &mut dyn SerialPort) -> anyhow::Result<u8> {
    let mut verdict: Option<u8> = None;
    let mut byte = [0u8; 1];
    loop {
        match port.read(&mut byte) {
            Ok(0) => bail!("board closed the connection"),
            Ok(_) => match byte[0] {
                b'\n' | b'\r' => {
                    if let Some(v) = verdict {
                        return Ok(v);
                    }
                    // Blank line — keep reading for the real reply.
                }
                v @ (b'K' | b'D' | b'E') => verdict = Some(v),
                _ => {} // ignore stray bytes
            },
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                bail!("timed out — the board stopped responding (it may have stalled or reset)")
            }
            Err(e) => return Err(e.into()),
        }
    }
}

fn print_progress(done: usize, total: usize, bytes: usize) {
    let pct = done * 100 / total;
    let filled = pct / 5;
    let bar: String = (0..20).map(|i| if i < filled { '#' } else { ' ' }).collect();
    print!("\r  [{bar}] {pct:3}%  block {done}/{total} ({bytes} bytes)");
    let _ = std::io::stdout().flush();
}

#[cfg(test)]
mod tests {
    /// Byte-for-byte reimplementation of the firmware's `crc32_update`
    /// (clawlight-firmware `src/main.rs`): standard CRC-32, seed 0xFFFFFFFF,
    /// finalize by XOR 0xFFFFFFFF. The whole protocol hinges on this matching
    /// the `crc32fast` value we put in the trigger, so pin it here.
    fn firmware_crc(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &byte in data {
            crc ^= byte as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        crc ^ 0xFFFF_FFFF
    }

    #[test]
    fn host_crc_matches_firmware_crc() {
        let cases: &[&[u8]] = &[
            b"",
            b"clawlight",
            b"123456789", // classic CRC-32 check vector → 0xCBF43926
            &[0u8; 4096],
            &[0xFFu8; 4097],
        ];
        for data in cases {
            assert_eq!(
                crc32fast::hash(data),
                firmware_crc(data),
                "CRC mismatch for {}-byte input",
                data.len()
            );
        }
        // Spot-check the well-known CRC-32 check value while we're here.
        assert_eq!(crc32fast::hash(b"123456789"), 0xCBF4_3926);
    }
}
