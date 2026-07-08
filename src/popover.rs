//! Custom tray popover (designs 1a/1c from the Tray Menu Designs doc): a
//! frameless, always-on-top, transparent window anchored to the tray icon,
//! rendered by the system webview (WKWebView on macOS, WebView2 on Windows).
//!
//! Linux is excluded — libappindicator emits no tray click events, so there
//! is nothing to anchor to; the tray keeps its native menu there.

use std::time::{Duration, Instant};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Window, WindowBuilder, WindowId};
use tray_icon::Rect;
use wry::WebView;

use crate::config::{self, BillingMode, YellowMode};
use crate::menubar::{display_name, project_label, ICON_GREEN, ICON_NONE, ICON_RED, ICON_YELLOW};
use crate::state::{aggregate, Aggregate, HookState, Status};
use crate::usage::{self, UsageSnapshot};

/// Logical width of the popover window. Must match the card width in
/// assets/popover.html; the window is sized exactly to the card and the
/// shadow is the native window shadow.
#[cfg(target_os = "macos")]
const WIDTH: f64 = 338.0;
#[cfg(target_os = "windows")]
const WIDTH: f64 = 344.0;

/// Gap between the card edge and the tray icon / taskbar edge (logical px).
const ANCHOR_GAP: f64 = 6.0;

/// Clicking the tray icon while the popover is open first steals its focus
/// (auto-hiding it) and then delivers the click event; without this debounce
/// the same click would immediately reopen it, making toggle impossible.
const REOPEN_DEBOUNCE: Duration = Duration::from_millis(300);

/// macOS activation is cooperative: right after showing, the system can
/// briefly bounce key status back to the previously active app, delivering a
/// spurious Focused(false). Ignore focus loss this soon after showing.
const FOCUS_GRACE: Duration = Duration::from_millis(600);

/// Messages posted by the popover page over the webview IPC channel.
#[derive(Debug, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum PopoverMsg {
    /// The page finished a render and reports its content height (logical px).
    Resize { height: f64 },
    /// The Focus button on a needs-help session row.
    Focus { id: String },
    /// The Open Dashboard footer button.
    Dashboard,
    /// The Quit footer button.
    Quit,
    /// Escape pressed inside the popover.
    Hide,
    /// The page finished loading and can accept `__setState` calls.
    Ready,
    /// A yellow-light option picked in the Settings view.
    SetYellowMode { mode: YellowMode },
    /// A usage-tracking option picked in the Settings view: whether to track
    /// usage at all (`enabled`) and, when on, which readout (`mode`). The page
    /// sends both together so "Off" and the Plan/API choice are one control.
    SetUsage { enabled: bool, mode: BillingMode },
}

#[derive(Serialize)]
struct SessionPayload<'a> {
    id: &'a str,
    name: String,
    project: String,
    status: &'a Status,
    #[serde(rename = "lastUpdated")]
    last_updated: &'a str,
}

#[derive(Serialize)]
struct SettingsPayload {
    #[serde(rename = "yellowMode")]
    yellow_mode: YellowMode,
    /// Whether usage tracking is opted into; drives the Settings selection and
    /// whether the usage section shows at all.
    #[serde(rename = "usageEnabled")]
    usage_enabled: bool,
    #[serde(rename = "billingMode")]
    billing_mode: BillingMode,
}

#[derive(Serialize)]
struct StatePayload<'a> {
    aggregate: &'static str,
    sessions: Vec<SessionPayload<'a>>,
    settings: SettingsPayload,
    /// Usage section (design 1a/1c); absent until the first scan completes.
    usage: Option<UsageSnapshot>,
}

pub struct Popover {
    window: Window,
    webview: WebView,
    anchor: Option<Rect>,
    /// Show is deferred until the page reports its height for the new state,
    /// so the window never flashes at a stale size.
    pending_show: bool,
    hidden_at: Option<Instant>,
    shown_at: Option<Instant>,
    /// CLAWLIGHT_POPOVER_DEBUG: the popover was force-opened without user
    /// interaction, so macOS revokes our activation; don't treat the
    /// resulting focus loss as a dismissal.
    debug: bool,
}

