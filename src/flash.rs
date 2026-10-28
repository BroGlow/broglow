//! `broglow flash`: write the release's ESP32-C3 images to a USB lamp with
//! espflash, no Python toolchain needed.
//!
//! The four images go to the same fixed offsets the Python tool used. NVS at
//! 0x9000 (Bluetooth bonds, lamp name) is never written, so a flashed lamp
//! reconnects without pairing again. Images come from the release layout next
//! to the executable, or from `--firmware DIR`.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use espflash::connection::{Connection, ResetAfterOperation, ResetBeforeOperation};
use espflash::flasher::{FlashSize, Flasher};
use espflash::image_format::Segment;
use espflash::target::{Chip, ProgressCallbacks};
use serialport::{SerialPort, SerialPortType, UsbPortInfo};

use crate::config;

const ESPRESSIF_VID: u16 = 0x303a;
/// Native USB Serial/JTAG of the ESP32-C3 SuperMini.
const USB_JTAG_PID: u16 = 0x1001;

/// The application image; the only one carrying a version line.
const FIRMWARE_IMAGE: &str = "firmware.bin";

/// Flash offset, file name and the room before the next segment or NVS.
const IMAGES: [(u32, &str, usize); 4] = [
    (0x0000, "bootloader.bin", 0x8000),
    (0x8000, "partitions.bin", 0x1000),
    (0xE000, "boot_app0.bin", 0x2000),
    (0x10000, FIRMWARE_IMAGE, 0x140000),
];

/// Release images the same build wrote; the firmware's own version string.
#[derive(Debug)]
pub struct Images {
    pub version: String,
    segments: Vec<(u32, Vec<u8>)>,
}

#[derive(clap::Args)]
pub struct FlashArgs {
    /// Serial device path; otherwise require exactly one ESP32 303a:1001.
    #[arg(long)]
    port: Option<String>,
    /// Directory with the four release images (default: the ones installed next
    /// to this executable).
    #[arg(long)]
    firmware: Option<PathBuf>,
    /// Write even when the lamp already reports this version or runs unknown firmware.
    #[arg(long)]
    force: bool,
    /// Flash without an interactive prompt.
    #[arg(long)]
    yes: bool,
}

/// Where a release puts the images, relative to the executable: Homebrew's
/// pkgshare, the Mac app bundle, the Mac package and the Windows archive.
pub fn bundled_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    let bin = exe.parent()?;
    [
        "../share/broglow/firmware/esp32-c3-supermini",
        "../Resources/firmware/esp32-c3-supermini",
        "../firmware/esp32-c3-supermini",
        "firmware/esp32-c3-supermini",
    ]
    .iter()
    .map(|relative| bin.join(relative))
    .find(|dir| IMAGES.iter().all(|(_, name, _)| dir.join(name).is_file()))
}

/// Copy the four images (nothing else) into `destination`.
#[cfg(target_os = "macos")]
pub fn copy_images(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    for (_, name, _) in IMAGES {
        std::fs::copy(source.join(name), destination.join(name))
            .with_context(|| format!("Copying firmware image {name}"))?;
    }
    Ok(())
}

pub fn load(dir: &Path) -> Result<Images> {
    let mut segments = Vec::new();
    for (offset, name, room) in IMAGES {
        let path = dir.join(name);
        let data = std::fs::read(&path)
            .with_context(|| format!("Reading firmware image {}", path.display()))?;
        anyhow::ensure!(
            !data.is_empty() && data.len() <= room,
            "{name} is {} bytes; expected 1..={room}",
            data.len()
        );
        segments.push((offset, data));
    }
    let version = firmware_version(&segments[3].1)
        .with_context(|| format!("{} has no version line", dir.join(FIRMWARE_IMAGE).display()))?;
    Ok(Images { version, segments })
}

/// The firmware prints one literal `version=X.Y.Z\n`; find it in the image.
fn firmware_version(image: &[u8]) -> Option<String> {
    let start = image.windows(8).position(|w| w == b"version=")? + 8;
    let end = image[start..].iter().position(|&b| b == b'\n')?;
    let version = std::str::from_utf8(&image[start..start + end]).ok()?;
    (!version.is_empty()
        && version.len() <= 32
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-'))
    .then(|| version.to_string())
}

/// What one flash attempt did; the CLI prints it, the tray shows it.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The lamp already reports the bundled version; nothing was written.
    UpToDate(String),
    Cancelled,
    /// Written, reset and verified: the lamp now reports this version.
    Flashed(String),
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Outcome::UpToDate(version) => write!(f, "Lamp already runs firmware {version}."),
            Outcome::Cancelled => write!(f, "Flash cancelled; nothing was written."),
            Outcome::Flashed(version) => write!(f, "Flashed and verified firmware {version}."),
        }
    }
}

