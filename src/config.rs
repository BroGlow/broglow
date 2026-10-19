//! Persistent user settings at `~/.claude/broglow/config.json`.
//!
//! Kept separate from the hook-written `state.json` so the daemon and the TUI
//! can share opt-in preferences: lamp transport, remembered lamps and
//! brightness, yellow mode, usage tracking and agent-name display.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// How an `Inactive` (idle) session affects the aggregate light when other
/// sessions are still `Active`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum YellowMode {
    /// Any idle session turns the light yellow, even while others are working.
    #[default]
    AnyInactive,
    /// Working sessions win: stay green while anything is active; yellow only
    /// when every live session is idle.
    ActiveWins,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum LedTransport {
    #[default]
    Usb,
    Ble,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Show the optional terminal-dashboard entry points in the tray app.
    pub show_dashboard_button: bool,
    /// Name each session's coding agent beside its icon in the tray popover;
    /// off leaves the icon alone. On by default.
    pub show_agent_names: bool,
    /// List sessions under their working directory (the popover's project
    /// headings, the TUI's sort order and the Linux menu's headings) instead
    /// of under their status. Off by default; set from the popover's
    /// Settings view, or by hand on Linux.
    pub group_by_project: bool,
    /// Whether the menu bar daemon should mirror session state to an ESP32 over
    /// serial. Off by default; macOS USB setup can enable a newly added lamp.
    pub led_enabled: bool,
    /// Optional explicit serial device path. When `None`, the daemon
    /// auto-detects a known ESP32 board by USB vendor ID on each scan, which
    /// survives the board being unplugged and replugged at a different path.
    pub led_port: Option<String>,
    pub led_transport: LedTransport,
    /// Remembered CoreBluetooth UUIDs, never guessed MAC addresses/names.
    pub ble_peripherals: Vec<String>,
    /// Add forgotten lamps on a detected USB replug. Startup alone never adds them.
    pub auto_add_forgotten_usb: bool,
    /// Verified USB base MAC → Bluetooth ID, including forgotten boards.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub usb_lamps: std::collections::BTreeMap<String, String>,
    /// Optional user labels, keyed by the saved Bluetooth identity.
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub lamp_names: std::collections::BTreeMap<String, String>,
    /// Saved brightness for BLE lamps, including when mirroring is disabled.
    #[serde(deserialize_with = "deserialize_led_brightness")]
    pub led_brightness: u8,
    /// Per-lamp BLE brightness; lamps without an override use led_brightness.
    #[serde(
        deserialize_with = "deserialize_lamp_brightness",
        skip_serializing_if = "std::collections::BTreeMap::is_empty"
    )]
    pub lamp_brightness: std::collections::BTreeMap<String, u8>,
    /// How idle sessions color the aggregate (tray icon / LED) — see
    /// [`YellowMode`]. Set from the tray popover's Settings view.
    pub yellow_mode: YellowMode,
    /// Whether the tray shows today's usage (the plan percentage or token
    /// readout and the popover's usage section). Off by default — strictly opt-in, like the
    /// LEDs: until a user turns it on in the popover's Settings view, the daemon
    /// never scans transcripts, reads Claude Code's credentials, or contacts the
    /// usage endpoint. Enabling it is what authorizes that work.
    pub usage_enabled: bool,
    /// Preserve settings introduced by newer versions or local integrations.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_dashboard_button: false,
            show_agent_names: true,
            group_by_project: false,
            led_enabled: false,
            led_port: None,
            led_transport: LedTransport::default(),
            ble_peripherals: Vec::new(),
            auto_add_forgotten_usb: true,
            usb_lamps: Default::default(),
            lamp_names: Default::default(),
            led_brightness: 100,
            lamp_brightness: Default::default(),
            yellow_mode: YellowMode::default(),
            usage_enabled: false,
            extra: Default::default(),
        }
    }
}