impl Popover {
    pub fn new<T>(
        target: &EventLoopWindowTarget<T>,
        on_msg: impl Fn(PopoverMsg) + 'static,
    ) -> anyhow::Result<Self> {
        #[allow(unused_mut)]
        let mut builder = WindowBuilder::new()
            .with_title("clawlight")
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top(true)
            .with_visible(false)
            .with_transparent(true)
            .with_inner_size(LogicalSize::new(WIDTH, 240.0));
        #[cfg(target_os = "windows")]
        {
            use tao::platform::windows::WindowBuilderExtWindows;
            builder = builder.with_skip_taskbar(true);
        }
        let window = builder.build(target).context("Building popover window")?;

        let webview = wry::WebViewBuilder::new()
            .with_transparent(true)
            .with_html(popover_html())
            // macOS: the popover may be visible without key focus; the first
            // click on a button should act, not just focus the window.
            .with_accept_first_mouse(true)
            .with_ipc_handler(move |req| {
                if std::env::var_os("CLAWLIGHT_POPOVER_DEBUG").is_some() {
                    eprintln!("popover ipc: {}", req.body());
                }
                if let Ok(msg) = serde_json::from_str::<PopoverMsg>(req.body()) {
                    on_msg(msg);
                }
            })
            .build(&window)
            .context("Building popover webview")?;

        Ok(Self {
            window,
            webview,
            anchor: None,
            pending_show: false,
            hidden_at: None,
            shown_at: None,
            debug: std::env::var_os("CLAWLIGHT_POPOVER_DEBUG").is_some(),
        })
    }

    pub fn window_id(&self) -> WindowId {
        self.window.id()
    }

    pub fn is_visible(&self) -> bool {
        self.window.is_visible()
    }

    /// True when the popover was hidden so recently that the current tray
    /// click must be the one that dismissed it (see [`REOPEN_DEBOUNCE`]).
    pub fn just_dismissed(&self) -> bool {
        self.hidden_at
            .is_some_and(|t| t.elapsed() < REOPEN_DEBOUNCE)
    }

    /// Anchor to the tray icon and show once the page reports its height.
    pub fn open_at(&mut self, anchor: Rect, state: &HookState) {
        self.anchor = Some(anchor);
        self.pending_show = true;
        self.push_state(state);
    }

    pub fn hide(&mut self) {
        if self.window.is_visible() {
            self.window.set_visible(false);
            self.hidden_at = Some(Instant::now());
        }
        self.pending_show = false;
    }

    /// The window lost focus: dismiss, unless the loss is the activation
    /// bounce right after showing (see [`FOCUS_GRACE`]).
    pub fn on_focus_lost(&mut self) {
        if self.debug {
            return;
        }
        if self.shown_at.is_some_and(|t| t.elapsed() < FOCUS_GRACE) {
            return;
        }
        self.hide();
    }

    /// Re-render the page from fresh hook state. If the popover is open (or
    /// opening), the page answers with a Resize that re-applies geometry.
    pub fn push_state(&self, state: &HookState) {
        let payload = build_payload(state);
        let _ = self
            .webview
            .evaluate_script(&format!("window.__setState({payload})"));
    }

    /// The page reported its rendered height: size the window to fit, keep it
    /// glued to the anchor, and complete a deferred show.
    pub fn on_resize(&mut self, logical_height: f64) {
        let size = LogicalSize::new(WIDTH, logical_height.clamp(120.0, 720.0));
        self.window.set_inner_size(size);
        if let Some(anchor) = self.anchor {
            self.position_at(anchor, size.to_physical(self.window.scale_factor()));
        }
        if self.pending_show {
            self.pending_show = false;
            self.window.set_visible(true);
            self.window.set_focus();
            self.shown_at = Some(Instant::now());
        }
    }

    /// Position the window against the tray icon rect (physical px, top-left
    /// origin on every platform — tray-icon pre-flips macOS coordinates).
    /// macOS hangs the card below the menu bar icon; Windows floats it above
    /// the taskbar. Horizontally centered on the icon, clamped to the monitor.
    fn position_at(&self, anchor: Rect, size: PhysicalSize<u32>) {
        let scale = self.window.scale_factor();
        let w = size.width as f64;

        let icon_center = anchor.position.x + anchor.size.width as f64 / 2.0;
        let mut x = icon_center - w / 2.0;
        if let Some(monitor) = self
            .window
            .available_monitors()
            .find(|m| {
                let p = m.position();
                let s = m.size();
                icon_center >= p.x as f64 && icon_center < (p.x + s.width as i32) as f64
            })
            .or_else(|| self.window.primary_monitor())
        {
            // Keep a small margin so the card never touches the screen edge.
            let margin = 8.0 * scale;
            let min = monitor.position().x as f64 + margin;
            let max = min + monitor.size().width as f64 - w - 2.0 * margin;
            x = x.clamp(min, max.max(min));
        }

        #[cfg(target_os = "macos")]
        let y = {
            // ANCHOR_GAP below the bottom edge of the menu bar icon.
            let icon_bottom = anchor.position.y + anchor.size.height as f64;
            icon_bottom + ANCHOR_GAP * scale
        };
        #[cfg(target_os = "windows")]
        let y = {
            // ANCHOR_GAP above the top edge of the taskbar icon.
            anchor.position.y - size.height as f64 - ANCHOR_GAP * scale
        };

        self.window.set_outer_position(PhysicalPosition::new(x, y));
    }
}

