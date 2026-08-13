//! Dashboard tab — overview with metrics gauges, mental command, session info.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, Paragraph};

use crate::app::{App, ConnectionPhase};

/// Render the dashboard tab content.
pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    if app.phase != ConnectionPhase::Ready {
        let loading = Paragraph::new(app.phase.label())
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().title(" Dashboard ").borders(Borders::ALL));
        frame.render_widget(loading, area);
        return;
    }

    // Split into left (session info + mental command) and right (metrics gauges)
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    draw_session_info(frame, app, chunks[0]);
    draw_metrics_panel(frame, app, chunks[1]);
}

/// Left panel: session info, headset summary, active subscriptions.
fn draw_session_info(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines = Vec::new();

    // Headset
    lines.push(Line::from(vec![
        Span::styled(" Headset: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            app.headset_id.as_deref().unwrap_or("—"),
            Style::default().fg(Color::Cyan),
        ),
    ]));

    // Model
    if let Some(ref model) = app.headset_model {
        lines.push(Line::from(vec![
            Span::styled("   Model: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(
                "{} ({} ch, {} Hz)",
                model,
                model.num_channels(),
                model.sampling_rate_hz()
            )),
        ]));
    }

    // Session
    lines.push(Line::from(vec![
        Span::styled(" Session: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            app.session_id
                .as_deref()
                .map_or("—".to_string(), |s| s[..16.min(s.len())].to_string()),
            Style::default().fg(Color::Cyan),
        ),
    ]));

    lines.push(Line::from(""));

    // Active subscriptions
    lines.push(Line::from(Span::styled(
        " Active Streams:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    if app.subscribed_streams.is_empty() {
        lines.push(Line::from(Span::styled(
            "   (none)",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for st in &app.subscribed_streams {
            lines.push(Line::from(format!("   ● {st:?}")));
        }
    }

    lines.push(Line::from(""));

    // Mental command (latest)
    if let Some(ref mc) = app.mental_command {
        lines.push(Line::from(vec![
            Span::styled(
                " Mental Cmd: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(&mc.action, Style::default().fg(Color::Magenta)),
            Span::raw(format!(" ({:.2})", mc.power)),
        ]));
    }

    // Facial expression (latest)
    if let Some(ref fe) = app.facial_expression {
        lines.push(Line::from(vec![
            Span::styled(" Face: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(
                "eye={} upper={} ({:.2}) lower={} ({:.2})",
                fe.eye_action,
                fe.upper_face_action,
                fe.upper_face_power,
                fe.lower_face_action,
                fe.lower_face_power,
            )),
        ]));
    }

    let block = Block::default()
        .title(" Session Info ")
        .borders(Borders::ALL);
    let paragraph = Paragraph::new(lines).block(block);

    frame.render_widget(paragraph, area);
}

/// Right panel: performance metrics as horizontal gauge bars.
fn draw_metrics_panel(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Performance Metrics ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(ref m) = app.metrics else {
        let placeholder = Paragraph::new("  Waiting for metrics data…")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(placeholder, inner);
        return;
    };

    let metrics = [
        ("Attention ", m.attention),
        ("Engagement", m.engagement),
        ("Excitement", m.excitement),
        ("Interest  ", m.interest),
        ("Relaxation", m.relaxation),
        ("Stress    ", m.stress),
    ];

    // 2 lines per metric: label line + gauge
    let constraints: Vec<Constraint> = metrics
        .iter()
        .flat_map(|_| [Constraint::Length(1), Constraint::Length(1)])
        .chain(std::iter::once(Constraint::Min(0)))
        .collect();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, (label, value)) in metrics.iter().enumerate() {
        let val = value.unwrap_or(0.0);
        let color = metric_color(val);

        let label_line = Line::from(vec![
            Span::raw("  "),
            Span::styled(*label, Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("  {:.0}%", val * 100.0)),
        ]);
        frame.render_widget(Paragraph::new(label_line), rows[i * 2]);

        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
            .ratio(f64::from(val).clamp(0.0, 1.0))
            .label("");

        frame.render_widget(gauge, rows[i * 2 + 1]);
    }
}

/// Pick a color based on metric magnitude.
fn metric_color(val: f32) -> Color {
    if val > 0.7 {
        Color::Green
    } else if val > 0.4 {
        Color::Yellow
    } else {
        Color::Red
    }
}
