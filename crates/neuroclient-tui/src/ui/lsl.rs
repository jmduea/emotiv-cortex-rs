//! LSL tab — stream selection checklist and active-stream XML metadata viewer.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
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

    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    {
        use crate::app::ConnectionPhase;
        if let Some(ref handle) = app.lsl_streaming {
            draw_active(frame, app, handle, inner);
        } else if app.phase == ConnectionPhase::Ready {
            draw_selection(frame, app, inner);
        } else {
            draw_waiting(frame, inner);
        }
    }

    #[cfg(not(all(feature = "lsl", not(target_os = "linux"))))]
    {
        let para = Paragraph::new(Span::styled(
            "  LSL feature not enabled. Build with --features lsl",
            Style::default().fg(Color::DarkGray),
        ));
        frame.render_widget(para, inner);
    }
}

// ─── Waiting state ───────────────────────────────────────────────────────────

fn draw_waiting(frame: &mut Frame, area: Rect) {
    let para = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Waiting for connection before LSL can be started…",
            Style::default().fg(Color::DarkGray),
        )),
    ]);
    frame.render_widget(para, area);
}

// ─── Stream selection checklist ──────────────────────────────────────────────

#[cfg(all(feature = "lsl", not(target_os = "linux")))]
fn draw_selection(frame: &mut Frame, app: &App, area: Rect) {
    use crate::lsl::LslStream;

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  Select streams to publish, then press Enter or l to start:",
        Style::default().fg(Color::Cyan),
    )));
    lines.push(Line::from(""));

    for (i, &stream) in LslStream::all().iter().enumerate() {
        let checked = app.lsl_selected_streams.contains(&stream);
        let is_cursor = i == app.lsl_cursor;

        let checkbox = if checked { "[x]" } else { "[ ]" };
        let cursor_glyph = if is_cursor { "▶ " } else { "  " };

        let row_style = if is_cursor {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else if checked {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        lines.push(Line::from(vec![
            Span::raw(format!("  {cursor_glyph}")),
            Span::styled(checkbox, row_style),
            Span::styled(format!("  {}", stream.label()), row_style),
        ]));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "  ↑↓ Move   Space Toggle   a Select All   n Clear   Enter/l Start",
        Style::default().fg(Color::DarkGray),
    )));

    frame.render_widget(Paragraph::new(lines), area);
}

// ─── Active streaming view ───────────────────────────────────────────────────

#[cfg(all(feature = "lsl", not(target_os = "linux")))]
fn draw_active(frame: &mut Frame, app: &App, handle: &crate::lsl::LslStreamingHandle, area: Rect) {
    if app.lsl_show_xml && !handle.stream_xml_metadata.is_empty() {
        // Split vertically: top half = status summary, bottom half = XML viewer
        let split = area.height.saturating_sub(area.height / 2);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(split), Constraint::Min(0)])
            .split(area);
        draw_status_summary(frame, app, handle, chunks[0]);
        draw_xml_viewer(frame, app, handle, chunks[1]);
    } else {
        draw_status_summary(frame, app, handle, area);
    }
}

/// Compact status panel rendered at the top of the active view.
#[cfg(all(feature = "lsl", not(target_os = "linux")))]
fn draw_status_summary(
    frame: &mut Frame,
    app: &App,
    handle: &crate::lsl::LslStreamingHandle,
    area: Rect,
) {
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

    let mut lines: Vec<Line> = Vec::new();

    // Status + uptime
    lines.push(Line::from(vec![
        Span::styled("  Status: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("Active ▶", Style::default().fg(Color::Green)),
        Span::raw("   "),
        Span::styled("Uptime: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(time_str),
    ]));

    lines.push(Line::from(""));

    // Active outlets
    lines.push(Line::from(Span::styled(
        "  Active Outlets:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    for outlet in &handle.active_streams {
        lines.push(Line::from(format!("    {outlet}")));
    }

    lines.push(Line::from(""));

    // Sample counts
    lines.push(Line::from(Span::styled(
        "  Sample Counts:",
        Style::default().add_modifier(Modifier::BOLD),
    )));
    for (name, count) in handle.sample_counts.iter() {
        let n = count.load(std::sync::atomic::Ordering::Relaxed);
        lines.push(Line::from(format!("    {name:<25} {n:>12} samples")));
    }

    lines.push(Line::from(""));

    // Key hints
    let xml_hint = if app.lsl_show_xml {
        "x Hide XML"
    } else {
        "x Show XML"
    };
    lines.push(Line::from(Span::styled(
        format!("  l Stop   {xml_hint}"),
        Style::default().fg(Color::DarkGray),
    )));

    frame.render_widget(Paragraph::new(lines), area);
}

/// XML metadata viewer rendered below the status panel.
#[cfg(all(feature = "lsl", not(target_os = "linux")))]
fn draw_xml_viewer(
    frame: &mut Frame,
    app: &App,
    handle: &crate::lsl::LslStreamingHandle,
    area: Rect,
) {
    let xml_count = handle.stream_xml_metadata.len();
    let idx = if xml_count == 0 {
        0
    } else {
        app.lsl_xml_stream_idx.min(xml_count - 1)
    };

    let (stream_name, xml_content) = handle
        .stream_xml_metadata
        .get(idx)
        .map(|(n, x)| (n.as_str(), x.as_str()))
        .unwrap_or(("", ""));

    let nav_hint = if xml_count > 1 {
        format!(
            "  ◀/▶ switch stream [{}/{}]   ↑↓ scroll",
            idx + 1,
            xml_count
        )
    } else {
        "  ↑↓ scroll".to_string()
    };

    let title = format!(" XML: {stream_name}{nav_hint} ");

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines: Vec<Line> = xml_content
        .lines()
        .map(|l| {
            Line::from(Span::styled(
                l.to_string(),
                Style::default().fg(Color::White),
            ))
        })
        .collect();

    // Clamp the scroll so the paragraph never scrolls into empty space,
    // which would leave stale terminal cells visible.
    let max_scroll = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_sub(inner.height);
    let scroll = app.lsl_xml_scroll.min(max_scroll);

    frame.render_widget(Paragraph::new(lines).scroll((scroll, 0)), inner);
}
