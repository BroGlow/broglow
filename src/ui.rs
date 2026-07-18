use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
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
// Per-harness badge (One Dark purple on a muted fill) — only non-Claude
// sessions carry one, so Claude rows stay unadorned. The label comes from the
// shared `session::harness_badge` so the TUI and popover can't diverge.
const BADGE_FG: Color = Color::Rgb(0xc6, 0x78, 0xdd);
const BADGE_BG: Color = Color::Rgb(0x30, 0x28, 0x3a);

pub fn render(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header: name/version + count chips
            Constraint::Length(1), // Rule
            Constraint::Min(3),    // Table
            Constraint::Length(1), // Keycap hint bar
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);

    let rule =
        Paragraph::new("─".repeat(chunks[1].width as usize)).style(Style::default().fg(RULE));
    frame.render_widget(rule, chunks[1]);

    render_table(frame, app, chunks[2]);
    render_hint_bar(frame, app, chunks[3]);
}

/// `clawlight v0.8.0` on the left; live count chips and LED state on the right.
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

    let (led_text, led_style) = match (app.led_enabled, app.led_detected) {
        (true, true) => ("LED ● on", Style::default().fg(GREEN)),
        (true, false) => ("LED ● no board", Style::default().fg(YELLOW)),
        (false, true) => ("LED ○ off (l)", Style::default().fg(CYAN)),
        (false, false) => ("LED ○ off", Style::default().fg(DIM)),
    };

    let left = vec![
        Span::raw(" "),
        Span::styled(
            "clawlight",
            Style::default().fg(WHITE).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(
            concat!("v", env!("CARGO_PKG_VERSION")),
            Style::default().fg(DIM),
        ),
    ];
    let right = vec![
        chip(working, "working", GREEN, false),
        Span::raw("  "),
        chip(paused, "paused", YELLOW, false),
        Span::raw("  "),
        chip(help, "needs help", RED, true),
        Span::styled(" │ ", Style::default().fg(DIM)),
        Span::styled(led_text, led_style),
        Span::raw(" "),
    ];

    frame.render_widget(
        Paragraph::new(justified_line(left, right, area.width)),
        area,
    );
}

fn render_table(frame: &mut Frame, app: &mut App, area: ratatui::layout::Rect) {
    let header = Row::new(vec![
        Cell::from(""),
        Cell::from("SESSION"),
        Cell::from("PROJECT"),
        Cell::from("BRANCH"),
        Cell::from(Line::from("MSGS").right_aligned()),
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

            let branch = s
                .git_branch
                .as_ref()
                .map(|b| format!("⎇ {b}"))
                .unwrap_or_else(|| "—".to_string());

            // A subtle harness tag ahead of the name for non-Claude sessions;
            // Claude rows (no harness) show just the name.
            let mut name_spans = Vec::new();
            if let Some(h) = &s.harness {
                name_spans.push(Span::styled(
                    format!(" {} ", crate::session::harness_badge(h)),
                    Style::default().fg(BADGE_FG).bg(BADGE_BG),
                ));
                name_spans.push(Span::raw(" "));
            }
            name_spans.push(Span::styled(s.name.clone(), name_style));

            let dim = Style::default().fg(DIM);
            let mut row = Row::new(vec![
                Cell::from(Line::from(vec![
                    bar,
                    Span::styled(dot, Style::default().fg(dot_color)),
                ])),
                Cell::from(Line::from(name_spans)),
                Cell::from(Span::styled(s.project_name.clone(), dim)),
                Cell::from(Span::styled(branch, dim)),
                Cell::from(
                    Line::from(Span::styled(s.message_count.to_string(), dim)).right_aligned(),
                ),
                Cell::from(
                    Line::from(Span::styled(format_relative_time(&s.modified), dim))
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
            Constraint::Length(2),  // selection bar + status dot
            Constraint::Fill(3),    // session
            Constraint::Fill(2),    // project
            Constraint::Fill(2),    // branch
            Constraint::Length(4),  // msgs
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
        for (key, label) in [
            ("↵", "focus"),
            ("j/k", "move"),
            ("x", "clear"),
            ("r", "reload"),
            ("l", "led"),
            ("q", "quit"),
        ] {
            left.push(Span::raw(" "));
            left.push(Span::styled(
                format!(" {key} "),
                Style::default().bg(KBD_BG).fg(YELLOW),
            ));
            left.push(Span::raw(" "));
            left.push(Span::styled(label, Style::default().fg(FOOT_FG)));
        }
        let right = vec![
            Span::styled(
                "~/.claude/clawlight/state.json · live",
                Style::default().fg(DIM),
            ),
            Span::raw(" "),
        ];
        justified_line(left, right, area.width)
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
    let pad = (width as usize).saturating_sub(used);
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
