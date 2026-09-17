//! Opt-in BLE control. One existing LED worker drives every remembered nearby lamp.
use crate::config::{self, read_config_strict as read_setup_config, Config, LedTransport};

#[derive(clap::Subcommand)]
pub enum BleCommand {
    /// Discover lamps for 8 seconds (disable the lamp first).
    Scan,
    /// Remember a lamp via its USB cable, then pair it over Bluetooth (macOS).
    Pair {
        /// Explicit USB serial path; otherwise require exactly one ESP32 303a:1001.
        #[arg(long)]
        port: Option<String>,
    },
    /// Add a remembered CoreBluetooth ID and enable all saved BLE lamps.
    Add { id: String },
    /// Alias for add; existing remembered lamps are retained.
    Select { id: String },
    /// Forget a saved lamp (does not erase its Bluetooth bond).
    Remove { id: String },
    /// Name a remembered lamp; an empty name restores its automatic label.
    Rename { id: String, name: String },
    /// Save brightness (0–100); without --id, apply to all lamps and clear overrides.
    Brightness {
        percent: u8,
        /// A remembered CoreBluetooth ID; omit to set all lamps.
        #[arg(long)]
        id: Option<String>,
    },
    /// List remembered lamps and their current connection states.
    List,
    /// Return to USB (disabled until explicitly enabled).
    Usb,
    /// Disable either transport, retaining all remembered lamps.
    Off,
    /// Show the actual background worker's recent status.
    Status,
}

pub fn command(command: BleCommand) -> anyhow::Result<()> {
    if matches!(command, BleCommand::Status) {
        let status = crate::led::connection_status();
        println!("{}: {}", status.state, status.detail);
        for device in status.devices {
            println!("{}  {}: {}", device.id, device.state, device.detail);
        }
        return Ok(());
    }
    let mut cfg = read_setup_config()?;
    match command {
        BleCommand::Scan => return scan(&cfg),
        BleCommand::Pair { port } => return pair(port),
        BleCommand::Add { id } | BleCommand::Select { id } => {
            #[cfg(target_os = "macos")]
            {
                let id = uuid::Uuid::parse_str(&id)?.to_string();
                remember(&mut cfg, id);
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = id;
                anyhow::bail!("BLE control is supported on macOS only");
            }
        }
        BleCommand::Remove { id } => {
            #[cfg(target_os = "macos")]
            let id = uuid::Uuid::parse_str(&id)?.to_string();
            anyhow::ensure!(
                cfg.ble_ids().contains(&id),
                "This lamp is not remembered: {id}"
            );
            cfg.ble_peripherals = cfg
                .ble_ids()
                .into_iter()
                .filter(|saved| saved != &id)
                .collect();
            cfg.ble_peripheral = None;
            cfg.lamp_names.remove(&id);
            cfg.lamp_brightness.remove(&id);
            if cfg.ble_peripherals.is_empty() && cfg.led_transport == LedTransport::Ble {
                cfg.led_enabled = false;
            }
        }
        BleCommand::Rename { id, name } => {
            #[cfg(target_os = "macos")]
            let id = uuid::Uuid::parse_str(&id)?.to_string();
            anyhow::ensure!(
                cfg.ble_ids().contains(&id),
                "This lamp is not remembered: {id}"
            );
            let name = name.trim();
            anyhow::ensure!(
                name.chars().count() <= 40 && !name.chars().any(char::is_control),
                "Use a lamp name of at most 40 characters without control characters"
            );
            if name.is_empty() {
                cfg.lamp_names.remove(&id);
            } else {
                cfg.lamp_names.insert(id, name.to_owned());
            }
        }
        BleCommand::Brightness { percent, id } => {
            anyhow::ensure!(percent <= 100, "Brightness must be between 0 and 100");
            if let Some(id) = id {
                #[cfg(target_os = "macos")]
                let id = uuid::Uuid::parse_str(&id)?.to_string();
                anyhow::ensure!(
                    cfg.ble_ids().contains(&id),
                    "This lamp is not remembered: {id}"
                );
                cfg.lamp_brightness.insert(id, percent);
            } else {
                cfg.led_brightness = percent;
                cfg.lamp_brightness.clear();
            }
        }
        BleCommand::List => {
            let status = crate::led::connection_status();
            for id in cfg.ble_ids() {
                if let Some(device) = status.devices.iter().find(|d| d.id == id) {
                    println!("{id}  {}: {}", device.state, device.detail);
                } else {
                    println!("{id}  {}: {}", status.state, status.detail);
                }
            }
            return Ok(());
        }
        BleCommand::Usb => {
            cfg.led_transport = LedTransport::Usb;
            cfg.led_enabled = false;
        }
        BleCommand::Off => cfg.led_enabled = false,
        BleCommand::Status => unreachable!(),
    }
    config::write_config(&cfg)?;
    println!("Saved lamp settings. The installed tray service applies them automatically.");
    Ok(())
}

/// Use the installed identity and keep setup/rollback alive if the tray quits.
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) fn pair_from_app() -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::process::CommandExt;
        use std::process::Stdio;
        let log = config::config_file_path()
            .with_file_name(format!("pairing-{}.log", std::process::id()));
        std::fs::create_dir_all(log.parent().unwrap())?;
        // A file, not a pipe: closing the tray must not break the child's rollback.
        let result = std::process::Command::new(crate::mac_bundle_executable()?)
            .args(["ble", "pair"])
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(&log)?)
            .status();
        let error = std::fs::read_to_string(&log).unwrap_or_default();
        let _ = std::fs::remove_file(log);
        let status = result?;
        anyhow::ensure!(
            status.success(),
            "USB setup failed ({status}): {}",
            error.trim()
        );
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    anyhow::bail!("Bluetooth setup is supported on macOS only")
}

#[cfg(target_os = "macos")]
fn remember(cfg: &mut Config, id: String) {
    cfg.ble_peripherals = cfg.ble_ids();
    cfg.ble_peripherals.push(id);
    cfg.ble_peripherals.sort();
    cfg.ble_peripherals.dedup();
    cfg.ble_peripheral = None;
    cfg.led_transport = LedTransport::Ble;
    cfg.led_enabled = true;
}