/// The version of the images installed next to this executable, if any.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn bundled_version() -> Option<String> {
    static VERSION: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    VERSION
        .get_or_init(|| {
            let image = std::fs::read(bundled_dir()?.join(FIRMWARE_IMAGE)).ok()?;
            firmware_version(&image)
        })
        .clone()
}

pub fn run(args: FlashArgs) -> Result<()> {
    let mut confirm = |current: &str, next: &str| {
        println!("Flash: firmware {current} -> {next}");
        if args.yes {
            return Ok(true);
        }
        prompt()
    };
    let mut last = 0;
    let mut report = |done: usize, total: usize| {
        let percent = done * 100 / total.max(1);
        if percent / 10 > last / 10 || (percent == 100 && last != 100) {
            println!("  {percent}%");
            last = percent;
        }
    };
    let outcome = update(
        args.port,
        args.firmware,
        args.force,
        &mut confirm,
        &mut report,
    )?;
    println!("{outcome}");
    Ok(())
}

/// Flash the bundled (or `dir`) images onto the single USB lamp (or `port`).
/// `confirm` sees the current and new versions before anything is written;
/// `report` gets bytes written out of the total. Lamp mirroring is paused for
/// the duration and restored afterwards, even on failure.
pub fn update(
    port: Option<String>,
    dir: Option<PathBuf>,
    force: bool,
    confirm: &mut dyn FnMut(&str, &str) -> Result<bool>,
    report: &mut dyn FnMut(usize, usize),
) -> Result<Outcome> {
    let dir = dir.or_else(bundled_dir).context(
        "No firmware images next to this executable; pass --firmware DIR with the release images",
    )?;
    let images = load(&dir)?;
    let path = match port {
        Some(path) => path,
        None => usb_lamp_port()?,
    };
    // Only USB mirroring holds the cable. Otherwise answer "up to date" from a
    // plain INFO first, so a Bluetooth lamp never disconnects for a check.
    let cfg = config::read_config();
    let cable_held = cfg.led_enabled && cfg.led_transport == config::LedTransport::Usb;
    if !cable_held && !force {
        let mut serial = open_lamp(&path)?;
        let current = lamp_version(serial.as_mut())
            .context("Use --force to flash a board without BroGlow firmware")?;
        if current.as_deref() == Some(images.version.as_str()) {
            return Ok(Outcome::UpToDate(images.version));
        }
    }
    // Mirroring may hold the cable; pause it until the lamp is back.
    let resume = pause_mirroring()?;
    let result = flash_lamp(&path, &images, force, confirm, report);
    if resume {
        config::update_config(|cfg| {
            cfg.led_enabled = true;
            Ok(())
        })
        .context("Flashing finished, but lamp control could not be re-enabled")?;
    }
    result
}

fn flash_lamp(
    path: &str,
    images: &Images,
    force: bool,
    confirm: &mut dyn FnMut(&str, &str) -> Result<bool>,
    report: &mut dyn FnMut(usize, usize),
) -> Result<Outcome> {
    let mut serial = open_lamp(path)?;
    let current = match lamp_version(serial.as_mut()) {
        Ok(version) => version,
        Err(_) if force => None,
        Err(e) => return Err(e.context("Use --force to flash a lamp without BroGlow firmware")),
    };
    let current_label = current.as_deref().unwrap_or("unknown");
    if current.as_deref() == Some(images.version.as_str()) && !force {
        return Ok(Outcome::UpToDate(images.version.clone()));
    }
    if !confirm(current_label, &images.version)? {
        return Ok(Outcome::Cancelled);
    }
    drop(serial);
    write(path, images, report)?;
    // The lamp re-enumerates after the reset; then it must report the new version.
    let deadline = Instant::now() + Duration::from_secs(20);
    let flashed = loop {
        match open_lamp(path).and_then(|mut serial| lamp_version(serial.as_mut())) {
            Ok(Some(version)) => break version,
            Ok(None) => bail!("Flashed, but the lamp reports no firmware version"),
            Err(e) if Instant::now() >= deadline => {
                return Err(e.context("The flashed lamp did not answer INFO within 20 seconds"))
            }
            Err(_) => std::thread::sleep(Duration::from_millis(250)),
        }
    };
    anyhow::ensure!(
        flashed == images.version,
        "Flashed, but the lamp reports firmware {flashed} instead of {}",
        images.version
    );
    Ok(Outcome::Flashed(flashed))
}

/// Exactly one native ESP32 USB candidate; INFO then proves the firmware.
pub fn usb_lamp_port() -> Result<String> {
    let mut candidates: Vec<String> = serialport::available_ports()?
        .into_iter()
        .filter(|p| {
            matches!(&p.port_type, SerialPortType::UsbPort(usb)
                if usb.vid == ESPRESSIF_VID && usb.pid == USB_JTAG_PID)
        })
        .map(|p| p.port_name)
        // macOS lists each device twice; cu.* opens without waiting for carrier.
        .filter(|name| !name.contains("/tty."))
        .collect();
    anyhow::ensure!(
        candidates.len() == 1,
        "Expected one ESP32 USB candidate (303a:1001), found {}. Attach the lamp's data cable or use --port",
        candidates.len()
    );
    Ok(candidates.remove(0))
}

