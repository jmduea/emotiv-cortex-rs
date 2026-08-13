//! Log tab — scrollable list of application events.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;
use crate::event::LogLevel;

/// Render the log tab.
pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(format!(" Log ({} entries) ", app.log_entries.len()))
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.log_entries.is_empty() {
        let msg =
            Paragraph::new("  No log entries yet.").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(msg, inner);
        return;
    }

    let visible_height = inner.height as usize;
    let total = app.log_entries.len();

    // Determine scroll position
    let scroll = if app.log_auto_scroll {
        total.saturating_sub(visible_height)
    } else {
        (app.scroll_offset as usize).min(total.saturating_sub(visible_height))
    };

    let lines: Vec<Line<'_>> = app
        .log_entries
        .iter()
        .skip(scroll)
        .take(visible_height)
        .map(|entry| {
            let elapsed = entry.timestamp.elapsed();
            let age = format_age(elapsed);

            let (level_str, level_color) = match entry.level {
                LogLevel::Info => ("INFO", Color::Green),
                LogLevel::Warn => ("WARN", Color::Yellow),
                LogLevel::Error => ("ERR ", Color::Red),
            };

            Line::from(vec![
                Span::styled(format!(" {age:>8} "), Style::default().fg(Color::DarkGray)),
                Span::styled(
                    level_str,
                    Style::default()
                        .fg(level_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::raw(&entry.message),
            ])
        })
        .collect();

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}

/// Format an elapsed duration as a human-readable age string.
fn format_age(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else {
        format!("{}h ago", secs / 3600)
    }
}
