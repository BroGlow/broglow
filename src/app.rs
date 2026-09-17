use std::collections::HashMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use notify::{EventKind, RecursiveMode, Watcher};
use ratatui::{backend::CrosstermBackend, widgets::TableState, Terminal};

use crate::config;
use crate::led;
use crate::menubar;
use crate::notification::send_notification;
use crate::session::{load_all_sessions, DisplaySession};
use crate::state::{self, read_hook_state, Status};
use crate::ui;

enum AppEvent {
    FileChanged,
}

pub struct App {
    pub sessions: Vec<DisplaySession>,
    pub table_state: TableState,
    pub should_quit: bool,
    pub lamp_details: bool,
    pub lamp_details_scroll: u16,
    /// Whether ESP32 status LEDs are enabled (mirrors `config.json`).
    pub led_enabled: bool,
    pub led_transport: config::LedTransport,
    pub led_selected_count: usize,
    /// Cached worker result; UI refresh never initializes Bluetooth.
    pub led_status: led::ConnectionStatus,
    /// Whether the tray daemon is running (refreshed on the same timer);
    /// `t` starts it.
    pub tray_running: bool,
    /// Transient one-line feedback shown in the status bar (message + when set).
    pub status_message: Option<(String, Instant)>,
    previous_statuses: HashMap<String, Status>,
}

impl App {
    pub fn new() -> Self {
        let hook_state = read_hook_state();
        let sessions = load_all_sessions(&hook_state);
        let previous_statuses: HashMap<String, Status> = sessions
            .iter()
            .map(|s| (s.session_id.clone(), s.status.clone()))
            .collect();

        let mut table_state = TableState::default();
        if !sessions.is_empty() {
            table_state.select(Some(0));
        }

        let cfg = config::read_config();
        Self {
            sessions,
            table_state,
            should_quit: false,
            lamp_details: false,
            lamp_details_scroll: 0,
            led_enabled: cfg.led_enabled,
            led_transport: cfg.led_transport,
            led_selected_count: cfg.ble_ids().len(),
            led_status: led::connection_status(),
            tray_running: menubar::is_running(),
            status_message: None,
            previous_statuses,
        }
    }

    /// BLE selection is persistent and works without a USB data connection.
    fn toggle_led(&mut self) {
        let mut cfg = config::read_config();
        if !cfg.led_enabled {
            match cfg.led_transport {
                config::LedTransport::Ble if cfg.ble_ids().is_empty() => {
                    self.set_status(
                        "Add a lamp: hold its BOOT button 3s, clawlight ble scan, then clawlight ble add <ID>"
                            .to_string(),
                    );
                    return;
                }
                config::LedTransport::Usb
                    if cfg.led_port.is_none() && led::detect_board().is_none() =>
                {
                    self.set_status("No USB board detected — connect it, or select a lamp with clawlight ble scan / select <ID>.".to_string());
                    return;
                }
                _ => {}
            }
        }
        cfg.led_enabled = !cfg.led_enabled;
        match config::write_config(&cfg) {
            Ok(()) => {
                self.refresh_led();
                self.set_status(if cfg.led_enabled {
                    "LED enabled — waiting for the tray service to connect.".to_string()
                } else {
                    "LED disabled.".to_string()
                });
            }
            Err(err) => self.set_status(format!("Could not save LED setting: {err}")),
        }
    }

    fn refresh_led(&mut self) {
        let cfg = config::read_config();
        self.led_enabled = cfg.led_enabled;
        self.led_transport = cfg.led_transport;
        self.led_selected_count = cfg.ble_ids().len();
        self.led_status = led::connection_status();
    }

    /// `t`: start the tray daemon if it isn't running (launchd on macOS,
    /// detached spawn elsewhere — see `menubar::ensure_running`).
    fn start_tray(&mut self) {
        let msg = menubar::ensure_running();
        // The process can take a beat to appear; the periodic check below
        // corrects the chip either way.
        self.tray_running = menubar::is_running();
        self.set_status(msg);
    }

