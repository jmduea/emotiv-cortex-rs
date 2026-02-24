//! Tab bar widget.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Tabs as RatatuiTabs};

use crate::app::{App, Tab};

/// Render the tab bar.
pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line<'_>> = Tab::all()
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let num = format!("{}", i + 1);
            Line::from(vec![
                Span::styled(
                    num,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(":"),
                Span::raw(tab.label()),
            ])
        })
        .collect();

    let selected = Tab::all()
        .iter()
        .position(|&t| t == app.active_tab)
        .unwrap_or(0);

    let tabs = RatatuiTabs::new(titles)
        .block(Block::default().borders(Borders::BOTTOM))
        .select(selected)
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::raw(" │ "));

    frame.render_widget(tabs, area);
}
