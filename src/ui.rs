use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table, Wrap},
    Frame,
};

use crate::app::App;
use crate::state::Status;

// One Dark-flavored palette from the dashboard redesign (Tray Menu Designs,
// card 2a). Plain cells + RGB colors — no new dependencies.
const GREEN: Color = Color::Rgb(0x98, 0xc3, 0x79);
const YELLOW: Color = Color::Rgb(0xe5, 0xc0, 0x7b);
const RED: Color = Color::Rgb(0xe0, 0x6c, 0x75);
const CYAN: Color = Color::Rgb(0x56, 0xb6, 0xc2);
const DIM: Color = Color::Rgb(0x5c, 0x63, 0x70);
const WHITE: Color = Color::Rgb(0xe8, 0xea, 0xf0);
const RULE: Color = Color::Rgb(0x3a, 0x3f, 0x4d);
const SEL_BG: Color = Color::Rgb(0x26, 0x2b, 0x36);
const HELP_BG: Color = Color::Rgb(0x30, 0x23, 0x2c);
const FOOT_BG: Color = Color::Rgb(0x1d, 0x20, 0x29);
const FOOT_FG: Color = Color::Rgb(0x8a, 0x8d, 0x98);
const KBD_BG: Color = Color::Rgb(0x2a, 0x2e, 0x3a);
// The AGENT column (One Dark purple): the agent's name, or its short tag
// (`cc`, `cx`) with names off. Labels come from the shared `harness::agent`,
// so the TUI and popover can't diverge.
const BADGE_FG: Color = Color::Rgb(0xc6, 0x78, 0xdd);

pub fn render(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header: name/version + count chips
            Constraint::Length(1), // Rule
            Constraint::Min(3),    // Sessions or lamp details
            Constraint::Length(1), // Keycap hint bar
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);

    let rule =
        Paragraph::new("─".repeat(chunks[1].width as usize)).style(Style::default().fg(RULE));
    frame.render_widget(rule, chunks[1]);

    if app.lamp_details {
        render_lamp_details(frame, app, chunks[2]);
    } else {
        render_table(frame, app, chunks[2]);
    }
    render_hint_bar(frame, app, chunks[3]);

    if !app.truecolor {
        to_256_colors(frame.buffer_mut());
    }
}

/// Whether the terminal advertises 24-bit color. Terminals that don't (macOS
/// Terminal.app before 26) read an RGB escape's components as separate SGR
/// codes, so `48;2;48;40;58` paints a black background; without the
/// advertisement, draw from the xterm 256-color palette instead.
pub fn supports_truecolor() -> bool {
    std::env::var("COLORTERM").is_ok_and(|v| v == "truecolor" || v == "24bit")
}

fn to_256_colors(buffer: &mut ratatui::buffer::Buffer) {
    for cell in &mut buffer.content {
        if let Color::Rgb(r, g, b) = cell.fg {
            cell.fg = Color::Indexed(ansi256(r, g, b));
        }
        if let Color::Rgb(r, g, b) = cell.bg {
            cell.bg = Color::Indexed(ansi256(r, g, b));
        }
    }
}

/// The nearest xterm-256 color: the 6×6×6 cube or the 24-step gray ramp.
fn ansi256(r: u8, g: u8, b: u8) -> u8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let nearest = |v: u8| {
        (0..6)
            .min_by_key(|&i| LEVELS[i].abs_diff(v))
            .expect("six levels")
    };
    let distance = |(r2, g2, b2): (u8, u8, u8)| {
        [(r, r2), (g, g2), (b, b2)]
            .iter()
            .map(|&(a, c)| u32::from(a.abs_diff(c)).pow(2))
            .sum::<u32>()
    };
    let (ri, gi, bi) = (nearest(r), nearest(g), nearest(b));
    let cube = (LEVELS[ri], LEVELS[gi], LEVELS[bi]);
    let average = (u32::from(r) + u32::from(g) + u32::from(b)) / 3;
    let step = (average.saturating_sub(3) / 10).min(23) as u8;
    let gray = 8 + 10 * step;
    if distance((gray, gray, gray)) < distance(cube) {
        232 + step
    } else {
        16 + 36 * ri as u8 + 6 * gi as u8 + bi as u8
    }
}