    fn set_status(&mut self, msg: String) {
        self.status_message = Some((msg, Instant::now()));
    }

    /// Best-effort focus of the selected session's terminal window, using
    /// the terminal identity the hook backend captured (same logic as the
    /// tray popover — see `terminal::focus`). Runs synchronously: the brief
    /// osascript pause is fine for a deliberate keypress.
    fn focus_selected(&mut self) {
        let (session_id, project_path, name) = {
            let Some(session) = self
                .table_state
                .selected()
                .and_then(|i| self.sessions.get(i))
            else {
                return;
            };
            (
                session.session_id.clone(),
                session.project_path.clone(),
                session.name.clone(),
            )
        };

        let terminal = read_hook_state()
            .sessions
            .get(&session_id)
            .and_then(|s| s.terminal.clone());
        let path = (!project_path.is_empty()).then_some(project_path.as_str());
        if crate::terminal::focus(terminal.as_ref(), path) {
            self.set_status(format!("Focused the terminal window for \"{name}\"."));
        } else {
            self.set_status(format!("Couldn't find a terminal window for \"{name}\"."));
        }
    }

    pub fn reload_data(&mut self) {
        let hook_state = read_hook_state();
        let sessions = load_all_sessions(&hook_state);

        // Check for needs_help transitions and notify
        for session in &sessions {
            if session.status == Status::NeedsHelp {
                let prev = self.previous_statuses.get(&session.session_id);
                if prev != Some(&Status::NeedsHelp) {
                    send_notification("clawlight", &format!("\"{}\" needs help!", session.name));
                }
            }
        }

        self.previous_statuses = sessions
            .iter()
            .map(|s| (s.session_id.clone(), s.status.clone()))
            .collect();

        // Preserve selection if possible
        let selected = self.table_state.selected().unwrap_or(0);
        self.sessions = sessions;
        if !self.sessions.is_empty() {
            self.table_state
                .select(Some(selected.min(self.sessions.len() - 1)));
        }
    }

    fn next(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.sessions.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn clear_selected(&mut self) {
        let Some(idx) = self.table_state.selected() else {
            return;
        };
        let Some(session) = self.sessions.get(idx) else {
            return;
        };
        if state::clear_session(&session.session_id).is_ok() {
            self.reload_data();
        }
    }

    fn previous(&mut self) {
        if self.sessions.is_empty() {
            return;
        }
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.sessions.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('d') => {
                self.lamp_details = !self.lamp_details;
                self.lamp_details_scroll = 0;
                self.status_message = None;
            }
            KeyCode::Esc if self.lamp_details => self.lamp_details = false,
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Down | KeyCode::Char('j') if self.lamp_details => {
                self.lamp_details_scroll = self.lamp_details_scroll.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k') if self.lamp_details => {
                self.lamp_details_scroll = self.lamp_details_scroll.saturating_sub(1);
            }
            KeyCode::Enter if !self.lamp_details => self.focus_selected(),
            KeyCode::Down | KeyCode::Char('j') => self.next(),
            KeyCode::Up | KeyCode::Char('k') => self.previous(),
            KeyCode::Char('x') if !self.lamp_details => self.clear_selected(),
            KeyCode::Char('r') => {
                self.reload_data();
                self.refresh_led();
            }
            KeyCode::Char('l') => self.toggle_led(),
            KeyCode::Char('t') => self.start_tray(),
            _ => {}
        }
    }

    pub fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::channel::<AppEvent>();

