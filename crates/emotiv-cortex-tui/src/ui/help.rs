//! Help overlay — displayed when the user presses '?'.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::app::App;

/// Render a centered help overlay.
pub fn draw(frame: &mut Frame, _app: &App) {
    let area = centered_rect(60, 70, frame.area());

    // Clear the area behind the overlay
    frame.render_widget(Clear, area);

    let lines = vec![
        Line::from(Span::styled(
            "Keyboard Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        key_line("q / Ctrl+C", "Quit the application"),
        key_line("Tab", "Next tab"),
        key_line("Shift+Tab", "Previous tab"),
        key_line("1-5", "Jump to tab by number"),
        key_line("↑ / k", "Scroll up"),
        key_line("↓ / j", "Scroll down"),
        key_line("v", "Cycle stream view (Streams tab)"),
        key_line("Enter", "Connect to selected headset (Device tab)"),
        key_line("r", "Refresh headset list (Device tab)"),
        key_line("l", "Toggle LSL streaming (LSL tab)"),
        key_line("?", "Toggle this help overlay"),
        Line::from(""),
        Line::from(Span::styled(
            "Press ? to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .title(" Help ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Left);

    frame.render_widget(paragraph, area);
}

/// Format a key binding help line.
fn key_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{key:<16}"),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(desc),
    ])
}

/// Create a centered rect using percentage of the parent area.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