fn render_lamp_details(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let mut lines = vec![
        Line::styled(
            " Lamp details",
            Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
        ),
        Line::from(format!(" {}", app.led_status.detail)),
    ];
    for device in &app.led_status.devices {
        lines.push(Line::from(""));
        lines.push(Line::from(format!(" {} — {}", device.id, device.state)));
        lines.push(Line::from(format!(" {}", device.detail)));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((app.lamp_details_scroll, 0))
            .style(Style::default().fg(FOOT_FG)),
        area,
    );
}

/// The app name and version on the left; live count chips and LED state on the right.
fn render_header(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let working = count(app, Status::Active);
    let paused = count(app, Status::Inactive);
    let help = count(app, Status::NeedsHelp);

    let chip = |n: usize, label: &str, color: Color, bold: bool| {
        let mut style = Style::default().fg(if n == 0 { DIM } else { color });
        if bold && n > 0 {
            style = style.add_modifier(Modifier::BOLD);
        }
        Span::styled(format!("● {n} {label}"), style)
    };

    let transport = match app.led_transport {
        crate::config::LedTransport::Usb => "USB",
        crate::config::LedTransport::Ble => "BLE",
    };
    let led_state = if app.led_enabled {
        app.led_status.state.as_str()
    } else {
        "disabled"
    };
    let led_style = Style::default().fg(match led_state {
        "connected" => GREEN,
        "disabled" => DIM,
        "error" | "unavailable" => RED,
        _ => YELLOW,
    });
    let led_text = if app.led_transport == crate::config::LedTransport::Ble && app.led_enabled {
        let connected = app
            .led_status
            .devices
            .iter()
            .filter(|device| device.state == "connected")
            .count();
        format!(
            "{transport} {connected}/{} {led_state}",
            app.led_selected_count
        )
    } else {
        format!("{transport} {led_state}")
    };

    let (tray_text, tray_style) = if app.tray_running {
        ("tray ● on", Style::default().fg(GREEN))
    } else {
        ("tray ○ off (t)", Style::default().fg(YELLOW))
    };

    // A narrow terminal (80 columns) can't fit everything: drop the empty
    // chips first, then the version, so the lamp status is never cut off.
    let build = |show_version: bool, hide_empty: bool| {
        let mut left = vec![
            Span::raw(" "),
            Span::styled(
                "BroGlow",
                Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
            ),
        ];
        if show_version {
            left.push(Span::raw(" "));
            left.push(Span::styled(
                concat!("v", env!("CARGO_PKG_VERSION")),
                Style::default().fg(DIM),
            ));
        }
        let mut right = Vec::new();
        for (n, label, color, bold) in [
            (working, "working", GREEN, false),
            (paused, "paused", YELLOW, false),
            (help, "needs help", RED, true),
        ] {
            if hide_empty && n == 0 {
                continue;
            }
            if !right.is_empty() {
                right.push(Span::raw("  "));
            }
            right.push(chip(n, label, color, bold));
        }
        if !right.is_empty() {
            right.push(Span::styled(" │ ", Style::default().fg(DIM)));
        }
        right.push(Span::styled(tray_text, tray_style));
        right.push(Span::styled(" │ ", Style::default().fg(DIM)));
        right.push(Span::styled(led_text.clone(), led_style));
        right.push(Span::raw(" "));
        (left, right)
    };
    let width = |spans: &[Span]| {
        spans
            .iter()
            .map(|s| s.content.chars().count())
            .sum::<usize>()
    };
    let fits = |(left, right): &(Vec<Span<'static>>, Vec<Span<'static>>)| {
        width(left) + 1 + width(right) <= area.width as usize
    };
    let (left, right) = [(true, false), (true, true), (false, true)]
        .into_iter()
        .map(|(version, hide)| build(version, hide))
        .find(fits)
        .unwrap_or_else(|| build(false, true));

    frame.render_widget(
        Paragraph::new(justified_line(left, right, area.width)),
        area,
    );
}