#[cfg(not(target_os = "macos"))]
fn scan(_: &Config) -> anyhow::Result<()> {
    anyhow::bail!("BLE control is supported on macOS only")
}
#[cfg(not(target_os = "macos"))]
fn pair(_: Option<String>) -> anyhow::Result<()> {
    anyhow::bail!("BLE pairing is supported on macOS only")
}
#[cfg(not(target_os = "macos"))]
pub fn run(_: &Config) -> anyhow::Result<()> {
    anyhow::bail!("BLE control is supported on macOS only")
}
#[cfg(target_os = "macos")]
pub(crate) use mac::{preview_brightness, preview_lamp_brightness, run};
#[cfg(target_os = "windows")]
pub(crate) fn preview_brightness(_: u8) {}
#[cfg(target_os = "windows")]
pub(crate) fn preview_lamp_brightness(_: &str, _: u8) -> anyhow::Result<()> {
    anyhow::bail!("Bluetooth lamp identification is supported on macOS only")
}
#[cfg(target_os = "macos")]
use mac::{pair, scan};

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use anyhow::{bail, Context, Result};
    use btleplug::api::{
        Central, CentralEvent, CentralState, CharPropFlags, Manager as _, Peripheral as _,
        ScanFilter, WriteType,
    };
    use btleplug::platform::{Adapter, Manager, Peripheral};
    use fs4::fs_std::FileExt;
    use futures_util::{future::LocalBoxFuture, stream::FuturesUnordered, FutureExt, StreamExt};
    use std::cell::{Cell, RefCell};
    use std::{
        collections::BTreeMap,
        future::Future,
        sync::Mutex,
        time::{Duration, Instant as StdInstant},
    };
    use tokio::sync::{broadcast, Notify};
    use tokio::time::{sleep, timeout, Instant};
    use uuid::Uuid;

    const SERVICE: Uuid = Uuid::from_u128(0x7d2ea28a_f7bd_485a_bd9d_92ad6ecfe93e);
    const STATUS: Uuid = Uuid::from_u128(0x7d2ea28b_f7bd_485a_bd9d_92ad6ecfe93e);
    const BRIGHTNESS: Uuid = Uuid::from_u128(0x7d2ea28c_f7bd_485a_bd9d_92ad6ecfe93e);
    const POLL: Duration = Duration::from_millis(250);
    const HEARTBEAT: Duration = Duration::from_secs(2);
    const OP_TIMEOUT: Duration = Duration::from_secs(10);
    const PREVIEW_DURATION: Duration = Duration::from_secs(2);
    static PREVIEW: Mutex<BTreeMap<Option<Uuid>, BrightnessPreview>> = Mutex::new(BTreeMap::new());
    static PREVIEW_CHANGED: Notify = Notify::const_new();

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct BrightnessPreview {
        percent: u8,
        requested_at: StdInstant,
    }

    impl BrightnessPreview {
        fn active_at(self, now: StdInstant) -> bool {
            now.saturating_duration_since(self.requested_at) < PREVIEW_DURATION
        }

        fn payload(self) -> [u8; 2] {
            [self.percent, 1]
        }
    }

    /// A process-local preview never changes preferences or enables the lamp.
    pub(crate) fn preview_brightness(percent: u8) {
        if percent > 100 {
            return;
        }
        store_preview(None, percent);
    }

    pub(crate) fn preview_lamp_brightness(id: &str, percent: u8) -> Result<()> {
        let target = Uuid::parse_str(id)?;
        let id = target.to_string();
        anyhow::ensure!(percent <= 100, "Brightness must be between 0 and 100");
        let cfg = read_setup_config()?;
        anyhow::ensure!(
            cfg.ble_ids().contains(&id),
            "This lamp is not remembered: {id}"
        );
        anyhow::ensure!(
            cfg.led_enabled && cfg.led_transport == LedTransport::Ble,
            "Enable Bluetooth lamps before previewing brightness"
        );
        anyhow::ensure!(
            crate::led::connection_status()
                .devices
                .iter()
                .any(|device| { device.id == id && device.state == "connected" }),
            "This lamp is not connected; wait for it to reconnect"
        );
        store_preview(Some(target), percent);
        Ok(())
    }

    fn store_preview(target: Option<Uuid>, percent: u8) {
        let now = StdInstant::now();
        let mut previews = PREVIEW.lock().unwrap_or_else(|error| error.into_inner());
        previews.retain(|_, preview| preview.active_at(now));
        previews.insert(
            target,
            BrightnessPreview {
                percent,
                requested_at: now,
            },
        );
        drop(previews);
        PREVIEW_CHANGED.notify_waiters();
    }

    fn current_preview(id: Uuid) -> Option<BrightnessPreview> {
        let now = StdInstant::now();
        let previews = PREVIEW.lock().unwrap_or_else(|error| error.into_inner());
        previews
            .get(&None)
            .into_iter()
            .chain(previews.get(&Some(id)))
            .copied()
            .filter(|preview| preview.active_at(now))
            .max_by_key(|preview| preview.requested_at)
    }

    fn pending_preview(
        last: Option<BrightnessPreview>,
        current: Option<BrightnessPreview>,
        now: StdInstant,
    ) -> Option<BrightnessPreview> {
        current.filter(|preview| Some(*preview) != last && preview.active_at(now))
    }

    fn radio_lock() -> Result<std::fs::File> {
        anyhow::ensure!(
            std::env::current_exe()? == crate::mac_bundle_executable()?,
            "Run clawlight install; Bluetooth requires the installed ClawLight.app identity"
        );
        let path = config::config_file_path().with_file_name(".ble.lock");
        std::fs::create_dir_all(path.parent().unwrap())?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        anyhow::ensure!(
            file.try_lock_exclusive()?,
            "Bluetooth is busy; disable the lamp and wait before scanning"
        );
        Ok(file)
    }

    fn runtime() -> Result<tokio::runtime::Runtime> {
        Ok(tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()?)
    }

    // Every radio wait is both bounded and interruptible, including discovery/connect.
    async fn wait<T>(
        future: impl Future<Output = Result<T>>,
        keep: &impl Fn() -> bool,
    ) -> Result<T> {
        wait_for(future, keep, OP_TIMEOUT).await
    }

    async fn wait_for<T>(
        future: impl Future<Output = Result<T>>,
        keep: &impl Fn() -> bool,
        limit: Duration,
    ) -> Result<T> {
        tokio::pin!(future);
        let deadline = sleep(limit);
        tokio::pin!(deadline);
        loop {
            if !keep() {
                bail!("Lamp settings changed");
            }
            tokio::select! {
                result = &mut future => return result,
                _ = &mut deadline => bail!("Bluetooth timed out; check power and Privacy & Security > Bluetooth > ClawLight"),
                _ = sleep(POLL) => {},
            }
        }
    }

    async fn adapter(keep: &impl Fn() -> bool) -> Result<Adapter> {
        wait(
            async {
                let manager = Manager::new().await?;
                manager
                    .adapters()
                    .await?
                    .into_iter()
                    .next()
                    .context("No Bluetooth adapter; check Bluetooth permission for ClawLight")
            },
            keep,
        )
        .await
    }

    async fn start(adapter: &Adapter, keep: &impl Fn() -> bool) -> Result<()> {
        wait(
            async {
                match adapter.adapter_state().await? {
                    CentralState::PoweredOn => {}
                    CentralState::PoweredOff => {
                        bail!("Bluetooth is off; turn it on in System Settings")
                    }
                    _ => bail!(
                        "Bluetooth unavailable; allow ClawLight in Privacy & Security > Bluetooth"
                    ),
                }
                adapter
                    .start_scan(ScanFilter {
                        services: vec![SERVICE],
                    })
                    .await?;
                Ok(())
            },
            keep,
        )
        .await
    }

    pub(super) fn scan(cfg: &Config) -> Result<()> {
        anyhow::ensure!(
            !cfg.led_enabled,
            "Run clawlight ble off, wait 3 seconds, then scan"
        );
        let _lock = radio_lock()?;
        runtime()?.block_on(async {
            let keep = || !config::read_config().led_enabled;
            let adapter = adapter(&keep).await?;
            let result = async {
                start(&adapter, &keep).await?;
                wait(async { sleep(Duration::from_secs(8)).await; Ok(()) }, &keep).await?;
                let mut found = 0;
                for p in adapter.peripherals().await? {
                    if let Some(props) = p.properties().await? {
                        if props.services.contains(&SERVICE) {
                            println!("{}  {}", p.id(), props.local_name.unwrap_or_default());
                            found += 1;
                        }
                    }
                }
                println!("{found} advertised lamp(s). Select the intended ID; the worker verifies its writable characteristic on connection.");
                Ok(())
            }.await;
            let _ = timeout(Duration::from_secs(1), adapter.stop_scan()).await;
            result
        })
    }

    /// Only probe a unique native ESP32 USB candidate; INFO proves firmware identity.
    fn pair_port(
        override_path: Option<String>,
        ports: &[serialport::SerialPortInfo],
    ) -> Result<String> {
        if let Some(path) = override_path {
            return Ok(path);
        }
        let mut candidates: Vec<_> = ports
            .iter()
            .filter(|p| {
                matches!(&p.port_type, serialport::SerialPortType::UsbPort(usb)
                if usb.vid == 0x303a && usb.pid == 0x1001)
            })
            .map(|p| p.port_name.clone())
            .collect();
        let twins = candidates.clone();
        candidates.retain(|path| {
            !path.starts_with("/dev/tty.")
                || !twins.contains(&path.replacen("/dev/tty.", "/dev/cu.", 1))
        });
        candidates.sort();
        candidates.dedup();
        anyhow::ensure!(candidates.len() == 1,
            "Expected one ESP32 USB candidate (303a:1001), found {}. Attach the lamp's data cable or use --port with its verified serial path", candidates.len());
        Ok(candidates.remove(0))
    }

    fn parse_pair_identity(line: &str) -> Result<[u8; 6]> {
        let line = line.trim_end_matches(['\r', '\n']);
        let identity = line
            .strip_prefix("clawlight_pair identity=")
            .and_then(|s| s.strip_suffix(&format!(" service={SERVICE}")))
            .context(
                "USB reply is not the expected ClawLight pairing protocol; update the C3 firmware",
            )?;
        anyhow::ensure!(
            identity.len() == 12
                && identity
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "USB reply has an invalid six-byte lamp identity"
        );
        let mut bytes = [0; 6];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&identity[i * 2..i * 2 + 2], 16)?;
        }
        Ok(bytes)
    }

    fn usb_identity(port: &mut dyn serialport::SerialPort, command: &[u8]) -> Result<[u8; 6]> {
        port.write_all(command)?;
        port.flush()?;
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut line = Vec::new();
        while std::time::Instant::now() < deadline {
            let mut byte = [0];
            match port.read(&mut byte) {
                Ok(1) if byte[0] == b'\n' => {
                    if line.starts_with(b"clawlight_pair") {
                        return parse_pair_identity(std::str::from_utf8(&line)?);
                    }
                    line.clear(); // Ignore ordinary firmware diagnostic lines.
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
        }
        bail!("No ClawLight INFO/PAIR reply over USB; close serial monitors and flash the current C3 firmware")
    }

    fn identity_matches(props: &btleplug::api::PeripheralProperties, identity: &[u8; 6]) -> bool {
        props.services.contains(&SERVICE)
            && props
                .manufacturer_data
                .get(&0xffff)
                .is_some_and(|bytes| bytes.as_slice() == identity)
    }

    pub(super) fn pair(port_override: Option<String>) -> Result<()> {
        let ports = if port_override.is_some() {
            Vec::new()
        } else {
            serialport::available_ports()?
        };
        let path = pair_port(port_override, &ports)?;
        let lock_path = config::config_file_path().with_file_name(".pair.lock");
        std::fs::create_dir_all(lock_path.parent().unwrap())?;
        let _setup = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        anyhow::ensure!(
            _setup.try_lock_exclusive()?,
            "Lamp setup is already running; wait for it to finish"
        );
        let original_bytes = match std::fs::read(config::config_file_path()) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        let original: Config = original_bytes
            .as_deref()
            .map(serde_json::from_slice)
            .transpose()?
            .unwrap_or_default();
        let mut paused = original.clone();
        paused.led_enabled = false;
        config::write_config(&paused)?;
        let result = (|| {
            let deadline = std::time::Instant::now() + Duration::from_secs(8);
            let _worker = loop {
                match crate::led::worker_lease() {
                    Ok(lease) => break lease,
                    Err(_) if std::time::Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(100))
                    }
                    Err(error) => return Err(error),
                }
            };
            let _radio = radio_lock()?;
            let mut serial = serialport::new(&path, 115_200)
                .timeout(Duration::from_millis(100))
                .open()
                .with_context(|| {
                    format!("Opening USB lamp at {path}; close other serial monitors")
                })?;
            serial.write_data_terminal_ready(true)?;
            serial.clear(serialport::ClearBuffer::Input)?;
            let identity = usb_identity(serial.as_mut(), b"INFO\n")?;
            println!("Verified ClawLight firmware on {path}; opening its pairing window.");
            anyhow::ensure!(
                usb_identity(serial.as_mut(), b"PAIR\n")? == identity,
                "USB lamp identity changed between INFO and PAIR; selection was not saved"
            );
            drop(serial);
            let id = runtime()?.block_on(async {
                let keep = || !config::read_config().led_enabled;
                let adapter = adapter(&keep).await?;
                let found = async {
                    start(&adapter, &keep).await?;
                    wait(async { sleep(Duration::from_secs(8)).await; Ok(()) }, &keep).await?;
                    let mut matching = Vec::new();
                    for p in adapter.peripherals().await? {
                        if p.properties().await?.is_some_and(|props| identity_matches(&props, &identity)) {
                            matching.push(p.id().to_string());
                        }
                    }
                    anyhow::ensure!(matching.len() == 1,
                        "Expected exactly one BLE advertisement matching the USB lamp, found {}; check Bluetooth and current firmware", matching.len());
                    Ok(matching.remove(0))
                }.await;
                let _ = timeout(Duration::from_secs(1), adapter.stop_scan()).await;
                found
            })?;
            let mut latest = read_setup_config()?;
            remember(&mut latest, id.clone());
            config::write_config(&latest)?;
            println!("Remembered lamp {id}. The tray service will connect automatically; approve any macOS pairing request for this lamp. Existing saved lamps are retained.");
            Ok(())
        })();
        if result.is_err() {
            let mut latest = read_setup_config()
                .context("Pairing failed; cannot safely restore unreadable settings")?;
            if serde_json::to_value(&latest)? == serde_json::to_value(&paused)? {
                if let Some(bytes) = original_bytes {
                    let path = config::config_file_path();
                    let tmp = path.with_extension(format!("{}.restore.tmp", std::process::id()));
                    std::fs::write(&tmp, bytes)?;
                    std::fs::rename(tmp, path)?;
                } else {
                    std::fs::remove_file(config::config_file_path())?;
                }
            } else if !latest.led_enabled {
                latest.led_enabled = original.led_enabled;
                config::write_config(&latest)
                    .context("Pairing failed; could not restore previous lamp enable setting")?;
            }
        }
        result
    }

    pub(crate) fn run(cfg: &Config) -> Result<()> {
        let ids = cfg.ble_ids();
        anyhow::ensure!(
            !ids.is_empty(),
            "Add lamps: clawlight ble off; clawlight ble scan; clawlight ble add <ID>"
        );
        let _lock = radio_lock()?;
        runtime()?.block_on(async {
            let keep = || crate::led::same_target(cfg, &config::read_config());
            crate::led::publish_status(cfg, "searching", "Initializing Bluetooth");
            let adapter = adapter(&keep).await?;
            let statuses = RefCell::new(
                ids.iter()
                    .map(|id| {
                        crate::led::DeviceStatus::new(
                            id,
                            "searching",
                            "Waiting for a fresh advertisement",
                        )
                    })
                    .collect::<Vec<_>>(),
            );
            let update = |id: &str, state: &str, detail: &str| {
                let mut devices = statuses.borrow_mut();
                if let Some(device) = devices.iter_mut().find(|d| d.id == id) {
                    *device = crate::led::DeviceStatus::new(id, state, detail);
                }
                crate::led::publish_ble_status(cfg, devices.clone());
            };
            let (advertisements, _) = broadcast::channel(ids.len().max(8) * 2);
            let scan_error = RefCell::new(None);
            // All futures share one adapter/runtime. A missing lamp or a pairing
            // prompt cannot delay another lamp's writes or heartbeat.
            let mut tasks = Vec::new();
            for id in &ids {
                tasks.push(
                    lamp_loop(id, &advertisements, &scan_error, &keep, &update).boxed_local(),
                );
            }
            tasks.push(discover(&adapter, &ids, &advertisements, &scan_error, &keep).boxed_local());
            run_all(tasks).await;
            Ok(())
        })
    }

    type Advertisements = broadcast::Sender<(String, Peripheral)>;

    async fn run_all(tasks: Vec<LocalBoxFuture<'_, ()>>) {
        let mut pending: FuturesUnordered<_> = tasks.into_iter().collect();
        while pending.next().await.is_some() {}
    }

    async fn discover(
        adapter: &Adapter,
        ids: &[String],
        advertisements: &Advertisements,
        scan_error: &RefCell<Option<String>>,
        keep: &impl Fn() -> bool,
    ) {
        while keep() {
            let result = async {
                let mut events = adapter.events().await?;
                start(adapter, keep).await?;
                *scan_error.borrow_mut() = None;
                while keep() {
                    tokio::select! {
                        event = events.next() => match event {
                            Some(CentralEvent::ServicesAdvertisement { id, services }) => {
                                let candidate = id.to_string();
                                if selected_advertisement(&candidate, &services, ids) {
                                    if let Ok(peripheral) = adapter.peripheral(&id).await {
                                        let _ = advertisements.send((candidate, peripheral));
                                    }
                                }
                            }
                            Some(CentralEvent::StateUpdate(state)) if state != CentralState::PoweredOn => {
                                bail!("Bluetooth unavailable; check power and ClawLight permission")
                            }
                            None => bail!("Bluetooth discovery stopped"),
                            _ => {}
                        },
                        _ = sleep(POLL) => {}
                    }
                }
                Ok(())
            }.await;
            // Only this shared discovery loop ever stops the scanner.
            let _ = timeout(Duration::from_secs(1), adapter.stop_scan()).await;
            if let Err(error) = result {
                *scan_error.borrow_mut() = Some(format!("{error:#}"));
            }
            let _ = wait(
                async {
                    sleep(HEARTBEAT).await;
                    Ok(())
                },
                keep,
            )
            .await;
        }
    }

    async fn lamp_loop(
        id: &str,
        advertisements: &Advertisements,
        scan_error: &RefCell<Option<String>>,
        keep: &impl Fn() -> bool,
        update: &impl Fn(&str, &str, &str),
    ) {
        let publish = |state: &str, detail: &str| update(id, state, detail);
        while keep() {
            // Subscribe anew after each disconnect: cached/queued advertisements
            // cannot reconnect a stale native CoreBluetooth peripheral handle.
            let mut fresh = advertisements.subscribe();
            publish(
                "searching",
                "Waiting for a fresh advertisement; absent lamps are retried automatically",
            );
            let discovered = wait(
                async {
                    loop {
                        match fresh.recv().await {
                            Ok((candidate, peripheral)) if candidate == id => {
                                return Ok(peripheral)
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                bail!("Bluetooth discovery stopped")
                            }
                            _ => {}
                        }
                    }
                },
                keep,
            )
            .await;
            match discovered {
                Ok(peripheral) => {
                    publish("connecting", "Connecting to saved lamp");
                    let result = drive(&peripheral, keep, &publish).await;
                    // Cancelling a Rust future doesn't cancel a native pending connect.
                    let _ = timeout(Duration::from_secs(2), peripheral.disconnect()).await;
                    if let Err(error) = result {
                        if keep() {
                            publish("error", &format!("{error:#}"));
                        }
                    }
                }
                Err(_) if keep() => {
                    if let Some(error) = scan_error.borrow().as_ref() {
                        publish("error", error);
                    } else {
                        publish("searching", "Not nearby; will reconnect automatically");
                    }
                }
                Err(_) => {}
            }
            let _ = wait(
                async {
                    sleep(HEARTBEAT).await;
                    Ok(())
                },
                keep,
            )
            .await;
        }
    }

    async fn drive(
        p: &Peripheral,
        keep: &impl Fn() -> bool,
        publish: &impl Fn(&str, &str),
    ) -> Result<()> {
        let id = Uuid::parse_str(&p.id().to_string())?;
        let target = id.to_string();
        wait(
            async {
                p.connect().await?;
                p.discover_services().await?;
                Ok(())
            },
            keep,
        )
        .await?;
        let characteristic = p
            .characteristics()
            .into_iter()
            .find(|c| {
                c.service_uuid == SERVICE
                    && c.uuid == STATUS
                    && c.properties.contains(CharPropFlags::WRITE)
            })
            .context("Saved device lacks the ClawLight writable status characteristic")?;
        let brightness_characteristic = p.characteristics().into_iter().find(|c| {
            c.service_uuid == SERVICE
                && c.uuid == BRIGHTNESS
                && c.properties.contains(CharPropFlags::WRITE)
        });
        // Set the saved brightness before any status/connection flash. This first
        // encrypted write may prompt for pairing; sample current settings again afterward.
        // Older firmware keeps working with the original one-byte status contract.
        let initial = [if brightness_characteristic.is_some() {
            config::read_config().lamp_brightness_percent(&target)
        } else {
            b'N'
        }];
        let first_characteristic = brightness_characteristic
            .as_ref()
            .unwrap_or(&characteristic);
        wait_for(async {
            let write = p.write(first_characteristic, &initial, WriteType::WithResponse);
            tokio::pin!(write);
            loop {
                tokio::select! {
                    result = &mut write => return Ok(result?),
                    _ = sleep(HEARTBEAT) => {
                        publish("connecting", "Waiting for an encrypted write; approve any macOS pairing request for this lamp");
                    }
                }
            }
        }, keep, Duration::from_secs(60)).await?;
        let last_brightness = Cell::new(brightness_characteristic.as_ref().map(|_| initial[0]));
        let last_preview = Cell::new(None::<(BrightnessPreview, StdInstant)>);
        let brightness_characteristic = brightness_characteristic.as_ref();
        let characteristic = &characteristic;
        let last_brightness = &last_brightness;
        let last_preview = &last_preview;
        let result = send_updates(
            |(byte, brightness, preview), _first| async move {
                wait_for(
                    async {
                        if let Some(setting) = brightness_characteristic {
                            if last_brightness.get() != Some(brightness) {
                                p.write(setting, &[brightness], WriteType::WithResponse)
                                    .await
                                    .context("Lamp brightness write failed")?;
                                last_brightness.set(Some(brightness));
                            }
                            // A heartbeat must not renew a preview, and a slow
                            // preceding write must not replay an expired event.
                            if let Some(preview) = pending_preview(
                                last_preview.get().map(|(preview, _)| preview),
                                preview,
                                StdInstant::now(),
                            ) {
                                p.write(setting, &preview.payload(), WriteType::WithResponse)
                                    .await
                                    .context("Lamp brightness preview failed")?;
                                last_preview.set(Some((preview, StdInstant::now())));
                            }
                        }
                        p.write(characteristic, &[byte], WriteType::WithResponse)
                            .await?;
                        Ok(())
                    },
                    keep,
                    Duration::from_secs(3),
                )
                .await?;
                let detail = if brightness_characteristic.is_some() {
                    if let Some((preview, _)) = last_preview
                        .get()
                        .filter(|(_, written)| written.elapsed() < PREVIEW_DURATION)
                    {
                        format!(
                            "Status acknowledged; brightness preview {}%",
                            preview.percent
                        )
                    } else {
                        format!("Status acknowledged; brightness {brightness}% applied")
                    }
                } else {
                    "Status acknowledged; brightness unavailable — update the C3 firmware".into()
                };
                publish("connected", &detail);
                Ok(())
            },
            || {
                let cfg = config::read_config();
                (
                    crate::led::status_byte(crate::state::aggregate(
                        &crate::state::read_hook_state(),
                        cfg.yellow_mode,
                    )),
                    cfg.lamp_brightness_percent(&target),
                    current_preview(id),
                )
            },
            keep,
        )
        .await;
        let _ = timeout(
            Duration::from_secs(1),
            p.write(characteristic, b"N", WriteType::WithResponse),
        )
        .await;
        result
    }

    async fn send_updates<T: Copy + PartialEq, F: Future<Output = Result<()>>>(
        mut write: impl FnMut(T, bool) -> F,
        read: impl Fn() -> T,
        keep: &impl Fn() -> bool,
    ) -> Result<()> {
        let mut last = None;
        let mut written = Instant::now();
        while keep() {
            // Register before reading so a concurrent slider event wakes every
            // lamp, including events arriving during an acknowledged write.
            let changed = PREVIEW_CHANGED.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let current = read();
            if due(last, current, written.elapsed()) {
                write(current, last.is_none()).await?;
                last = Some(current);
                written = Instant::now();
            }
            tokio::select! {
                _ = &mut changed => {},
                _ = sleep(POLL) => {},
            }
        }
        Ok(())
    }

    fn selected_advertisement(candidate: &str, services: &[Uuid], selected: &[String]) -> bool {
        selected.iter().any(|id| id == candidate) && services.contains(&SERVICE)
    }

    fn due<T: PartialEq>(last: Option<T>, current: T, elapsed: Duration) -> bool {
        last != Some(current) || elapsed >= HEARTBEAT
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn preview_expiry_and_write_deduplication() {
            let now = StdInstant::now();
            let preview = BrightnessPreview {
                percent: 25,
                requested_at: now,
            };
            assert_eq!(pending_preview(None, Some(preview), now), Some(preview));
            assert_eq!(preview.payload(), [25, 1]);
            assert_eq!(pending_preview(Some(preview), Some(preview), now), None);
            // A reconnect must not replay the last slider event after expiry.
            assert_eq!(
                pending_preview(None, Some(preview), now + PREVIEW_DURATION),
                None
            );
            let renewed = BrightnessPreview {
                requested_at: now + Duration::from_millis(500),
                ..preview
            };
            assert!(due(Some(Some(preview)), Some(renewed), Duration::ZERO));
            assert_eq!(
                pending_preview(Some(preview), Some(renewed), renewed.requested_at),
                Some(renewed)
            );
        }

        #[test]
        fn preview_is_ephemeral_and_wakes_every_lamp() {
            // HOME is isolated in a subprocess, never changed underneath other
            // tests or pointed at the user's real preferences.
            let Some(test_home) = std::env::var_os("CLAWLIGHT_PREVIEW_TEST_HOME") else {
                let home = tempfile::TempDir::new().unwrap();
                let result = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "ble::mac::tests::preview_is_ephemeral_and_wakes_every_lamp",
                    ])
                    .env("HOME", home.path())
                    .env("CLAWLIGHT_PREVIEW_TEST_HOME", home.path())
                    .output()
                    .unwrap();
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stdout)
                );
                return;
            };
            let path = config::config_file_path();
            let ids = [Uuid::from_u128(0xaa), Uuid::from_u128(0xbb)];
            let names = ids.map(|id| id.to_string());
            assert!(path.starts_with(test_home));
            preview_brightness(0);
            assert!(!path.exists(), "preview must not create configuration");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let original = br#"{"led_enabled":false,"led_brightness":100,"unknown":"preserve"}"#;
            std::fs::write(&path, original).unwrap();
            preview_brightness(25);
            let valid = current_preview(ids[0]);
            preview_brightness(101);
            assert_eq!(
                current_preview(ids[0]),
                valid,
                "invalid input must be ignored"
            );
            assert_eq!(std::fs::read(&path).unwrap(), original);

            let mut cfg: Config = serde_json::from_slice(original).unwrap();
            cfg.led_enabled = true;
            cfg.led_transport = LedTransport::Ble;
            cfg.ble_peripherals = names.to_vec();
            cfg.led_brightness = 0;
            config::write_config(&cfg).unwrap();
            let configured = std::fs::read(&path).unwrap();
            let connected = || {
                names
                    .iter()
                    .map(|id| crate::led::DeviceStatus::new(id, "connected", "Write acknowledged"))
                    .collect::<Vec<_>>()
            };
            crate::led::publish_ble_status(&cfg, connected());
            let rejects = |id: &str, percent| {
                let before = std::fs::read(&path).unwrap();
                assert!(preview_lamp_brightness(id, percent).is_err());
                assert_eq!(std::fs::read(&path).unwrap(), before);
            };
            rejects("invalid", 100);
            rejects(&Uuid::from_u128(3).to_string(), 100);
            rejects(&names[0], 101);
            preview_lamp_brightness(&names[0].to_uppercase(), 0).unwrap();
            preview_lamp_brightness(&names[1], 100).unwrap();
            assert_eq!(current_preview(ids[0]).unwrap().payload(), [0, 1]);
            assert_eq!(current_preview(ids[1]).unwrap().payload(), [100, 1]);
            let previous = current_preview(ids[0]);
            preview_lamp_brightness(&names[0], 0).unwrap();
            assert!(
                pending_preview(previous, current_preview(ids[0]), StdInstant::now()).is_some()
            );
            preview_brightness(50);
            assert!(ids
                .iter()
                .all(|id| current_preview(*id).unwrap().percent == 50));
            preview_lamp_brightness(&names[0], 75).unwrap();
            assert_eq!(current_preview(ids[0]).unwrap().percent, 75);
            assert_eq!(current_preview(ids[1]).unwrap().percent, 50);
            assert_eq!(std::fs::read(&path).unwrap(), configured);

            let mut devices = connected();
            devices[1].state = "searching".into();
            crate::led::publish_ble_status(&cfg, devices);
            rejects(&names[1], 100);
            let mut devices = connected();
            devices[1].updated -= 7;
            crate::led::publish_ble_status(&cfg, devices);
            rejects(&names[1], 100); // A healthy peer cannot make a stale lamp eligible.
            cfg.led_enabled = false;
            config::write_config(&cfg).unwrap();
            rejects(&names[0], 100);
            cfg.led_enabled = true;
            cfg.led_transport = LedTransport::Usb;
            config::write_config(&cfg).unwrap();
            rejects(&names[0], 100);
            std::fs::write(&path, "{corrupt").unwrap();
            rejects(&names[0], 100);
            std::fs::write(&path, &configured).unwrap();
            cfg.led_transport = LedTransport::Ble;
            crate::led::publish_ble_status(&cfg, connected());
            PREVIEW.lock().unwrap().clear();

            runtime().unwrap().block_on(async {
                let active = Cell::new(true);
                let keep = || active.get();
                let events = RefCell::new(Vec::new());
                let (sent, mut received) = tokio::sync::mpsc::unbounded_channel();
                let mut tasks = (0..2)
                    .map(|lamp| {
                        let sent = &sent;
                        let events = &events;
                        let keep = &keep;
                        async move {
                            let last_preview = Cell::new(None);
                            send_updates(
                                |(status, brightness, preview), _| {
                                    assert_eq!((status, brightness), (b'R', 100));
                                    let next = pending_preview(
                                        last_preview.get(),
                                        preview,
                                        StdInstant::now(),
                                    );
                                    if next.is_some() {
                                        last_preview.set(next);
                                    }
                                    let payload = next.map(BrightnessPreview::payload);
                                    events.borrow_mut().push((lamp, payload));
                                    sent.send((lamp, payload)).unwrap();
                                    async { Ok(()) }
                                },
                                || (b'R', 100, current_preview(ids[lamp])),
                                keep,
                            )
                            .await
                            .unwrap();
                        }
                        .boxed_local()
                    })
                    .collect::<Vec<_>>();
                let control = async {
                    for _ in 0..2 {
                        assert!(received.recv().await.unwrap().1.is_none());
                    }
                    for _ in 0..2 {
                        preview_brightness(25);
                        let mut lamps = Vec::new();
                        for _ in 0..2 {
                            let (lamp, payload) = timeout(POLL / 2, received.recv())
                                .await
                                .expect("Notify must wake both lamps before the polling fallback")
                                .unwrap();
                            assert_eq!(payload, Some([25, 1]));
                            lamps.push(lamp);
                        }
                        lamps.sort();
                        assert_eq!(lamps, [0, 1]);
                    }
                    // Back-to-back targeted requests must both survive until their
                    // workers run, rather than overwriting one shared event.
                    preview_lamp_brightness(&names[0], 0).unwrap();
                    preview_lamp_brightness(&names[1], 100).unwrap();
                    let mut lamps = Vec::new();
                    for _ in 0..2 {
                        let (lamp, payload) =
                            timeout(POLL / 2, received.recv()).await.unwrap().unwrap();
                        assert_eq!(payload, Some([if lamp == 0 { 0 } else { 100 }, 1]));
                        lamps.push(lamp);
                    }
                    lamps.sort();
                    assert_eq!(lamps, [0, 1]);
                    sleep(PREVIEW_DURATION + POLL).await;
                    assert!(ids.iter().all(|id| current_preview(*id).is_none()));
                    active.set(false);
                    PREVIEW_CHANGED.notify_waiters();
                };
                tasks.push(control.boxed_local());
                timeout(Duration::from_secs(4), run_all(tasks))
                    .await
                    .unwrap();
                for lamp in 0..2 {
                    let events = events.borrow();
                    assert_eq!(
                        events
                            .iter()
                            .filter(|(id, payload)| *id == lamp && payload.is_some())
                            .count(),
                        3
                    );
                    assert!(
                        events
                            .iter()
                            .filter(|(id, payload)| *id == lamp && payload.is_none())
                            .count()
                            >= 2
                    );
                }
            });
            assert_eq!(std::fs::read(path).unwrap(), configured);
        }

        #[test]
        fn usb_pair_protocol_candidates_and_advertisement_identity() {
            let identity = [0x14, 0x63, 0x93, 0x92, 0x45, 0x24];
            let reply = format!("clawlight_pair identity=146393924524 service={SERVICE}");
            assert_eq!(
                parse_pair_identity(&format!("{reply}\r\n")).unwrap(),
                identity
            );
            for invalid in [
                reply.replace("146393924524", "14639392452"),
                reply.replace("146393924524", "14AA93924524"),
                reply.replace(&SERVICE.to_string(), &STATUS.to_string()),
                format!("{reply} extra"),
            ] {
                assert!(parse_pair_identity(&invalid).is_err());
            }
            let port = |name: &str, pid| serialport::SerialPortInfo {
                port_name: name.into(),
                port_type: serialport::SerialPortType::UsbPort(serialport::UsbPortInfo {
                    vid: 0x303a,
                    pid,
                    serial_number: None,
                    manufacturer: None,
                    product: None,
                }),
            };
            assert!(pair_port(None, &[]).is_err());
            assert!(pair_port(None, &[port("/dev/cu.one", 2)]).is_err());
            assert_eq!(
                pair_port(
                    None,
                    &[port("/dev/tty.one", 0x1001), port("/dev/cu.one", 0x1001)]
                )
                .unwrap(),
                "/dev/cu.one"
            );
            assert!(pair_port(
                None,
                &[port("/dev/cu.one", 0x1001), port("/dev/cu.two", 0x1001)]
            )
            .is_err());
            assert_eq!(
                pair_port(Some("/verified/path".into()), &[]).unwrap(),
                "/verified/path"
            );
            let mut props = btleplug::api::PeripheralProperties::default();
            props.services.push(SERVICE);
            props.manufacturer_data.insert(0xffff, identity.to_vec());
            assert!(identity_matches(&props, &identity));
            assert!(!identity_matches(&props, &[0, 0, 0, 0, 0, 0]));
            props.manufacturer_data.get_mut(&0xffff).unwrap().push(0);
            assert!(!identity_matches(&props, &identity));
            props.manufacturer_data.insert(0xffff, identity.to_vec());
            props.services.clear();
            assert!(!identity_matches(&props, &identity));
        }

        #[test]
        fn missing_lamp_does_not_block_another_lamps_heartbeat_or_shutdown() {
            runtime().unwrap().block_on(async {
                let active = std::cell::Cell::new(true);
                let cleaned = std::cell::Cell::new(0);
                let writes = RefCell::new(Vec::new());
                let started = Instant::now();
                let keep = || active.get();
                let missing = async {
                    assert!(wait(std::future::pending::<Result<()>>(), &keep)
                        .await
                        .is_err());
                    cleaned.set(cleaned.get() + 1);
                };
                let nearby = async {
                    send_updates(
                        |byte, first| {
                            writes.borrow_mut().push((byte, first, started.elapsed()));
                            if writes.borrow().len() == 2 {
                                active.set(false);
                            }
                            async { Ok(()) }
                        },
                        || b'G',
                        &keep,
                    )
                    .await
                    .unwrap();
                    cleaned.set(cleaned.get() + 1);
                };
                timeout(
                    Duration::from_secs(4),
                    run_all(vec![missing.boxed_local(), nearby.boxed_local()]),
                )
                .await
                .expect("both lamp tasks stop promptly");
                assert_eq!(cleaned.get(), 2);
                let writes = writes.borrow();
                assert_eq!(writes.len(), 2);
                assert_eq!((writes[0].0, writes[0].1), (b'G', true));
                assert!(writes[0].2 < HEARTBEAT);
                assert_eq!((writes[1].0, writes[1].1), (b'G', false));
                assert!(writes[1].2 >= HEARTBEAT);
            });
        }

        #[test]
        fn brightness_changes_send_without_color_change_and_resend_after_reconnect() {
            runtime().unwrap().block_on(async {
                let active = Cell::new(true);
                let first = "11111111-2222-3333-4444-555555555555";
                let second = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
                let original = Config {
                    led_enabled: true,
                    led_transport: LedTransport::Ble,
                    ble_peripherals: vec![first.into(), second.into()],
                    ..Config::default()
                };
                let cfg = RefCell::new(original.clone());
                let writes = RefCell::new(Vec::new());
                let started = Instant::now();
                let keep = || active.get();
                send_updates(
                    |value, is_first| {
                        writes.borrow_mut().push((value, is_first));
                        if writes.borrow().len() == 1 {
                            cfg.borrow_mut().lamp_brightness.insert(first.into(), 25);
                        } else {
                            active.set(false);
                        }
                        async { Ok(()) }
                    },
                    || (b'G', cfg.borrow().lamp_brightness_percent(first)),
                    &keep,
                )
                .await
                .unwrap();
                assert_eq!(
                    *writes.borrow(),
                    vec![((b'G', 100), true), ((b'G', 25), false)]
                );
                assert_eq!(cfg.borrow().lamp_brightness_percent(second), 100);
                assert!(crate::led::same_target(&original, &cfg.borrow()));
                assert!(started.elapsed() < HEARTBEAT);
                // Reconnection samples current per-lamp settings, including an offline edit.
                cfg.borrow_mut().lamp_brightness.insert(first.into(), 50);
                active.set(true);
                send_updates(
                    |value, first| {
                        writes.borrow_mut().push((value, first));
                        active.set(false);
                        async { Ok(()) }
                    },
                    || (b'G', cfg.borrow().lamp_brightness_percent(first)),
                    &keep,
                )
                .await
                .unwrap();
                assert_eq!(writes.borrow().last(), Some(&((b'G', 50), true)));
                assert!(!due(Some((b'G', 25)), (b'G', 25), Duration::ZERO));
                assert!(due(Some((b'G', 25)), (b'G', 0), Duration::ZERO));
            });
        }

        #[test]
        fn heartbeat_and_cancellation_without_radio() {
            let selected = vec!["home".into(), "work".into()];
            assert!(!selected_advertisement("other", &[SERVICE], &selected));
            assert!(!selected_advertisement("home", &[STATUS], &selected));
            assert!(selected_advertisement("home", &[SERVICE], &selected));
            assert!(selected_advertisement("work", &[SERVICE], &selected));
            assert!(due(None, b'N', Duration::ZERO));
            assert!(due(Some(b'R'), b'G', Duration::ZERO));
            assert!(!due(Some(b'G'), b'G', Duration::from_secs(1)));
            assert!(due(Some(b'G'), b'G', HEARTBEAT));
            runtime().unwrap().block_on(async {
                let polls = std::cell::Cell::new(0);
                let keep = || {
                    polls.set(polls.get() + 1);
                    polls.get() < 2
                };
                // Same cancellation gate wraps scanning, connect, and writes.
                let result = wait(std::future::pending::<Result<()>>(), &keep).await;
                assert!(result.unwrap_err().to_string().contains("settings changed"));
                let result = wait(async { Ok(42) }, &|| true).await.unwrap();
                assert_eq!(result, 42);
            });
        }
    }
}
