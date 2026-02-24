//! Status bar widget — always-visible top line showing connection state.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::{App, ConnectionPhase};

/// Render the status bar.
pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = Vec::new();

    // App title
    spans.push(Span::styled(
        " EMOTIV Cortex ",
        Style::default()
            .fg(Color::White)
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD),
    ));

    spans.push(Span::raw(" "));

    // Connection phase or ready state
    match app.phase {
        ConnectionPhase::Ready => {
            // Headset ID
            if let Some(ref id) = app.headset_id {
                spans.push(Span::styled(id.as_str(), Style::default().fg(Color::Cyan)));
            } else {
                spans.push(Span::styled(
                    "No headset",
                    Style::default().fg(Color::DarkGray),
                ));
            }

            spans.push(Span::raw(" │ "));

            // Battery
            if let Some(ref dq) = app.device_quality {
                let pct = dq.battery_percent;
                let color = match pct {
                    0..=15 => Color::Red,
                    16..=40 => Color::Yellow,
                    _ => Color::Green,
                };
                spans.push(Span::styled(
                    format!("🔋 {pct}%"),
                    Style::default().fg(color),
                ));
            } else {
                spans.push(Span::styled("🔋 --", Style::default().fg(Color::DarkGray)));
            }

            spans.push(Span::raw(" │ "));

            // Signal strength
            if let Some(ref dq) = app.device_quality {
                let level = dq.overall_quality;
                let bars = signal_bars(level);
                let color = if level > 0.7 {
                    Color::Green
                } else if level > 0.3 {
                    Color::Yellow
                } else {
                    Color::Red
                };
                spans.push(Span::styled(
                    format!("Signal: {bars}"),
                    Style::default().fg(color),
                ));
            } else {
                spans.push(Span::styled(
                    "Signal: ----",
                    Style::default().fg(Color::DarkGray),
                ));
            }

            spans.push(Span::raw(" │ "));

            // Session indicator
            if app.session_id.is_some() {
                spans.push(Span::styled("●", Style::default().fg(Color::Green)));
            } else {
                spans.push(Span::styled("○", Style::default().fg(Color::DarkGray)));
            }

            // LSL indicator
            #[cfg(all(feature = "lsl", not(target_os = "linux")))]
            if let Some(ref handle) = app.lsl_streaming {
                spans.push(Span::raw(" │ "));
                spans.push(Span::styled(
                    handle.format_status(),
                    Style::default().fg(Color::Magenta),
                ));
            }

            // Uptime
            let uptime = format_duration(app.uptime());
            spans.push(Span::raw("  "));
            spans.push(Span::styled(uptime, Style::default().fg(Color::DarkGray)));
        }
        phase => {
            spans.push(Span::styled(
                phase.label(),
                Style::default().fg(Color::Yellow),
            ));
        }
    }

    let bar = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Black));
    frame.render_widget(bar, area);
}

/// Convert a 0.0–1.0 quality value to a simple bar string.
fn signal_bars(level: f32) -> &'static str {
    if level > 0.8 {
        "████"
    } else if level > 0.6 {
        "███░"
    } else if level > 0.4 {
        "██░░"
    } else if level > 0.2 {
        "█░░░"
    } else {
        "░░░░"
    }
}

/// Format a duration into `HH:MM:SS` or `MM:SS`.
fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}