fn agg_str(agg: Aggregate) -> &'static str {
    match agg {
        Aggregate::Red => "red",
        Aggregate::Yellow => "yellow",
        Aggregate::Green => "green",
        Aggregate::None => "none",
    }
}

/// Serialize hook state for `window.__setState`. Grouping, sorting, and
/// relative-time formatting happen in the page.
fn build_payload(state: &HookState) -> String {
    let cfg = config::read_config();
    let sessions: Vec<SessionPayload> = state
        .sessions
        .iter()
        .filter(|(_, s)| s.status != Status::Done)
        .map(|(id, s)| SessionPayload {
            id,
            name: display_name(id, s),
            project: project_label(s),
            status: &s.status,
            last_updated: &s.last_updated,
        })
        .collect();
    let payload = StatePayload {
        aggregate: agg_str(aggregate(state, cfg.yellow_mode)),
        sessions,
        settings: SettingsPayload {
            yellow_mode: cfg.yellow_mode,
            usage_enabled: cfg.usage_enabled,
            billing_mode: cfg.billing_mode,
        },
        // Only surface usage once the user has opted in — otherwise the section
        // stays absent even if a stale snapshot is still cached.
        usage: cfg.usage_enabled.then(usage::latest).flatten(),
    };
    serde_json::to_string(&payload)
        .unwrap_or_else(|_| r#"{"aggregate":"none","sessions":[]}"#.to_string())
}

fn popover_html() -> String {
    const HTML: &str = include_str!("../assets/popover.html");
    let platform = if cfg!(target_os = "macos") {
        "mac"
    } else {
        "win"
    };
    HTML.replace("__PLATFORM__", platform)
        .replace("__ICON_RED__", &data_uri(ICON_RED))
        .replace("__ICON_YELLOW__", &data_uri(ICON_YELLOW))
        .replace("__ICON_GREEN__", &data_uri(ICON_GREEN))
        .replace("__ICON_NONE__", &data_uri(ICON_NONE))
}

fn data_uri(png: &[u8]) -> String {
    format!("data:image/png;base64,{}", base64(png))
}

/// Minimal standard-alphabet base64 (with padding) — used only to inline the
/// four embedded status PNGs into the popover page; not worth a dependency.
fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from_be_bytes([0, b[0], b[1], b[2]]);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{base64, PopoverMsg};
    use crate::config::{BillingMode, YellowMode};

    /// Pins the IPC wire format the popover page's JS emits for the Settings
    /// view (`{cmd:'set_yellow_mode', mode:'...'}` in assets/popover.html).
    #[test]
    fn set_yellow_mode_parses_from_page_json() {
        let msg: PopoverMsg =
            serde_json::from_str(r#"{"cmd":"set_yellow_mode","mode":"active_wins"}"#).unwrap();
        assert!(matches!(
            msg,
            PopoverMsg::SetYellowMode {
                mode: YellowMode::ActiveWins
            }
        ));
        let msg: PopoverMsg =
            serde_json::from_str(r#"{"cmd":"set_yellow_mode","mode":"any_inactive"}"#).unwrap();
        assert!(matches!(
            msg,
            PopoverMsg::SetYellowMode {
                mode: YellowMode::AnyInactive
            }
        ));
    }

    /// Pins the IPC wire format for the usage-tracking setting
    /// (`{cmd:'set_usage', enabled:..., mode:'...'}` in assets/popover.html):
    /// the "Off" row disables tracking; the Plan/API rows enable it and pick the
    /// readout.
    #[test]
    fn set_usage_parses_from_page_json() {
        let msg: PopoverMsg =
            serde_json::from_str(r#"{"cmd":"set_usage","enabled":true,"mode":"api"}"#).unwrap();
        assert!(matches!(
            msg,
            PopoverMsg::SetUsage {
                enabled: true,
                mode: BillingMode::Api
            }
        ));
        let msg: PopoverMsg =
            serde_json::from_str(r#"{"cmd":"set_usage","enabled":false,"mode":"plan"}"#).unwrap();
        assert!(matches!(
            msg,
            PopoverMsg::SetUsage {
                enabled: false,
                mode: BillingMode::Plan
            }
        ));
    }

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
