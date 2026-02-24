//! Device tab — headset discovery list + detail view.
//!
//! Before connection the user sees a selectable list of discovered
//! headsets and can press Enter to connect.  After connection the tab
//! shows full headset metadata and per-channel contact quality gauges.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph};

use crate::app::{App, ConnectionPhase};

/// Render the device tab.
pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    match app.phase {
        ConnectionPhase::Discovered => draw_headset_list(frame, app, area),
        ConnectionPhase::ConnectingHeadset => {
            let msg = Paragraph::new("  Connecting to headset…")
                .style(Style::default().fg(Color::Yellow))
                .block(
                    Block::default()
                        .title(" Device ")
                        .borders(Borders::ALL),
                );
            frame.render_widget(msg, area);
        }
        ConnectionPhase::Ready => draw_connected_detail(frame, app, area),
        _ => {
            let msg = Paragraph::new("  Authenticating… please wait.")
                .style(Style::default().fg(Color::DarkGray))
                .block(
                    Block::default()
                        .title(" Device ")
                        .borders(Borders::ALL),
                );
            frame.render_widget(msg, area);
        }
    }
}

// ─── Headset selection list ──────────────────────────────────────────────

/// Render a selectable list of discovered headsets.
fn draw_headset_list(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Select Headset ")
        .borders(Borders::ALL);

    if app.discovered_headsets.is_empty() {
        let msg = Paragraph::new(vec![
            Line::from(""),
            Line::from("  No headsets found."),
            Line::from("  Make sure your headset is powered on."),
            Line::from(""),
            Line::from(Span::styled(
                "  Press 'r' to refresh",
                Style::default().fg(Color::Cyan),
            )),
        ])
        .style(Style::default().fg(Color::DarkGray))
        .block(block);
        frame.render_widget(msg, area);
        return;
    }

    let items: Vec<ListItem<'_>> = app
        .discovered_headsets
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let selected = i == app.selected_headset_idx;
            let marker = if selected { "▸ " } else { "  " };

            let status_color = match h.status.as_str() {
                "connected" => Color::Green,
                "discovered" => Color::Yellow,
                _ => Color::DarkGray,
            };

            let line = Line::from(vec![
                Span::styled(
                    marker,
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    &h.id,
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    &h.status,
                    Style::default().fg(status_color),
                ),
                Span::raw(battery_label(h.battery_percent)),
            ]);

            let style = if selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

/// Format an optional battery percentage for display.
fn battery_label(pct: Option<u32>) -> String {
    pct.map_or_else(String::new, |p| format!("  🔋 {p}%"))
}

// ─── Connected detail view ──────────────────────────────────────────────

/// Two-panel detail view shown when a headset is connected (phase = Ready).
fn draw_connected_detail(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_headset_info(frame, app, chunks[0]);
    draw_contact_quality(frame, app, chunks[1]);
}

/// Left panel: headset metadata.
fn draw_headset_info(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Headset Info ")
        .borders(Borders::ALL);

    let Some(ref info) = app.headset_info else {
        let msg = Paragraph::new("  No headset connected")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(msg, area);
        return;
    };

    let mut lines = Vec::new();

    let field = |label: &str, value: &str| -> Line<'_> {
        Line::from(vec![
            Span::styled(
                format!("  {label:<20}"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(value.to_string()),
        ])
    };

    lines.push(field("ID", &info.id));
    lines.push(field("Status", &info.status));

    if let Some(ref conn) = info.connected_by {
        lines.push(field("Connected By", conn));
    }
    if let Some(ref fw) = info.firmware {
        lines.push(field("Firmware", fw));
    }
    if let Some(ref fw_disp) = info.firmware_display {
        lines.push(field("Firmware (display)", fw_disp));
    }
    if let Some(ref mode) = info.mode {
        lines.push(field("Mode", mode));
    }
    if let Some(ref name) = info.custom_name {
        lines.push(field("Custom Name", name));
    }
    if let Some(pct) = info.battery_percent {
        lines.push(field("Battery", &format!("{pct}%")));
    }
    if let Some(sig) = info.signal_strength {
        lines.push(field("Signal Strength", &format!("{sig}")));
    }
    if let Some(ref power) = info.power {
        lines.push(field("Power", power));
    }
    if let Some(uptime) = info.system_up_time {
        lines.push(field("System Uptime", &format!("{uptime}s")));
    }
    if let Some(ref sensors) = info.sensors {
        lines.push(field("EEG Sensors", &sensors.join(", ")));
    }
    if let Some(ref motion) = info.motion_sensors {
        lines.push(field("Motion Sensors", &motion.join(", ")));
    }
    if let Some(is_virtual) = info.is_virtual {
        lines.push(field("Virtual", &format!("{is_virtual}")));
    }

    // Model details from enum
    if let Some(ref model) = app.headset_model {
        lines.push(Line::from(""));
        lines.push(field("Model", &model.to_string()));
        lines.push(field("Channels", &model.num_channels().to_string()));
        lines.push(field(
            "Sample Rate",
            &format!("{} Hz", model.sampling_rate_hz()),
        ));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}

/// Right panel: per-channel contact quality as colored gauges.
fn draw_contact_quality(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Contact Quality ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(ref dq) = app.device_quality else {
        let msg = Paragraph::new("  Waiting for device quality data…")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(msg, inner);
        return;
    };

    let channel_names: Vec<String> = app
        .headset_model
        .as_ref()
        .map_or_else(
            || {
                (0..dq.channel_quality.len())
                    .map(|i| format!("Ch{i}"))
                    .collect()
            },
            |m| m.channel_names().iter().map(|s| (*s).to_string()).collect(),
        );

    let num = dq.channel_quality.len();
    let constraints: Vec<Constraint> = (0..num)
        .flat_map(|_| [Constraint::Length(1), Constraint::Length(1)])
        .chain(std::iter::once(Constraint::Min(0)))
        .collect();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    for (i, &quality) in dq.channel_quality.iter().enumerate() {
        let label = channel_names.get(i).map_or("?", |s| s.as_str());
        let color = quality_color(quality);

        let label_line = Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{label:<6}"),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{:.0}%", quality * 100.0),
                Style::default().fg(color),
            ),
        ]);
        frame.render_widget(Paragraph::new(label_line), rows[i * 2]);

        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
            .ratio(f64::from(quality).clamp(0.0, 1.0))
            .label("");

        frame.render_widget(gauge, rows[i * 2 + 1]);
    }
}

/// Map contact quality (0.0–1.0) to a color.
fn quality_color(q: f32) -> Color {
    if q > 0.7 {
        Color::Green
    } else if q > 0.3 {
        Color::Yellow
    } else {
        Color::Red
    }
}