fn deserialize_led_brightness<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u8, D::Error> {
    // A hand-edited brightness must not make unrelated settings fall back to defaults.
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value.as_f64().unwrap_or(100.0).clamp(0.0, 100.0) as u8)
}

fn deserialize_lamp_brightness<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<std::collections::BTreeMap<String, u8>, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    // Invalid overrides inherit the global preference without resetting other settings.
    Ok(value
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(id, value)| {
            value
                .as_f64()
                .map(|percent| (id.clone(), percent.clamp(0.0, 100.0) as u8))
        })
        .collect())
}

impl Config {
    #[cfg(any(target_os = "macos", target_os = "windows", test))]
    pub fn led_brightness_percent(&self) -> u8 {
        self.led_brightness.min(100)
    }

    #[cfg(any(target_os = "macos", target_os = "windows", test))]
    pub fn lamp_brightness_percent(&self, id: &str) -> u8 {
        self.lamp_brightness
            .get(id)
            .copied()
            .unwrap_or_else(|| self.led_brightness_percent())
            .min(100)
    }

    /// Compare remembered targets independent of order, without duplicates.
    pub fn ble_ids(&self) -> Vec<String> {
        let mut ids = self.ble_peripherals.clone();
        ids.sort();
        ids.dedup();
        ids
    }
}

pub fn config_file_path() -> PathBuf {
    dirs::home_dir()
        .expect("Home directory must exist")
        .join(".claude")
        .join("broglow")
        .join("config.json")
}

/// Read the config, falling back to defaults if it's missing or unreadable.
pub fn read_config() -> Config {
    read_config_strict().unwrap_or_default()
}

/// Settings edits must not replace unreadable configuration with defaults.
pub fn read_config_strict() -> anyhow::Result<Config> {
    match std::fs::read(config_file_path()) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(e.into()),
    }
}