/// Disable mirroring while the cable is in use; true when it must be re-enabled.
fn pause_mirroring() -> Result<bool> {
    if !config::read_config().led_enabled {
        return Ok(false); // Never create config.json just to flash.
    }
    config::update_config(|cfg| {
        cfg.led_enabled = false;
        Ok(())
    })?;
    Ok(true)
}

/// A paused worker releases the port within its next poll; keep trying briefly.
fn open_lamp(path: &str) -> Result<Box<dyn SerialPort>> {
    open_lamp_within(path, Duration::from_secs(8))
}

fn open_lamp_within(path: &str, retry: Duration) -> Result<Box<dyn SerialPort>> {
    let deadline = Instant::now() + retry;
    loop {
        match serialport::new(path, 115_200)
            .timeout(Duration::from_millis(100))
            .open()
        {
            Ok(mut serial) => {
                serial.write_data_terminal_ready(true)?;
                serial.clear(serialport::ClearBuffer::Input)?;
                return Ok(serial);
            }
            Err(e) if Instant::now() >= deadline => {
                return Err(anyhow::Error::new(e).context(format!(
                    "Opening USB lamp at {path}; close serial monitors and other BroGlow sessions"
                )))
            }
            Err(_) => std::thread::sleep(Duration::from_millis(200)),
        }
    }
}

/// What the lamp on USB answered to INFO.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LampInfo {
    /// Twelve hex digits of the base MAC, the key of `config.usb_lamps`.
    pub identity: String,
    /// `None` from older BroGlow firmware that only prints the pair line.
    pub version: Option<String>,
}

/// Ask the single USB lamp once, without retries or pausing anything; the tray
/// does this in the background so Lamp details can name the lamp and its version.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn probe(path: &str) -> Result<LampInfo> {
    lamp_info(open_lamp_within(path, Duration::ZERO)?.as_mut())
}

pub fn lamp_version(port: &mut dyn SerialPort) -> Result<Option<String>> {
    Ok(lamp_info(port)?.version)
}

/// INFO over USB: the pair line proves BroGlow firmware; a `version` line
/// follows from firmware 1.2.0 on. An error means no BroGlow reply at all.
pub fn lamp_info(port: &mut dyn SerialPort) -> Result<LampInfo> {
    port.write_all(b"INFO\n")?;
    port.flush()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut paired: Option<(String, Instant)> = None;
    let mut line = Vec::new();
    while Instant::now() < deadline {
        let mut byte = [0];
        match port.read(&mut byte) {
            Ok(1) if byte[0] == b'\n' => {
                if let Some((identity, _)) = &paired {
                    if line.starts_with(b"version=") {
                        let version = std::str::from_utf8(&line[8..])?.trim_end_matches('\r');
                        return Ok(LampInfo {
                            identity: identity.clone(),
                            version: Some(version.to_string()),
                        });
                    }
                } else if let Some(rest) = line.strip_prefix(b"broglow_pair identity=") {
                    let identity = std::str::from_utf8(rest)?
                        .split(' ')
                        .next()
                        .unwrap_or_default()
                        .to_string();
                    anyhow::ensure!(
                        identity.len() == 12 && identity.bytes().all(|b| b.is_ascii_hexdigit()),
                        "Unexpected BroGlow identity in the INFO reply"
                    );
                    paired = Some((identity, Instant::now()));
                }
                line.clear();
            }
            Ok(1) => {
                anyhow::ensure!(line.len() < 256, "Unexpected oversized USB reply");
                line.push(byte[0]);
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => return Err(e.into()),
            Ok(0) => bail!("USB serial connection closed before the firmware reply"),
            _ => {}
        }
        // Older firmware answers INFO with the pair line only.
        if let Some((identity, at)) = &paired {
            if at.elapsed() > Duration::from_millis(500) {
                return Ok(LampInfo {
                    identity: identity.clone(),
                    version: None,
                });
            }
        }
    }
    bail!("No BroGlow INFO reply over USB; the lamp runs no BroGlow firmware or another program holds the port")
}

/// Progress as bytes written over the whole image set.
/// espflash counts chunks per segment; convert to bytes using the segment sizes.
struct Progress<'a> {
    sizes: Vec<(u32, usize)>,
    done: usize,
    segment: usize,
    chunks: usize,
    total: usize,
    report: &'a mut dyn FnMut(usize, usize),
}

