//! Custom tray popover (designs 1a/1c from the Tray Menu Designs doc): a
//! frameless, always-on-top, transparent window anchored to the tray icon,
//! rendered by the system webview (WKWebView on macOS, WebView2 on Windows).
//!
//! Linux is excluded — libappindicator emits no tray click events, so there
//! is nothing to anchor to; the tray keeps its native menu there.

use std::time::{Duration, Instant, SystemTime};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tao::dpi::{LogicalSize, PhysicalPosition, PhysicalSize};
use tao::event_loop::EventLoopWindowTarget;
use tao::window::{Window, WindowBuilder, WindowId};
use tray_icon::Rect;
use wry::WebView;

use crate::config::{self, LedTransport, YellowMode};
use crate::menubar::{display_name, ICON_GREEN, ICON_NONE, ICON_RED, ICON_YELLOW};
use crate::state::{aggregate, Aggregate, HookState, SessionStatus, Status};
use crate::usage::{self, UsageSnapshot};

/// Logical width of the popover window. Must match the card width in
/// assets/popover/popover.css; the window is sized exactly to the card and the
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
    /// The page finished a render and reports its content height (logical px)
    /// plus the devicePixelRatio WebView2 is actually rendering at — on
    /// Windows the window is sized from that ratio, since it's the only scale
    /// value guaranteed to match the rendered surface (tao's and Win32's
    /// opinions of the window's DPI both diverge from it in practice).
    Resize {
        height: f64,
        #[serde(default)]
        dpr: Option<f64>,
    },
    /// The Focus button on a needs-help session row.
    Focus {
        id: String,
    },
    /// Dismiss a session from BroGlow until its next hook event.
    ClearSession {
        id: String,
    },
    /// Dismiss only sessions that are still paused when the command is handled.
    ClearPaused,
    /// The Dashboard footer button.
    Dashboard,
    /// The Quit footer button.
    Quit,
    /// Escape pressed inside the popover.
    Hide,
    /// The page finished loading and can accept `__setState` calls.
    Ready,
    /// A yellow-light option picked in the Settings view.
    SetYellowMode {
        mode: YellowMode,
    },
    /// Show the terminal dashboard shortcuts, using the same preference as the CLI.
    SetDashboardButton {
        enabled: bool,
    },
    /// Name each session's agent beside its icon, or show the icon alone.
    SetAgentNames {
        enabled: bool,
    },
    /// List sessions under their working directory instead of their status.
    SetGroupByProject {
        enabled: bool,
    },
    SetPopoverReset {
        enabled: bool,
    },
    SetPopoverResetMinutes {
        #[serde(deserialize_with = "deserialize_popover_reset_minutes")]
        minutes: u16,
    },
    /// Add forgotten lamps when their USB cable is reconnected (macOS only).
    SetAutoAddForgottenUsb {
        enabled: bool,
    },
    /// The Settings view's usage-tracking toggle.
    SetUsage {
        enabled: bool,
    },
    /// Show usage in the menu bar without changing background tracking.
    SetUsageInMenuBar {
        enabled: bool,
    },
    /// The Lamp details control: connect (`enabled: true`) or disconnect the
    /// physical status lamp by toggling LED mirroring.
    SetLed {
        enabled: bool,
    },
    /// Identify the single USB lamp and remember its Bluetooth identity.
    AddLamp,
    /// Write the bundled firmware to the single USB lamp.
    UpdateFirmware,
    /// Stop remembering/reconnecting to one lamp; retain Bluetooth bonds.
    ForgetLamp {
        id: String,
    },
    RenameLamp {
        id: String,
        name: String,
    },
    IdentifyLamp {
        id: String,
    },
    /// A saved BLE brightness preference, separate from lamp enablement.
    SetBrightness {
        #[serde(deserialize_with = "deserialize_brightness_percent")]
        percent: u8,
    },
    SetLampBrightness {
        id: String,
        #[serde(deserialize_with = "deserialize_brightness_percent")]
        percent: u8,
    },
    /// Temporary all-LED feedback while adjusting the brightness slider.
    PreviewBrightness {
        id: Option<String>,
        #[serde(deserialize_with = "deserialize_brightness_percent")]
        percent: u8,
    },
}