fn render_table(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    // AGENT and HOST are only as wide as their longest value, so SESSION and
    // PROJECT keep the rest of an 80-column terminal. HOST says just "app" for
    // an agent's desktop app: the AGENT column already names it.
    let labels: Vec<(String, String)> = app
        .sessions
        .iter()
        .map(|s| {
            let agent = crate::harness::agent(s.harness.as_deref());
            let agent = if app.show_agent_names {
                agent.label
            } else {
                agent.badge
            };
            let host = match &s.host {
                Some(h) if h.kind == crate::terminal::HostKind::App => "app".to_string(),
                Some(h) => h.name.clone(),
                None => "—".to_string(),
            };
            (agent, host)
        })
        .collect();
    let widest = |pick: fn(&(String, String)) -> &String, min: usize, max: usize| {
        let longest = labels.iter().map(|l| pick(l).chars().count()).max();
        longest.unwrap_or(0).clamp(min, max) as u16
    };
    let agent_width = widest(|l| &l.0, 2, 10);
    let host_width = widest(|l| &l.1, 4, 16);

    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("SESSION"),
        Cell::from(if agent_width >= 5 { "AGENT" } else { "" }),
        Cell::from("PROJECT"),
        Cell::from("HOST"),
        Cell::from(Line::from("LAST").right_aligned()),
        Cell::from(Line::from("STATUS").right_aligned()),
    ])
    .style(Style::default().fg(DIM));

    let selected = app.table_state.selected();
    let rows: Vec<Row> = app
        .sessions
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let is_selected = selected == Some(i);

            let (dot, dot_color, status_text, status_style) = match s.status {
                Status::NeedsHelp => (
                    "●",
                    RED,
                    "needs help",
                    Style::default().fg(RED).add_modifier(Modifier::BOLD),
                ),
                Status::Active => ("●", GREEN, "working", Style::default().fg(GREEN)),
                Status::Inactive => ("●", YELLOW, "paused", Style::default().fg(YELLOW)),
                Status::Done => ("○", DIM, "done", Style::default().fg(DIM)),
            };

            let name_style = match s.status {
                Status::NeedsHelp => Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
                Status::Done => Style::default().fg(DIM),
                _ => Style::default().fg(WHITE),
            };

            // Selection reads as a cyan bar on the left edge + a soft row
            // highlight, not a ">>" color reversal.
            let bar = if is_selected {
                Span::styled("▌", Style::default().fg(CYAN))
            } else {
                Span::raw(" ")
            };

            let dim = Style::default().fg(DIM);
            let (agent, host) = &labels[i];
            let mut row = Row::new(vec![
                Cell::from(Line::from(vec![
                    bar,
                    Span::styled(dot, Style::default().fg(dot_color)),
                ])),
                Cell::from(Span::styled(s.name.clone(), name_style)),
                Cell::from(Span::styled(agent.clone(), Style::default().fg(BADGE_FG))),
                Cell::from(Span::styled(s.project_name.clone(), dim)),
                Cell::from(Span::styled(host.clone(), dim)),
                Cell::from(
                    Line::from(Span::styled(format_relative_time(&s.last_updated), dim))
                        .right_aligned(),
                ),
                Cell::from(Line::from(Span::styled(status_text, status_style)).right_aligned()),
            ]);

            if is_selected {
                row = row.style(Style::default().bg(SEL_BG));
            } else if s.status == Status::NeedsHelp {
                row = row.style(Style::default().bg(HELP_BG));
            }
            row
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(2), // selection bar + status dot
            Constraint::Fill(3),   // session
            Constraint::Length(agent_width),
            Constraint::Fill(2), // project
            Constraint::Length(host_width),
            Constraint::Length(4),  // last
            Constraint::Length(10), // status
        ],
    )
    .header(header);

    frame.render_stateful_widget(table, area, &mut app.table_state);
}

