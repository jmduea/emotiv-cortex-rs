//! LSL tab — toggle LSL streaming, view per-stream stats.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;

/// Render the LSL tab.
pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Lab Streaming Layer ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    {
        if let Some(ref handle) = app.lsl_streaming {
            lines.push(Line::from(vec![
                Span::styled("  Status: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled("Active ▶", Style::default().fg(Color::Green)),
            ]));

            let elapsed = handle.started_at.elapsed();
            let secs = elapsed.as_secs();
            let h = secs / 3600;
            let m = (secs % 3600) / 60;
            let s = secs % 60;
            let time_str = if h > 0 {
                format!("{h}h {m}m {s}s")
            } else if m > 0 {
                format!("{m}m {s}s")
            } else {
                format!("{s}s")
            };

            lines.push(Line::from(vec![
                Span::styled("  Uptime: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(time_str),
            ]));

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Active Outlets:",
                Style::default().add_modifier(Modifier::BOLD),
            )));

            for outlet in &handle.active_streams {
                lines.push(Line::from(format!("    {outlet}")));
            }

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  Sample Counts:",
                Style::default().add_modifier(Modifier::BOLD),
            )));

            for (name, count) in handle.sample_counts.iter() {
                let n = count.load(std::sync::atomic::Ordering::Relaxed);
                lines.push(Line::from(format!("    {name:<25} {n:>12} samples")));
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled("  Status: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled("Inactive", Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from(""));
            if app.phase == crate::app::ConnectionPhase::Ready {
                lines.push(Line::from(
                    "  Press 'l' to start LSL streaming with all available streams.",
                ));
            } else {
                lines.push(Line::from(Span::styled(
                    "  Waiting for connection before LSL can be started…",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
    }

    #[cfg(not(all(feature = "lsl", not(target_os = "linux"))))]
    {
        lines.push(Line::from(Span::styled(
            "  LSL feature not enabled. Build with --features lsl",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, inner);
}