        // Set up file watcher
        let tx_watcher = tx.clone();
        let mut watcher =
            notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    match event.kind {
                        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                            let _ = tx_watcher.send(AppEvent::FileChanged);
                        }
                        _ => {}
                    }
                }
            })?;

        // Watch state file directory
        let state_dir = state::state_file_path().parent().map(|p| p.to_path_buf());
        if let Some(ref dir) = state_dir {
            if dir.exists() {
                let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
            }
        }

        // Watch projects directory
        if let Some(home) = dirs::home_dir() {
            let projects_dir = home.join(".claude").join("projects");
            if projects_dir.exists() {
                let _ = watcher.watch(&projects_dir, RecursiveMode::Recursive);
            }
        }

        let mut last_refresh = Instant::now();
        let refresh_interval = Duration::from_secs(5);

        let mut last_led_check = Instant::now();
        let led_check_interval = Duration::from_secs(3);

        loop {
            terminal.draw(|f| ui::render(f, self))?;

            // Pick up CLI selections and the worker's actual connection result.
            if last_led_check.elapsed() >= led_check_interval {
                self.refresh_led();
                self.tray_running = menubar::is_running();
                last_led_check = Instant::now();
            }

            // Poll for events with 250ms timeout
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code);
                    }
                }
            }

            // Check for file change events (non-blocking drain)
            let mut file_changed = false;
            while let Ok(event) = rx.try_recv() {
                if matches!(event, AppEvent::FileChanged) {
                    file_changed = true;
                }
            }

            if file_changed || last_refresh.elapsed() >= refresh_interval {
                self.reload_data();
                last_refresh = Instant::now();
            }

            if self.should_quit {
                break;
            }
        }

        // Keep watcher alive
        drop(watcher);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    fn app() -> App {
        App {
            sessions: vec![],
            table_state: TableState::default().with_selected(Some(0)),
            should_quit: false,
            lamp_details: false,
            lamp_details_scroll: 0,
            led_enabled: true,
            led_transport: config::LedTransport::Ble,
            led_selected_count: 2,
            led_status: led::ConnectionStatus {
                state: "connected".into(),
                detail: "One nearby lamp is responding".into(),
                devices: vec![
                    led::DeviceStatus::new("HOME-DEVICE-ID", "connected", "Write acknowledged"),
                    led::DeviceStatus::new(
                        "OFFICE-DEVICE-ID",
                        "searching",
                        "Waiting for advertisement",
                    ),
                ],
            },
            tray_running: true,
            status_message: None,
            previous_statuses: HashMap::new(),
        }
    }

    fn screen(app: &mut App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(160, 18)).unwrap();
        terminal.draw(|frame| ui::render(frame, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn lamp_details_are_separate_from_session_dashboard() {
        let mut app = app();
        let dashboard = screen(&mut app);
        assert!(dashboard.contains("SESSION"));
        assert!(dashboard.contains("BLE 1/2 connected"));
        assert!(!dashboard.contains("HOME-DEVICE-ID"));
        assert!(!dashboard.contains("One nearby lamp is responding"));
        app.handle_key(KeyCode::Char('d'));
        let details = screen(&mut app);
        assert!(!details.contains("SESSION"));
        assert!(details.contains("Lamp details"));
        assert!(details.contains("HOME-DEVICE-ID — connected"));
        assert!(details.contains("OFFICE-DEVICE-ID — searching"));
        assert!(details.contains("Write acknowledged"));
        assert!(details.contains("Waiting for advertisement"));
        app.handle_key(KeyCode::Char('d'));
        assert!(screen(&mut app).contains("SESSION"));
        assert!(!app.should_quit);
    }

    #[test]
    fn lamp_details_scroll_and_escape_preserve_dashboard_selection() {
        let mut app = app();
        app.handle_key(KeyCode::Char('d'));
        app.handle_key(KeyCode::Down);
        app.handle_key(KeyCode::Char('j'));
        assert_eq!(app.lamp_details_scroll, 2);
        assert_eq!(app.table_state.selected(), Some(0));
        app.handle_key(KeyCode::Up);
        app.handle_key(KeyCode::Char('k'));
        app.handle_key(KeyCode::Char('k'));
        assert_eq!(app.lamp_details_scroll, 0);
        app.handle_key(KeyCode::Esc);
        assert!(!app.lamp_details && !app.should_quit);
        app.handle_key(KeyCode::Esc);
        assert!(app.should_quit);
    }
}