/// Keycap-style hint bar; a recent action shows transient feedback in its
/// place for a few seconds.
fn render_hint_bar(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let transient = app
        .status_message
        .as_ref()
        .filter(|(_, t)| t.elapsed().as_secs() < 4)
        .map(|(msg, _)| msg.clone());

    let line = if let Some(msg) = transient {
        Line::from(vec![
            Span::raw(" "),
            Span::styled(msg, Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
        ])
    } else {
        let mut left = Vec::new();
        let keys: &[(&str, &str)] = if app.lamp_details {
            &[
                ("d/Esc", "back"),
                ("j/k", "scroll"),
                ("r", "reload"),
                ("l", "led"),
                ("q", "quit"),
            ]
        } else {
            &[
                ("↵", "focus"),
                ("j/k", "move"),
                ("x", "clear"),
                ("r", "reload"),
                ("t", "tray"),
                ("l", "led"),
                ("d", "details"),
                ("q", "quit"),
            ]
        };
        for (key, label) in keys {
            left.push(Span::raw(" "));
            left.push(Span::styled(
                format!(" {key} "),
                Style::default().bg(KBD_BG).fg(YELLOW),
            ));
            left.push(Span::raw(" "));
            left.push(Span::styled(*label, Style::default().fg(FOOT_FG)));
        }
        Line::from(left)
    };

    frame.render_widget(
        Paragraph::new(line).style(Style::default().bg(FOOT_BG).fg(FOOT_FG)),
        area,
    );
}

/// One line with `left` spans flush left and `right` spans flush right,
/// padded apart with spaces. All our header/footer glyphs are single-width.
fn justified_line(
    left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    width: u16,
) -> Line<'static> {
    let used: usize = left
        .iter()
        .chain(right.iter())
        .map(|s| s.content.chars().count())
        .sum();
    // Never glue the two sides together, even when they overflow.
    let pad = (width as usize).saturating_sub(used).max(1);
    let mut spans = left;
    spans.push(Span::raw(" ".repeat(pad)));
    spans.extend(right);
    Line::from(spans)
}

fn count(app: &App, status: Status) -> usize {
    app.sessions.iter().filter(|s| s.status == status).count()
}

/// Compact relative time for the LAST column: `now`, `7m`, `4h`, `3d`, `2mo`.
fn format_relative_time(iso_str: &str) -> String {
    use chrono::{DateTime, Utc};

    let Ok(dt) = iso_str.parse::<DateTime<Utc>>() else {
        return "—".to_string();
    };
    let duration = Utc::now().signed_duration_since(dt);

    if duration.num_minutes() < 1 {
        "now".to_string()
    } else if duration.num_minutes() < 60 {
        format!("{}m", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{}h", duration.num_hours())
    } else if duration.num_days() < 30 {
        format!("{}d", duration.num_days())
    } else {
        format!("{}mo", duration.num_days() / 30)
    }
}

#[cfg(test)]
mod tests {
    use super::ansi256;

    #[test]
    fn ansi256_picks_the_nearest_cube_or_gray_entry() {
        assert_eq!(ansi256(0, 0, 0), 16);
        assert_eq!(ansi256(255, 255, 255), 231);
        assert_eq!(ansi256(255, 0, 0), 196);
        // The rule/badge grays land on the gray ramp, not a tinted cube entry.
        assert_eq!(ansi256(0x3a, 0x3f, 0x4d), 238);
        assert_eq!(ansi256(0x98, 0xc3, 0x79), 108); // GREEN stays green: (135, 175, 135)
    }
}