pub fn set_dashboard_button(enabled: bool) -> anyhow::Result<()> {
    update_config(|cfg| {
        cfg.show_dashboard_button = enabled;
        Ok(())
    })
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn set_agent_names(enabled: bool) -> anyhow::Result<()> {
    update_config(|cfg| {
        cfg.show_agent_names = enabled;
        Ok(())
    })
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn set_group_by_project(enabled: bool) -> anyhow::Result<()> {
    update_config(|cfg| {
        cfg.group_by_project = enabled;
        Ok(())
    })
}

/// Serialize settings edits across the tray, CLI and setup subprocess. The lock
/// releases when this file drops; never hold it across device I/O or prompts.
pub fn acquire_config_lock() -> anyhow::Result<std::fs::File> {
    let path = config_file_path().with_file_name(".config.lock");
    std::fs::create_dir_all(path.parent().expect("config path must have a parent"))?;
    let file = std::fs::File::options()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    file.lock()?;
    Ok(file)
}

/// Read the latest valid settings and commit one edit while holding the shared lock.
pub fn update_config(edit: impl FnOnce(&mut Config) -> anyhow::Result<()>) -> anyhow::Result<()> {
    let _lock = acquire_config_lock()?;
    let mut cfg = read_config_strict()?;
    edit(&mut cfg)?;
    write_config(&cfg)
}

/// Atomically write config (temp file + rename). Production callers must hold
/// acquire_config_lock() across their read and write, or use update_config().
pub fn write_config(cfg: &Config) -> anyhow::Result<()> {
    let path = config_file_path();
    let dir = path.parent().expect("config path must have a parent");
    std::fs::create_dir_all(dir)?;
    let tmp_path = dir.join(format!(".config.{}.tmp", std::process::id()));
    let serialized = serde_json::to_string_pretty(cfg)?;
    std::fs::write(&tmp_path, serialized)?;
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn forgotten_usb_preference_defaults_on_and_round_trips() {
        assert!(Config::default().auto_add_forgotten_usb);
        assert!(
            serde_json::from_str::<Config>("{}")
                .unwrap()
                .auto_add_forgotten_usb
        );
        let cfg: Config = serde_json::from_str(
            r#"{"auto_add_forgotten_usb":false,"usb_lamps":{"a":"lamp"},"custom":42}"#,
        )
        .unwrap();
        let saved = serde_json::to_value(&cfg).unwrap();
        assert_eq!(saved["auto_add_forgotten_usb"], false);
        assert_eq!(saved["usb_lamps"]["a"], "lamp");
        assert_eq!(saved["custom"], 42);
    }

    #[test]
    fn missing_options_leave_lamps_and_usage_opted_out() {
        for cfg in [
            Config::default(),
            serde_json::from_str("{}").unwrap(),
            serde_json::from_str(r#"{"led_port":null,"ble_peripherals":["home"]}"#).unwrap(),
        ] {
            assert!(
                !cfg.led_enabled,
                "remembering a lamp is not permission to enable it"
            );
            assert!(
                !cfg.usage_enabled,
                "usage scanning requires explicit opt-in"
            );
            assert_eq!(cfg.lamp_brightness_percent("home"), 100);
        }
        let lamps_only: Config = serde_json::from_str(r#"{"led_enabled":true}"#).unwrap();
        assert!(
            !lamps_only.usage_enabled,
            "lamp opt-in does not authorize usage scanning"
        );
    }

    #[test]
    fn edited_brightness_bounds_preserve_other_settings() {
        for (value, expected) in [(-300, 0), (0, 0), (37, 37), (100, 100), (900, 100)] {
            let cfg: Config = serde_json::from_value(serde_json::json!({
                "led_brightness": value, "led_enabled": true,
                "ble_peripherals": ["home", "office"], "custom_setting": {"keep": true}
            }))
            .unwrap();
            assert_eq!(cfg.led_brightness_percent(), expected);
            let saved = serde_json::to_value(&cfg).unwrap();
            assert_eq!(saved["led_brightness"], expected);
            assert_eq!(saved["custom_setting"]["keep"], true);
            assert!(cfg.led_enabled);
            assert_eq!(cfg.ble_ids(), ["home", "office"]);
        }
        let cfg: Config = serde_json::from_str(
            r#"{"led_brightness":"invalid","usage_enabled":true,"custom_setting":7}"#,
        )
        .unwrap();
        assert_eq!(cfg.led_brightness_percent(), 100);
        assert!(cfg.usage_enabled);
        assert_eq!(cfg.extra["custom_setting"], 7);
        assert_eq!(
            Config {
                led_brightness: 255,
                ..Config::default()
            }
            .led_brightness_percent(),
            100
        );
    }

    #[test]
    fn edited_lamp_brightness_is_bounded_and_invalid_overrides_inherit() {
        let cfg: Config = serde_json::from_value(serde_json::json!({
            "led_brightness": 37, "usage_enabled": true, "custom": 42,
            "lamp_brightness": {"dark": -5, "bright": 300, "half": 50, "bad": "invalid"}
        }))
        .unwrap();
        for (id, expected) in [
            ("dark", 0),
            ("bright", 100),
            ("half", 50),
            ("bad", 37),
            ("new", 37),
        ] {
            assert_eq!(cfg.lamp_brightness_percent(id), expected);
        }
        assert!(cfg.usage_enabled);
        assert_eq!(cfg.extra["custom"], 42);
        assert!(!cfg.led_enabled);
        for invalid in [
            serde_json::Value::Null,
            serde_json::json!([25]),
            serde_json::json!("invalid"),
        ] {
            let cfg: Config = serde_json::from_value(serde_json::json!({
                "led_brightness": 37, "lamp_brightness": invalid,
                "led_port": "keep", "custom": 42
            }))
            .unwrap();
            assert_eq!(cfg.lamp_brightness_percent("home"), 37);
            assert_eq!(cfg.led_port.as_deref(), Some("keep"));
            assert_eq!(cfg.extra["custom"], 42);
        }
    }
}