fn deserialize_popover_reset_minutes<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u16, D::Error> {
    let minutes = u16::deserialize(deserializer)?;
    if !(1..=1440).contains(&minutes) {
        return Err(serde::de::Error::custom(
            "Minutes must be between 1 and 1440",
        ));
    }
    Ok(minutes)
}

fn deserialize_brightness_percent<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<u8, D::Error> {
    let percent = u8::deserialize(deserializer)?;
    if percent > 100 {
        return Err(serde::de::Error::custom(
            "Brightness must be between 0 and 100",
        ));
    }
    Ok(percent)
}

#[derive(Serialize)]
struct SessionPayload<'a> {
    id: &'a str,
    name: String,
    /// The shared project label (`session::project_labels`), computed over
    /// the live sessions; "No project" when the directory is unknown.
    project: String,
    /// The working directory as recorded, the grouping key; `""` when
    /// unknown (the page relies on the empty sentinel for "No project").
    #[serde(rename = "projectPath")]
    project_path: &'a str,
    status: &'a Status,
    #[serde(rename = "capacityBlocked")]
    capacity_blocked: bool,
    #[serde(rename = "lastUpdated")]
    last_updated: &'a str,
    /// The session's coding agent, for the icon and name in the meta line.
    agent: AgentPayload,
    /// Where the session runs, for the host glyph; absent when unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    host: Option<HostPayload>,
}

#[derive(Serialize)]
struct HostPayload {
    /// `"app"`, `"terminal"` or `"ide"`: picks the page's glyph.
    kind: &'static str,
    /// Hover text and accessible label: "Claude app", "Ghostty", "VS Code".
    name: String,
}

impl From<crate::terminal::Host> for HostPayload {
    fn from(h: crate::terminal::Host) -> Self {
        Self {
            kind: h.kind.as_str(),
            name: h.name,
        }
    }
}

#[derive(Serialize)]
struct AgentPayload {
    label: String,
    /// SVG path data from `harness`; absent for an unknown harness, whose
    /// `badge` text (the TUI's tag) stands in for the icon.
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<&'static str>,
    badge: String,
}

impl From<crate::harness::Agent> for AgentPayload {
    fn from(a: crate::harness::Agent) -> Self {
        Self {
            label: a.label,
            icon: a.icon,
            badge: a.badge,
        }
    }
}

#[derive(Serialize)]
struct SettingsPayload {
    #[serde(rename = "showDashboardButton")]
    show_dashboard_button: bool,
    #[serde(rename = "showAgentNames")]
    show_agent_names: bool,
    #[serde(rename = "groupByProject")]
    group_by_project: bool,
    #[serde(rename = "popoverResetEnabled")]
    popover_reset_enabled: bool,
    #[serde(rename = "popoverResetMinutes")]
    popover_reset_minutes: u16,
    #[serde(rename = "autoAddForgottenUsb")]
    auto_add_forgotten_usb: bool,
    #[serde(rename = "yellowMode")]
    yellow_mode: YellowMode,
    /// Whether usage tracking is opted into; drives the Settings selection and
    /// whether the usage section shows at all.
    #[serde(rename = "usageEnabled")]
    usage_enabled: bool,
    #[serde(rename = "showUsageInMenuBar")]
    show_usage_in_menu_bar: bool,
}

/// Physical status-lamp state for the footer connection indicator.
#[derive(Serialize)]
struct LampPayload {
    names: std::collections::BTreeMap<String, String>,
    bluetooth_supported: bool,
    setup_busy: bool,
    /// A broglow board is attached over USB right now.
    present: bool,
    /// LED mirroring is turned on in config (the daemon drives the board).
    enabled: bool,
    transport: LedTransport,
    selected_count: usize,
    state: String,
    detail: String,
    devices: Vec<crate::led::DeviceStatus>,
    brightness_percent: u8,
    brightness_by_id: std::collections::BTreeMap<String, u8>,
    /// An ESP32 USB device is attached, whatever the selected transport.
    usb_present: bool,
    firmware: FirmwarePayload,
}

/// The Lamp details firmware section: what this release can write, and
/// whether a write is running right now.
#[derive(Serialize)]
struct FirmwarePayload {
    /// Version of the images installed with this app; absent without images.
    available: Option<String>,
    busy: bool,
    /// Bytes written so far as a percentage, while `busy`.
    progress: u8,
}