impl ProgressCallbacks for Progress<'_> {
    fn init(&mut self, addr: u32, chunks: usize) {
        self.segment = self
            .sizes
            .iter()
            .find(|(a, _)| *a == addr)
            .map_or(0, |(_, size)| *size);
        self.chunks = chunks.max(1);
    }
    fn update(&mut self, current: usize) {
        let written = self.segment * current.min(self.chunks) / self.chunks;
        (self.report)(self.done + written, self.total);
    }
    fn verifying(&mut self) {}
    fn finish(&mut self, _skipped: bool) {
        self.done += self.segment;
        (self.report)(self.done, self.total);
    }
}

/// Enter download mode, check the chip and flash size, write and verify every
/// image, then reset into the new firmware. The port must be free.
pub fn write(path: &str, images: &Images, report: &mut dyn FnMut(usize, usize)) -> Result<()> {
    let info = serialport::available_ports()?
        .into_iter()
        .find(|p| p.port_name == path)
        .and_then(|p| match p.port_type {
            SerialPortType::UsbPort(usb) => Some(usb),
            _ => None,
        })
        .unwrap_or(UsbPortInfo {
            vid: 0,
            pid: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        });
    let serial = serialport::new(path, 115_200)
        .flow_control(serialport::FlowControl::None)
        .open_native()
        .with_context(|| format!("Opening {path} for flashing"))?;
    let connection = Connection::new(
        serial,
        info,
        ResetAfterOperation::HardReset,
        ResetBeforeOperation::DefaultReset,
        115_200,
    );
    let mut flasher = Flasher::connect(connection, true, true, false, None, None)
        .context("Entering download mode; unplug and replug the lamp, then retry")?;
    let chip = flasher.chip();
    anyhow::ensure!(
        chip == Chip::Esp32c3,
        "Connected chip is {chip:?}, not an ESP32-C3; not flashing"
    );
    let size = flasher.flash_detect().context("Reading the flash size")?;
    anyhow::ensure!(
        size == Some(FlashSize::_4Mb),
        "Flash size is {size:?}, not 4 MB; the partition table needs 4 MB"
    );
    let segments: Vec<Segment<'_>> = images
        .segments
        .iter()
        .map(|(addr, data)| Segment::new(*addr, data))
        .collect();
    let mut progress = Progress {
        sizes: images
            .segments
            .iter()
            .map(|(addr, data)| (*addr, data.len()))
            .collect(),
        done: 0,
        segment: 0,
        chunks: 1,
        total: images.segments.iter().map(|(_, d)| d.len()).sum(),
        report,
    };
    flasher
        .write_bins_to_flash(&segments, &mut progress)
        .context("Writing the firmware images")?;
    flasher
        .connection()
        .reset_after(true, chip)
        .context("Resetting the lamp into the new firmware")?;
    Ok(())
}

fn prompt() -> Result<bool> {
    anyhow::ensure!(
        std::io::stdin().is_terminal(),
        "No terminal to confirm on; pass --yes to flash"
    );
    print!("Write the firmware now? [y/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_images(dir: &Path, version: &str) {
        for (_, name, _) in IMAGES {
            let data = if name == FIRMWARE_IMAGE {
                format!("\0\0lamp=%s\nversion={version}\nother").into_bytes()
            } else {
                vec![0xAA; 64]
            };
            std::fs::write(dir.join(name), data).unwrap();
        }
    }

    #[test]
    fn images_load_with_their_version_and_reject_oversized_or_missing_files() {
        let dir = tempfile::tempdir().unwrap();
        write_images(dir.path(), "1.2.3");
        let images = load(dir.path()).unwrap();
        assert_eq!(images.version, "1.2.3");
        assert_eq!(
            images.segments.iter().map(|(a, _)| *a).collect::<Vec<_>>(),
            [0x0, 0x8000, 0xE000, 0x10000]
        );

        std::fs::write(dir.path().join("partitions.bin"), vec![0; 0x1001]).unwrap();
        assert!(load(dir.path())
            .unwrap_err()
            .to_string()
            .contains("partitions.bin"));
        std::fs::write(dir.path().join("partitions.bin"), vec![0; 0x1000]).unwrap();
        std::fs::remove_file(dir.path().join("boot_app0.bin")).unwrap();
        assert!(load(dir.path())
            .unwrap_err()
            .to_string()
            .contains("boot_app0.bin"));
    }

    #[test]
    fn version_line_must_be_a_short_plain_token() {
        assert_eq!(
            firmware_version(b"x version=1.10.0\n").as_deref(),
            Some("1.10.0")
        );
        assert_eq!(
            firmware_version(b"version=unknown\n").as_deref(),
            Some("unknown")
        );
        assert_eq!(firmware_version(b"version=\n"), None);
        assert_eq!(firmware_version(b"version=%s\n"), None);
        assert_eq!(firmware_version(b"version=1.0"), None);
    }
}
