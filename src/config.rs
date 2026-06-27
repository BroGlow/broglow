//! Persistent user settings at `~/.claude/clawlight/config.json`.
//!
//! Kept separate from the hook-written `state.json` so the daemon and the TUI
//! can share opt-in preferences (currently just the ESP32 status LEDs).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    /// Whether the menu bar daemon should mirror session state to an ESP32 over
    /// serial. Off by default — the daemon never touches a serial port until a
    /// user explicitly enables it (TUI: press `l`).
    pub led_enabled: bool,
    /// Optional explicit serial device path. When `None`, the daemon
    /// auto-detects a known ESP32 board by USB vendor ID on each scan, which
    /// survives the board being unplugged and replugged at a different path.
    pub led_port: Option<String>,
}

pub fn config_file_path() -> PathBuf {
    dirs::home_dir()
        .expect("Home directory must exist")
        .join(".claude")
        .join("clawlight")
        .join("config.json")
}

/// Read the config, falling back to defaults if it's missing or unreadable.
pub fn read_config() -> Config {
    let path = config_file_path();
    if !path.exists() {
        return Config::default();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

/// Atomically write the config (temp file + rename), creating the directory if
/// needed.
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
    fn roundtrips_through_json() {
        let cfg = Config {
            led_enabled: true,
            led_port: Some("/dev/cu.usbmodem101".to_string()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&json).unwrap();
        assert!(back.led_enabled);
        assert_eq!(back.led_port.as_deref(), Some("/dev/cu.usbmodem101"));
    }

    #[test]
    fn empty_or_missing_fields_default_to_led_off() {
        // A brand-new user (no config file) and a partial config must both
        // resolve to LED off — the opt-in safety guarantee.
        let from_empty: Config = serde_json::from_str("{}").unwrap();
        assert!(!from_empty.led_enabled);
        assert!(from_empty.led_port.is_none());

        let from_partial: Config = serde_json::from_str(r#"{"led_port": null}"#).unwrap();
        assert!(!from_partial.led_enabled);

        assert!(!Config::default().led_enabled);
    }
}