#[derive(Serialize)]
struct StatePayload<'a> {
    aggregate: &'static str,
    sessions: Vec<SessionPayload<'a>>,
    settings: SettingsPayload,
    /// Usage section (design 1a/1c); absent until the first scan completes.
    usage: Option<UsageSnapshot>,
    /// Footer lamp connection indicator.
    lamp: LampPayload,
}

pub struct Popover {
    pub(crate) setup_busy: bool,
    /// A firmware write is in progress; USB setup and lamp edits wait for it.
    pub(crate) flash_busy: bool,
    flash_progress: u8,
    window: Window,
    webview: WebView,
    anchor: Option<Rect>,
    /// Show is deferred until the page reports its height for the new state,
    /// so the window never flashes at a stale size.
    pending_show: bool,
    hidden_at: Option<Instant>,
    /// Wall time includes time asleep; Instant remains the tray-click debounce clock.
    closed_at: Option<SystemTime>,
    shown_at: Option<Instant>,
    /// Windows: the physical size last applied from the page's reported
    /// devicePixelRatio — re-asserted when tao tries to rescale the window
    /// on monitor-DPI changes (see the ScaleFactorChanged arm in menubar).
    #[cfg(target_os = "windows")]
    wanted: Option<PhysicalSize<u32>>,
    /// Windows: last time on_window_resized re-asserted the wanted size.
    #[cfg(target_os = "windows")]
    enforced_at: Option<Instant>,
    /// BROGLOW_POPOVER_DEBUG: the popover was force-opened without user
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
            .with_title("BroGlow")
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
                if std::env::var_os("BROGLOW_POPOVER_DEBUG").is_some() {
                    eprintln!("popover ipc: {}", req.body());
                }
                if let Ok(msg) = serde_json::from_str::<PopoverMsg>(req.body()) {
                    on_msg(msg);
                }
            })
            .build(&window)
            .context("Building popover webview")?;
        #[cfg(target_os = "macos")]
        unlock_frame_rate(&webview);

        Ok(Self {
            setup_busy: false,
            flash_busy: false,
            flash_progress: 0,
            window,
            webview,
            anchor: None,
            pending_show: false,
            hidden_at: None,
            closed_at: None,
            shown_at: None,
            #[cfg(target_os = "windows")]
            wanted: None,
            #[cfg(target_os = "windows")]
            enforced_at: None,
            debug: std::env::var_os("BROGLOW_POPOVER_DEBUG").is_some(),
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
        let cfg = config::read_config();
        let reset_navigation =
            !self.is_visible() && should_reset_navigation(self.closed_at, SystemTime::now(), &cfg);
        self.anchor = Some(anchor);
        self.pending_show = true;
        self.render_state(state, &cfg, reset_navigation);
    }

    pub fn hide(&mut self) {
        if self.window.is_visible() {
            self.window.set_visible(false);
            self.hidden_at = Some(Instant::now());
            self.closed_at = Some(SystemTime::now());
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
        self.render_state(state, &config::read_config(), false);
    }

    fn render_state(&self, state: &HookState, cfg: &config::Config, reset_navigation: bool) {
        let payload = build_payload(
            state,
            self.setup_busy,
            (self.flash_busy, self.flash_progress),
            cfg,
        );
        let _ = self
            .webview
            .evaluate_script(&format!("window.__setState({payload}, {reset_navigation})"));
    }

    pub fn config_action_result(&self, error: Option<String>) {
        let error = serde_json::to_string(&error).expect("serialize settings error");
        let _ = self
            .webview
            .evaluate_script(&format!("window.__configActionResult({error})"));
    }

    pub fn firmware_started(&mut self) {
        self.flash_busy = true;
        self.flash_progress = 0;
        self.push_state(&crate::state::read_display_state());
    }

    pub fn firmware_progress(&mut self, done: usize, total: usize) {
        self.flash_progress = (done * 100 / total.max(1)).min(100) as u8;
        let _ = self.webview.evaluate_script(&format!(
            "window.__firmwareProgress({})",
            self.flash_progress
        ));
    }

    /// `Ok` is the outcome text; `Err` the failure. Both end the busy state.
    pub fn firmware_result(&mut self, result: Result<String, String>) {
        self.flash_busy = false;
        self.push_state(&crate::state::read_display_state());
        let (message, error) = match &result {
            Ok(message) => (message.as_str(), None),
            Err(error) => ("", Some(error.as_str())),
        };
        let message = serde_json::to_string(message).expect("serialize outcome");
        let error = serde_json::to_string(&error).expect("serialize flash error");
        let _ = self
            .webview
            .evaluate_script(&format!("window.__firmwareResult({message}, {error})"));
    }

    pub fn lamp_setup_result(&mut self, error: Option<String>) {
        self.setup_busy = false;
        self.push_state(&crate::state::read_display_state());
        let error = serde_json::to_string(&error).expect("serialize setup error");
        let _ = self
            .webview
            .evaluate_script(&format!("window.__lampSetupResult({error})"));
    }

    /// The page reported its rendered height: size the window to fit, keep it
    /// glued to the anchor, and complete a deferred show.
    pub fn on_resize(&mut self, logical_height: f64, dpr: Option<f64>) {
        let size = LogicalSize::new(WIDTH, logical_height.clamp(120.0, 720.0));
        // Windows: tao is wrong about this window three ways over — its
        // set_inner_size never resizes the window at all (stuck at the
        // builder size; the uncovered client area painted an opaque white
        // gutter), it pads requests by AdjustWindowRect extents for a frame
        // the undecorated window doesn't have, and both tao's scale_factor()
        // and Win32's GetDpiForWindow disagree with the scale WebView2
        // actually renders at on scaled monitors (the card drew 1.5x larger
        // than the window and got clipped). Size the frameless window
        // directly with SetWindowPos (outer == client) using the page's own
        // devicePixelRatio — the one scale that by definition matches the
        // rendered surface — and hand wry the logical size, since its bounds
        // are logical and it scales by the webview's own DPI. If a move to a
        // different-DPI monitor changes the ratio, the page's next render
        // reports the new dpr and this converges.
        #[cfg(target_os = "windows")]
        {
            let scale = dpr
                .filter(|d| *d > 0.0)
                .unwrap_or_else(|| self.true_scale());
            let want: PhysicalSize<u32> = size.to_physical(scale);
            self.wanted = Some(want);
            self.set_size_raw(want);
            let _ = self.webview.set_bounds(wry::Rect {
                position: tao::dpi::LogicalPosition::new(0.0, 0.0).into(),
                size: size.into(),
            });
            if let Some(anchor) = self.anchor {
                self.position_at(anchor, want);
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = dpr;
            self.window.set_inner_size(size);
            if let Some(anchor) = self.anchor {
                self.position_at(anchor, size.to_physical(self.window.scale_factor()));
            }
        }
        if self.pending_show {
            self.pending_show = false;
            self.window.set_visible(true);
            self.window.set_focus();
            self.shown_at = Some(Instant::now());
            self.closed_at = None;
        }
    }

    /// The physical size last applied from the page's devicePixelRatio, used
    /// by the menubar's ScaleFactorChanged handler to veto tao's rescaling.
    #[cfg(target_os = "windows")]
    pub fn wanted_physical(&self) -> Option<PhysicalSize<u32>> {
        self.wanted
    }

    /// Windows: tao rescales the window to the monitor's DPI on
    /// WM_DPICHANGED, which fights the devicePixelRatio-derived size when the
    /// two disagree (stale system DPI, virtual displays). Snap back whenever
    /// a resize lands somewhere other than the wanted size, rate-limited so a
    /// hypothetical ping-pong can't spin the event loop.
    #[cfg(target_os = "windows")]
    pub fn on_window_resized(&mut self, new_size: PhysicalSize<u32>) {
        let Some(want) = self.wanted else { return };
        if new_size == want {
            return;
        }
        if self
            .enforced_at
            .is_some_and(|t| t.elapsed() < Duration::from_millis(100))
        {
            return;
        }
        self.enforced_at = Some(Instant::now());
        self.set_size_raw(want);
        if let Some(anchor) = self.anchor {
            self.position_at(anchor, want);
        }
    }

    /// The window's real scale factor from its per-monitor DPI —
    /// tao's scale_factor() reports 1.0 regardless of monitor scaling.
    #[cfg(target_os = "windows")]
    fn true_scale(&self) -> f64 {
        use tao::platform::windows::WindowExtWindows;
        #[link(name = "user32")]
        extern "system" {
            fn GetDpiForWindow(hwnd: isize) -> u32;
        }
        let dpi = unsafe { GetDpiForWindow(self.window.hwnd()) };
        if dpi == 0 {
            self.window.scale_factor()
        } else {
            f64::from(dpi) / 96.0
        }
    }

    /// Set the window's size in physical pixels via SetWindowPos — the window
    /// is frameless, so outer == client and no frame adjustment applies.
    #[cfg(target_os = "windows")]
    fn set_size_raw(&self, want: PhysicalSize<u32>) {
        use tao::platform::windows::WindowExtWindows;
        #[link(name = "user32")]
        extern "system" {
            fn SetWindowPos(
                hwnd: isize,
                after: isize,
                x: i32,
                y: i32,
                cx: i32,
                cy: i32,
                flags: u32,
            ) -> i32;
        }
        // SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE
        const SWP_RESIZE_ONLY: u32 = 0x0002 | 0x0004 | 0x0010;
        // SetWindowPos consults WM_GETMINMAXINFO, and tao pins a
        // non-resizable window's min/max to its own idea of the size —
        // resize attempts get clamped right back (set_resizable(true) doesn't
        // lift this synchronously). Point the clamp at the size we want
        // instead: with min == max == want, whatever tao's handler enforces
        // IS the target.
        self.window.set_min_inner_size(Some(want));
        self.window.set_max_inner_size(Some(want));
        unsafe {
            SetWindowPos(
                self.window.hwnd(),
                0,
                0,
                0,
                want.width as i32,
                want.height as i32,
                SWP_RESIZE_ONLY,
            );
        }
    }

    /// Position the window against the tray icon rect (physical px, top-left
    /// origin on every platform — tray-icon pre-flips macOS coordinates).
    /// macOS hangs the card below the menu bar icon; Windows floats it above
    /// the taskbar. Horizontally centered on the icon, clamped to the monitor.
    fn position_at(&self, anchor: Rect, size: PhysicalSize<u32>) {
        #[cfg(target_os = "windows")]
        let scale = self.true_scale();
        #[cfg(not(target_os = "windows"))]
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

fn should_reset_navigation(
    closed_at: Option<SystemTime>,
    now: SystemTime,
    cfg: &config::Config,
) -> bool {
    cfg.popover_reset_enabled
        && closed_at
            .and_then(|closed| now.duration_since(closed).ok())
            .is_some_and(|elapsed| {
                elapsed >= Duration::from_secs(u64::from(cfg.popover_reset_minutes) * 60)
            })
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
/// The page's session rows: live sessions only, labelled over exactly those,
/// so a stale done session never lengthens a live one's label.
fn session_payloads(state: &HookState) -> Vec<SessionPayload<'_>> {
    let live: Vec<(&String, &SessionStatus)> = state
        .sessions
        .iter()
        .filter(|(_, s)| s.status != Status::Done)
        .collect();
    let labels = crate::session::project_labels(
        live.iter()
            .filter_map(|(_, s)| s.project_path.as_deref())
            .filter(|p| !p.is_empty()),
    );
    live.into_iter()
        .map(|(id, s)| SessionPayload {
            id,
            name: display_name(id, s),
            project: s
                .project_path
                .as_deref()
                .and_then(|p| labels.get(p))
                .cloned()
                .unwrap_or_else(|| crate::session::NO_PROJECT.to_string()),
            project_path: s.project_path.as_deref().unwrap_or_default(),
            status: &s.status,
            capacity_blocked: s.is_codex_capacity(),
            last_updated: &s.last_updated,
            agent: crate::harness::agent(s.harness.as_deref()).into(),
            host: crate::terminal::host(s.harness.as_deref(), s.terminal.as_ref()).map(Into::into),
        })
        .collect()
}

fn build_payload(
    state: &HookState,
    setup_busy: bool,
    flash: (bool, u8),
    cfg: &config::Config,
) -> String {
    let connection = crate::led::connection_status();
    let payload = StatePayload {
        aggregate: agg_str(aggregate(state, cfg.yellow_mode)),
        sessions: session_payloads(state),
        settings: SettingsPayload {
            show_dashboard_button: cfg.show_dashboard_button,
            show_agent_names: cfg.show_agent_names,
            group_by_project: cfg.group_by_project,
            popover_reset_enabled: cfg.popover_reset_enabled,
            popover_reset_minutes: cfg.popover_reset_minutes,
            auto_add_forgotten_usb: cfg.auto_add_forgotten_usb,
            yellow_mode: cfg.yellow_mode,
            usage_enabled: cfg.usage_enabled,
            show_usage_in_menu_bar: cfg.show_usage_in_menu_bar,
        },
        // Only surface usage once the user has opted in — otherwise the section
        // stays absent even if a stale snapshot is still cached.
        usage: cfg.usage_enabled.then(usage::latest).flatten(),
        lamp: LampPayload {
            names: cfg.lamp_names.clone(),
            bluetooth_supported: cfg!(target_os = "macos"),
            setup_busy,
            present: cfg.led_transport == LedTransport::Usb && crate::led::board_present_cached(),
            enabled: cfg.led_enabled,
            transport: cfg.led_transport,
            selected_count: cfg.ble_ids().len(),
            state: connection.state,
            detail: connection.detail,
            devices: connection.devices,
            brightness_percent: cfg.led_brightness_percent(),
            brightness_by_id: cfg
                .ble_ids()
                .into_iter()
                .map(|id| {
                    let percent = cfg.lamp_brightness_percent(&id);
                    (id, percent)
                })
                .collect(),
            usb_present: crate::led::board_present_cached(),
            firmware: FirmwarePayload {
                available: crate::flash::bundled_version(),
                busy: flash.0,
                progress: flash.1,
            },
        },
    };
    serde_json::to_string(&payload)
        .unwrap_or_else(|_| r#"{"aggregate":"none","sessions":[]}"#.to_string())
}

/// WKWebView renders at 60 fps by default, even on a 120 Hz ProMotion display,
/// so popover scrolling looks choppier than native views. Turn off WebKit's
/// `PreferPageRenderingUpdatesNear60FPSEnabled` feature (Safari's "Prefer Page
/// Rendering Updates near 60fps" flag) through the private `_WKFeature` API.
/// Each selector is checked first, so a WebKit without it keeps 60 fps.
#[cfg(target_os = "macos")]
fn unlock_frame_rate(webview: &WebView) {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::{msg_send, sel};
    use std::ffi::{c_char, CStr};
    use wry::WebViewExtMacOS;

    const FEATURE: &CStr = c"PreferPageRenderingUpdatesNear60FPSEnabled";
    let Some(class) = AnyClass::get(c"WKPreferences") else {
        return;
    };
    let wk = webview.webview();
    // SAFETY: plain Objective-C messages to WebKit objects; every private
    // selector is guarded by respondsToSelector:.
    unsafe {
        let has_features: bool = msg_send![class, respondsToSelector: sel!(_features)];
        if !has_features {
            return;
        }
        let config: Retained<AnyObject> = msg_send![&*wk, configuration];
        let prefs: Retained<AnyObject> = msg_send![&*config, preferences];
        let can_set: bool = msg_send![&*prefs, respondsToSelector: sel!(_setEnabled:forFeature:)];
        if !can_set {
            return;
        }
        let features: Retained<AnyObject> = msg_send![class, _features];
        let count: usize = msg_send![&*features, count];
        for i in 0..count {
            let feature: Retained<AnyObject> = msg_send![&*features, objectAtIndex: i];
            let key: Option<Retained<AnyObject>> = msg_send![&*feature, key];
            let Some(key) = key else { continue };
            let utf8: *const c_char = msg_send![&*key, UTF8String];
            if !utf8.is_null() && CStr::from_ptr(utf8) == FEATURE {
                let _: () = msg_send![&*prefs, _setEnabled: false, forFeature: &*feature];
                return;
            }
        }
    }
}

fn popover_html() -> String {
    const HTML: &str = include_str!("../assets/popover.html");
    const STYLES: &str = include_str!("../assets/popover/popover.css");
    const SCRIPT: &str = include_str!("../assets/popover/popover.js");
    const PRESENTATION: &str = include_str!("../assets/popover/presentation.js");
    let platform = if cfg!(target_os = "macos") {
        "mac"
    } else {
        "win"
    };
    HTML.replace("__POPOVER_STYLE__", STYLES)
        .replace("__POPOVER_PRESENTATION__", PRESENTATION)
        .replace("__POPOVER_HOST__", "")
        .replace("__POPOVER_SCRIPT__", SCRIPT)
        .replace("__PLATFORM__", platform)
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
    use super::{base64, popover_html, session_payloads, should_reset_navigation, PopoverMsg};
    use crate::config::Config;
    use crate::state::{HookState, SessionStatus, Status};
    use std::time::{Duration, SystemTime};

    #[test]
    fn shared_popover_assets_are_embedded_without_template_tokens() {
        let html = popover_html();
        for token in ["__POPOVER_", "__PLATFORM__", "__ICON_"] {
            assert!(!html.contains(token), "unresolved token: {token}");
        }
        assert!(html.contains(include_str!("../assets/popover/popover.css")));
        assert!(html.contains(include_str!("../assets/popover/presentation.js")));
        assert!(html.contains("window.__setState"));
        assert!(html.contains("window.ipc.postMessage"));
    }

    #[test]
    fn popover_reset_requires_a_full_closed_interval_and_respects_opt_out() {
        let closed = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let mut cfg = Config::default();
        let after = |seconds| closed + Duration::from_secs(seconds);
        assert!(!should_reset_navigation(None, after(3600), &cfg));
        assert!(!should_reset_navigation(Some(closed), after(299), &cfg));
        assert!(should_reset_navigation(Some(closed), after(300), &cfg));
        assert!(should_reset_navigation(Some(closed), after(3600), &cfg));
        // Reopening and closing starts a new interval, rather than accumulating time.
        assert!(!should_reset_navigation(Some(after(299)), after(300), &cfg));
        cfg.popover_reset_minutes = 60;
        assert!(!should_reset_navigation(Some(closed), after(3599), &cfg));
        assert!(should_reset_navigation(Some(closed), after(3600), &cfg));
        cfg.popover_reset_enabled = false;
        assert!(!should_reset_navigation(Some(closed), after(86400), &cfg));
    }

    #[test]
    fn popover_reset_handles_sleep_and_backward_clock_changes() {
        let closed = SystemTime::UNIX_EPOCH + Duration::from_secs(1000);
        let cfg = Config::default();
        // Only wall time matters: no timer ticks are required while asleep.
        assert!(should_reset_navigation(
            Some(closed),
            closed + Duration::from_secs(3600),
            &cfg
        ));
        assert!(!should_reset_navigation(
            Some(closed),
            closed - Duration::from_secs(1),
            &cfg
        ));
    }

    #[test]
    fn popover_reset_ipc_accepts_only_whole_minutes_in_range() {
        for minutes in [1, 5, 60, 1440] {
            let msg: PopoverMsg = serde_json::from_value(serde_json::json!({
                "cmd": "set_popover_reset_minutes", "minutes": minutes
            }))
            .unwrap();
            assert!(
                matches!(msg, PopoverMsg::SetPopoverResetMinutes { minutes: value } if value == minutes)
            );
        }
        for value in ["0", "-1", "1441", "65536", "1.5", "null", "\"5\""] {
            assert!(
                serde_json::from_str::<PopoverMsg>(&format!(
                    r#"{{"cmd":"set_popover_reset_minutes","minutes":{value}}}"#
                ))
                .is_err(),
                "{value}"
            );
        }
        for enabled in [false, true] {
            let msg: PopoverMsg = serde_json::from_value(serde_json::json!({
                "cmd": "set_popover_reset", "enabled": enabled
            }))
            .unwrap();
            assert!(
                matches!(msg, PopoverMsg::SetPopoverReset { enabled: value } if value == enabled)
            );
        }
    }

    #[test]
    fn session_payloads_carry_the_project_path_and_a_label_over_live_sessions() {
        let session = |status: Status, path: Option<&str>| SessionStatus {
            status,
            last_updated: "2026-01-01T00:00:00Z".into(),
            project_path: path.map(str::to_string),
            notification_type: None,
            name: Some("Task".into()),
            terminal: None,
            harness: None,
            codex_background: false,
        };
        let mut state = HookState::default();
        state
            .sessions
            .insert("a".into(), session(Status::Active, Some("/a/recco")));
        state
            .sessions
            .insert("b".into(), session(Status::Inactive, Some("/b/recco")));
        state
            .sessions
            .insert("none".into(), session(Status::Active, None));
        state
            .sessions
            .insert("old".into(), session(Status::Done, Some("/c/recco")));
        state.sessions.get_mut("b").unwrap().harness = Some("mystery".into());
        let payloads = serde_json::to_value(session_payloads(&state)).unwrap();
        let by_id = |id: &str| {
            payloads
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["id"] == id)
                .cloned()
        };
        assert!(by_id("old").is_none(), "done sessions stay off the page");
        assert_eq!(by_id("a").unwrap()["agent"]["label"], "Claude");
        assert!(by_id("a").unwrap()["agent"]["icon"].is_string());
        assert_eq!(by_id("b").unwrap()["agent"]["badge"], "my");
        assert!(by_id("b").unwrap()["agent"].get("icon").is_none());
        assert_eq!(by_id("a").unwrap()["projectPath"], "/a/recco");
        assert_eq!(by_id("a").unwrap()["project"], "a/recco");
        assert_eq!(by_id("b").unwrap()["project"], "b/recco");
        assert_eq!(by_id("none").unwrap()["projectPath"], "");
        assert_eq!(by_id("none").unwrap()["project"], "No project");

        // Without the paused one, the done /c/recco must not keep the
        // live label long.
        state.sessions.remove("b");
        let payloads = serde_json::to_value(session_payloads(&state)).unwrap();
        let live = payloads
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "a")
            .unwrap();
        assert_eq!(live["project"], "recco");
    }

    #[test]
    fn session_payloads_expose_capacity_without_losing_session_metadata() {
        let mut state = HookState::default();
        state.sessions.insert(
            "capacity".into(),
            serde_json::from_value(serde_json::json!({
                "status": "needs_help",
                "last_updated": "2026-01-01T00:00:00Z",
                "name": "Ship the fix",
                "project_path": "/work/broglow",
                "harness": "codex",
                "terminal": { "bundle_id": "com.openai.codex" },
                "notification_type": "codex_capacity"
            }))
            .unwrap(),
        );
        let payloads = serde_json::to_value(session_payloads(&state)).unwrap();
        let row = &payloads[0];
        assert_eq!(row["capacityBlocked"], true);
        assert_eq!(row["name"], "Ship the fix");
        assert_eq!(row["project"], "broglow");
        assert_eq!(row["projectPath"], "/work/broglow");
        assert_eq!(row["agent"]["label"], "Codex");
        assert_eq!(row["host"]["name"], "Codex app");
        state.sessions.get_mut("capacity").unwrap().status = Status::Active;
        let payloads = serde_json::to_value(session_payloads(&state)).unwrap();
        assert_eq!(payloads[0]["capacityBlocked"], false);
    }

    #[test]
    fn brightness_ipc_enforces_the_lamp_percentage_range() {
        for command in [
            "set_brightness",
            "set_lamp_brightness",
            "preview_brightness",
        ] {
            for percent in [0, 100] {
                let msg: PopoverMsg = serde_json::from_value(serde_json::json!({
                    "cmd": command, "percent": percent, "id": "saved-lamp"
                }))
                .unwrap();
                assert!(matches!(msg,
                    PopoverMsg::SetBrightness { percent: value }
                    | PopoverMsg::SetLampBrightness { percent: value, .. }
                    | PopoverMsg::PreviewBrightness { percent: value, .. } if value == percent
                ));
            }
            let error = serde_json::from_value::<PopoverMsg>(serde_json::json!({
                "cmd": command, "percent": 101, "id": "saved-lamp"
            }))
            .unwrap_err();
            assert!(error
                .to_string()
                .contains("Brightness must be between 0 and 100"));
        }
        assert!(matches!(
            serde_json::from_str::<PopoverMsg>(r#"{"cmd":"preview_brightness","percent":0}"#)
                .unwrap(),
            PopoverMsg::PreviewBrightness {
                id: None,
                percent: 0
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
