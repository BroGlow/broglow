//! Persistent user settings at `~/.claude/broglow/config.json`.
//!
//! Kept separate from the hook-written `state.json` so the daemon and the TUI
//! can share opt-in preferences (currently just the ESP32 status LEDs).

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

/// Which usage readout the tray shows (design 1a/1c "billing" tweak).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BillingMode {
    /// Subscription view: 5-hour block and weekly rate-limit percentages.
    #[default]
    Plan,
    /// API view: today's per-model tokens and list-price dollars.
    Api,
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
    /// Whether the menu bar daemon should mirror session state to an ESP32 over
    /// serial. Off by default — the daemon never touches a serial port until a
    /// user explicitly enables it (TUI: press `l`).
    pub led_enabled: bool,
    /// Optional explicit serial device path. When `None`, the daemon
    /// auto-detects a known ESP32 board by USB vendor ID on each scan, which
    /// survives the board being unplugged and replugged at a different path.
    pub led_port: Option<String>,
    pub led_transport: LedTransport,
    /// Remembered CoreBluetooth UUIDs, never guessed MAC addresses/names.
    pub ble_peripherals: Vec<String>,
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
    /// Whether the tray shows today's Claude Code usage (the token/$ readout and
    /// the popover's usage section). Off by default — strictly opt-in, like the
    /// LEDs: until a user turns it on in the popover's Settings view, the daemon
    /// never scans transcripts, reads Claude Code's credentials, or contacts the
    /// usage endpoint. Enabling it is what authorizes that work.
    pub usage_enabled: bool,
    /// Plan-percentage vs API-dollar usage readout — see [`BillingMode`]. Only
    /// consulted when [`usage_enabled`](Self::usage_enabled) is set. Set from
    /// the tray popover's Settings view.
    pub billing_mode: BillingMode,
    /// Preserve settings introduced by newer versions or local integrations.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            show_dashboard_button: false,
            led_enabled: false,
            led_port: None,
            led_transport: LedTransport::default(),
            ble_peripherals: Vec::new(),
            lamp_names: Default::default(),
            led_brightness: 100,
            lamp_brightness: Default::default(),
            yellow_mode: YellowMode::default(),
            usage_enabled: false,
            billing_mode: BillingMode::default(),
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
    use super::{BillingMode, Config, YellowMode};

    #[test]
    fn preserves_unrelated_settings_with_ble_selection() {
        let mut cfg: Config = serde_json::from_str(
            r#"{"custom_setting":{"keep":true},"led_port":"existing","yellow_mode":"active_wins"}"#,
        )
        .unwrap();
        cfg.ble_peripherals = vec!["selected-id".into()];
        cfg.led_transport = super::LedTransport::Ble;
        let json = serde_json::to_value(&cfg).unwrap();
        assert_eq!(json["custom_setting"]["keep"], true);
        let back: Config = serde_json::from_value(json).unwrap();
        assert_eq!(back.ble_peripherals, ["selected-id"]);
        assert_eq!(back.led_port.as_deref(), Some("existing"));
        assert_eq!(back.yellow_mode, YellowMode::ActiveWins);
        assert_eq!(back.ble_ids(), ["selected-id"]);
        let mut multiple = back;
        multiple.ble_peripherals = vec!["work".into(), "selected-id".into(), "home".into()];
        assert_eq!(multiple.ble_ids(), ["home", "selected-id", "work"]);
    }

    #[test]
    fn roundtrips_through_json() {
        let cfg = Config {
            led_enabled: true,
            led_port: Some("/dev/cu.usbmodem101".to_string()),
            yellow_mode: YellowMode::ActiveWins,
            usage_enabled: true,
            billing_mode: BillingMode::Api,
            led_brightness: 37,
            lamp_brightness: [("home".into(), 0), ("office".into(), 80)].into(),
            ..Config::default()
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert!(back.led_enabled);
        assert_eq!(back.led_port.as_deref(), Some("/dev/cu.usbmodem101"));
        assert_eq!(back.yellow_mode, YellowMode::ActiveWins);
        assert!(back.usage_enabled);
        assert_eq!(back.billing_mode, BillingMode::Api);
        assert_eq!(back.led_brightness_percent(), 37);
        assert_eq!(back.lamp_brightness_percent("home"), 0);
        assert_eq!(back.lamp_brightness_percent("office"), 80);
        assert_eq!(back.lamp_brightness_percent("new"), 37);
    }

    #[test]
    fn brightness_defaults_to_full_without_enabling_lamps() {
        for cfg in [
            Config::default(),
            serde_json::from_str("{}").unwrap(),
            serde_json::from_str(r#"{"led_enabled":false,"ble_peripherals":["home"]}"#).unwrap(),
        ] {
            assert_eq!(cfg.led_brightness_percent(), 100);
            assert_eq!(cfg.lamp_brightness_percent("home"), 100);
            assert!(serde_json::to_value(&cfg)
                .unwrap()
                .get("lamp_brightness")
                .is_none());
            assert!(!cfg.led_enabled);
        }
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

    #[test]
    fn usage_defaults_to_off() {
        // Usage tracking is strictly opt-in: a brand-new user (no config file),
        // a partial config, and a config written before the setting existed
        // must all resolve to usage off, so the daemon never scans transcripts
        // or reads credentials until the user turns it on.
        assert!(!Config::default().usage_enabled);
        let from_empty: Config = serde_json::from_str("{}").unwrap();
        assert!(!from_empty.usage_enabled);
        let old: Config = serde_json::from_str(r#"{"led_enabled": true}"#).unwrap();
        assert!(!old.usage_enabled);
        assert_eq!(old.led_transport, super::LedTransport::Usb);
        assert!(old.ble_peripherals.is_empty());
    }

    #[test]
    fn billing_mode_defaults_to_plan() {
        // Configs written before the setting existed default to the plan view.
        let old: Config = serde_json::from_str(r#"{"led_enabled": true}"#).unwrap();
        assert_eq!(old.billing_mode, BillingMode::Plan);
    }

    #[test]
    fn yellow_mode_defaults_to_any_inactive() {
        // Configs written before the setting existed must keep the original
        // behavior: any idle session shows yellow.
        let old: Config = serde_json::from_str(r#"{"led_enabled": true}"#).unwrap();
        assert_eq!(old.yellow_mode, YellowMode::AnyInactive);
        assert_eq!(Config::default().yellow_mode, YellowMode::AnyInactive);
    }

    #[test]
    fn empty_or_missing_fields_default_to_led_off() {
        // A brand-new user (no config file) and a partial config must both
        // resolve to LED off — the opt-in safety guarantee.
        let from_empty: Config = serde_json::from_str("{}").unwrap();
        assert!(!from_empty.led_enabled);
        assert!(!from_empty.show_dashboard_button);
        assert!(from_empty.led_port.is_none());

        let from_partial: Config = serde_json::from_str(r#"{"led_port": null}"#).unwrap();
        assert!(!from_partial.led_enabled);
        assert!(!from_partial.show_dashboard_button);

        assert!(!Config::default().led_enabled);
        assert!(!Config::default().show_dashboard_button);
    }
}
