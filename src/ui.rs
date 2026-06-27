use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::app::App;
use crate::state::Status;

pub fn render(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title
            Constraint::Min(5),   // Table
            Constraint::Length(1), // Status bar
        ])
        .split(frame.area());

    // Title
    let title = Paragraph::new("clawlight — Claude Code session monitor")
        .alignment(Alignment::Center)
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::BOTTOM));
    frame.render_widget(title, chunks[0]);

    // Session table
    let header = Row::new(vec![
        Cell::from("Name"),
        Cell::from("Status"),
        Cell::from("Project"),
        Cell::from("Branch"),
        Cell::from("Msgs"),
        Cell::from("Modified"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .sessions
        .iter()
        .map(|s| {
            let status_style = match s.status {
                Status::Active => Style::default().fg(Color::Green),
                Status::Inactive => Style::default().fg(Color::Yellow),
                Status::NeedsHelp => Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
                Status::Done => Style::default().fg(Color::DarkGray),
            };

            let status_text = match s.status {
                Status::Active => "active",
                Status::Inactive => "inactive",
                Status::NeedsHelp => "NEEDS HELP",
                Status::Done => "done",
            };

            Row::new(vec![
                Cell::from(s.name.clone()),
                Cell::from(status_text).style(status_style),
                Cell::from(s.project_name.clone()),
                Cell::from(s.git_branch.clone().unwrap_or_default()),
                Cell::from(format!("{}", s.message_count)),
                Cell::from(format_relative_time(&s.modified)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(30),
            Constraint::Length(12),
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Length(5),
            Constraint::Length(14),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::NONE))
    .highlight_style(
        Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(Color::Cyan),
    )
    .highlight_symbol(">> ");

    frame.render_stateful_widget(table, chunks[1], &mut app.table_state);

    // Status bar
    let active_count = app
        .sessions
        .iter()
        .filter(|s| s.status == Status::Active)
        .count();
    let help_count = app
        .sessions
        .iter()
        .filter(|s| s.status == Status::NeedsHelp)
        .count();

    let status_spans = vec![
        Span::styled(" q", Style::default().fg(Color::Yellow)),
        Span::raw(":quit  "),
        Span::styled("j/k", Style::default().fg(Color::Yellow)),
        Span::raw(":nav  "),
        Span::styled("r", Style::default().fg(Color::Yellow)),
        Span::raw(":reload  "),
        Span::styled("x", Style::default().fg(Color::Yellow)),
        Span::raw(":clear  "),
        Span::raw("| "),
        Span::styled(
            format!("{} active", active_count),
            Style::default().fg(Color::Green),
        ),
        Span::raw("  "),
        if help_count > 0 {
            Span::styled(
                format!("{} needs help", help_count),
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(
                "0 needs help".to_string(),
                Style::default().fg(Color::DarkGray),
            )
        },
    ];

    let status_bar = Paragraph::new(Line::from(status_spans))
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));
    frame.render_widget(status_bar, chunks[2]);
}

fn format_relative_time(iso_str: &str) -> String {
    use chrono::{DateTime, Utc};

    let parsed = iso_str.parse::<DateTime<Utc>>();
    match parsed {
        Ok(dt) => {
            let now = Utc::now();
            let duration = now.signed_duration_since(dt);

            if duration.num_minutes() < 1 {
                "just now".to_string()
            } else if duration.num_minutes() < 60 {
                format!("{}m ago", duration.num_minutes())
            } else if duration.num_hours() < 24 {
                format!("{}h ago", duration.num_hours())
            } else if duration.num_days() < 30 {
                format!("{}d ago", duration.num_days())
            } else {
                format!("{}mo ago", duration.num_days() / 30)
            }
        }
        Err(_) => iso_str.to_string(),
    }
}
