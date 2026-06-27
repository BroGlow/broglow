use std::collections::HashMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use notify::{EventKind, RecursiveMode, Watcher};
use ratatui::{backend::CrosstermBackend, widgets::TableState, Terminal};

use crate::notification::send_macos_notification;
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

        Self {
            sessions,
            table_state,
            should_quit: false,
            previous_statuses,
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
                    send_macos_notification(
                        "clawlight",
                        &format!("\"{}\" needs help!", session.name),
                    );
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
        let state_dir = state::state_file_path()
            .parent()
            .map(|p| p.to_path_buf());
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

        loop {
            terminal.draw(|f| ui::render(f, self))?;

            // Poll for events with 250ms timeout
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                self.should_quit = true;
                            }
                            KeyCode::Down | KeyCode::Char('j') => self.next(),
                            KeyCode::Up | KeyCode::Char('k') => self.previous(),
                            KeyCode::Char('r') => self.reload_data(),
                            KeyCode::Char('x') => self.clear_selected(),
                            _ => {}
                        }
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
