//! Top-level TUI layout and rendering entry point.
//!
//! Composes the status bar, tab bar, active tab content, and key-help
//! footer into the full-screen layout drawn each frame.

pub mod dashboard;
pub mod device;
pub mod help;
pub mod log;
pub mod status_bar;
pub mod streams;
pub mod tabs;

#[cfg(all(feature = "lsl", not(target_os = "linux")))]
pub mod lsl;

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};

use crate::app::App;

/// Render the entire TUI frame.
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Top-level vertical split:
    //   [1] Status bar (1 line)
    //   [2] Tab bar    (3 lines)
    //   [3] Content    (fill)
    //   [4] Key help   (1 line)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar
            Constraint::Length(3), // tab bar
            Constraint::Min(10),   // content area
            Constraint::Length(1), // key help
        ])
        .split(area);

    status_bar::draw(frame, app, chunks[0]);
    tabs::draw(frame, app, chunks[1]);

    // Render the active tab's content
    match app.active_tab {
        crate::app::Tab::Dashboard => dashboard::draw(frame, app, chunks[2]),
        crate::app::Tab::Streams => streams::draw(frame, app, chunks[2]),
        #[cfg(all(feature = "lsl", not(target_os = "linux")))]
        crate::app::Tab::Lsl => lsl::draw(frame, app, chunks[2]),
        crate::app::Tab::Device => device::draw(frame, app, chunks[2]),
        crate::app::Tab::Log => log::draw(frame, app, chunks[2]),
    }

    // Key help footer
    draw_key_help(frame, app, chunks[3]);

    // Help overlay (if toggled)
    if app.show_help {
        help::draw(frame, app);
    }
}

/// Render the bottom key-help bar.
fn draw_key_help(frame: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    use ratatui::style::{Color, Style};
    use ratatui::text::{Line, Span};
    use ratatui::widgets::Paragraph;

    let mut spans = vec![
        Span::styled(" q", Style::default().fg(Color::Yellow)),
        Span::raw(" Quit  "),
        Span::styled("Tab", Style::default().fg(Color::Yellow)),
        Span::raw(" Switch  "),
        Span::styled("1-5", Style::default().fg(Color::Yellow)),
        Span::raw(" Jump  "),
        Span::styled("↑↓", Style::default().fg(Color::Yellow)),
        Span::raw(" Scroll  "),
    ];

    if app.active_tab == crate::app::Tab::Streams {
        spans.push(Span::styled("v", Style::default().fg(Color::Yellow)));
        spans.push(Span::raw(" View  "));
    }

    if app.active_tab == crate::app::Tab::Device {
        if app.phase == crate::app::ConnectionPhase::Discovered {
            spans.push(Span::styled("Enter", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" Connect  "));
        }
        if app.phase == crate::app::ConnectionPhase::Ready {
            spans.push(Span::styled("d", Style::default().fg(Color::Yellow)));
            spans.push(Span::raw(" Disconnect  "));
        }
        spans.push(Span::styled("r", Style::default().fg(Color::Yellow)));
        spans.push(Span::raw(" Refresh  "));
    }

    #[cfg(all(feature = "lsl", not(target_os = "linux")))]
    if app.active_tab == crate::app::Tab::Lsl {
        spans.push(Span::styled("l", Style::default().fg(Color::Yellow)));
        spans.push(Span::raw(" Toggle LSL  "));
    }

    spans.push(Span::styled("?", Style::default().fg(Color::Yellow)));
    spans.push(Span::raw(" Help"));

    let help = Paragraph::new(Line::from(spans)).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, area);
}
