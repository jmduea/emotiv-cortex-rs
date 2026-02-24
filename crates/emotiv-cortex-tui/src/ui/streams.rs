//! Streams tab — live sparkline / chart views for EEG, Motion, Band Power.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Axis, Bar, BarChart, BarGroup, Block, Borders, Chart, Dataset, GraphType, Paragraph, Sparkline,
};

use crate::app::{App, StreamView};

/// Render the streams tab content.
pub fn draw(frame: &mut Frame, app: &App, area: Rect) {
    // Header showing current view + how to switch
    let header_area = Rect { height: 1, ..area };
    let content_area = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    };

    let header = Line::from(vec![
        Span::styled(" View: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(
            app.stream_view.label(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "  (press 'v' to cycle)",
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    frame.render_widget(Paragraph::new(header), header_area);

    match app.stream_view {
        StreamView::Eeg => draw_eeg(frame, app, content_area),
        StreamView::Motion => draw_motion(frame, app, content_area),
        StreamView::BandPower => draw_band_power(frame, app, content_area),
    }
}

/// EEG sparklines — one per channel.
fn draw_eeg(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" EEG Channels ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.eeg_buffers.is_empty() {
        let msg =
            Paragraph::new("  Waiting for EEG data…").style(Style::default().fg(Color::DarkGray));
        frame.render_widget(msg, inner);
        return;
    }

    let num_ch = app.eeg_buffers.len();
    let channel_names: Vec<String> = app.headset_model.as_ref().map_or_else(
        || (0..num_ch).map(|i| format!("Ch{i}")).collect(),
        |m| m.channel_names().iter().map(|s| (*s).to_string()).collect(),
    );

    let constraints: Vec<Constraint> = (0..num_ch)
        .map(|_| Constraint::Ratio(1, u32::try_from(num_ch).unwrap_or(u32::MAX)))
        .collect();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let colors = [
        Color::Cyan,
        Color::Green,
        Color::Yellow,
        Color::Magenta,
        Color::Red,
        Color::Blue,
        Color::LightCyan,
        Color::LightGreen,
        Color::LightYellow,
        Color::LightMagenta,
        Color::LightRed,
        Color::LightBlue,
        Color::White,
        Color::Gray,
    ];

    for (i, buf) in app.eeg_buffers.iter().enumerate() {
        let label = channel_names.get(i).map_or("?", |s| s.as_str());
        let color = colors[i % colors.len()];

        // Sparkline needs u64 data — normalize from f64 microvolts
        // Shift so minimum maps to 0 to avoid negative values
        let data: Vec<u64> = if buf.is_empty() {
            vec![]
        } else {
            let min = buf.iter().copied().fold(f64::INFINITY, f64::min);
            let max = buf.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let range = (max - min).max(1.0);
            buf.iter()
                .map(|&v| {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    {
                        ((v - min) / range * 100.0) as u64
                    }
                })
                .collect()
        };

        let spark = Sparkline::default()
            .block(Block::default().title(format!(" {label} ")))
            .data(&data)
            .style(Style::default().fg(color));

        frame.render_widget(spark, rows[i]);
    }
}

/// Motion data — accelerometer + magnetometer as line charts.
fn draw_motion(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_motion_chart(
        frame,
        " Accelerometer (g) ",
        &app.motion_accel,
        &["X", "Y", "Z"],
        &[Color::Red, Color::Green, Color::Blue],
        chunks[0],
    );
    draw_motion_chart(
        frame,
        " Magnetometer (µT) ",
        &app.motion_mag,
        &["X", "Y", "Z"],
        &[Color::Magenta, Color::Cyan, Color::Yellow],
        chunks[1],
    );
}

/// Render a 3-axis motion chart.
fn draw_motion_chart(
    frame: &mut Frame,
    title: &str,
    data: &std::collections::VecDeque<[f32; 3]>,
    labels: &[&str; 3],
    colors: &[Color; 3],
    area: Rect,
) {
    let block = Block::default().title(title).borders(Borders::ALL);

    if data.is_empty() {
        let msg = Paragraph::new("  Waiting for motion data…")
            .style(Style::default().fg(Color::DarkGray))
            .block(block);
        frame.render_widget(msg, area);
        return;
    }

    // Build dataset points for each axis
    let len = data.len();
    let mut datasets = Vec::new();
    let mut all_points: [Vec<(f64, f64)>; 3] = [Vec::new(), Vec::new(), Vec::new()];

    for (idx, sample) in data.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let x = idx as f64;
        for axis in 0..3 {
            all_points[axis].push((x, f64::from(sample[axis])));
        }
    }

    for axis in 0..3 {
        datasets.push(
            Dataset::default()
                .name(labels[axis])
                .marker(ratatui::symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(colors[axis]))
                .data(&all_points[axis]),
        );
    }

    // Auto-scale Y axis
    let y_min = all_points
        .iter()
        .flatten()
        .map(|(_, y)| *y)
        .fold(f64::INFINITY, f64::min);
    let y_max = all_points
        .iter()
        .flatten()
        .map(|(_, y)| *y)
        .fold(f64::NEG_INFINITY, f64::max);
    let y_margin = ((y_max - y_min) * 0.1).max(0.1);

    #[allow(clippy::cast_precision_loss)]
    let x_max = len as f64;

    let chart = Chart::new(datasets)
        .block(block)
        .x_axis(
            Axis::default()
                .bounds([0.0, x_max])
                .labels::<Vec<Line<'_>>>(vec![]),
        )
        .y_axis(
            Axis::default()
                .bounds([y_min - y_margin, y_max + y_margin])
                .labels(vec![
                    Span::raw(format!("{y_min:.1}")),
                    Span::raw(format!("{y_max:.1}")),
                ]),
        );

    frame.render_widget(chart, area);
}

/// Band power — grouped column chart with channels along the horizontal axis.
fn draw_band_power(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Band Power (θ α βL βH γ) ")
        .borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if app.band_power_buffers.is_empty() {
        let msg = Paragraph::new("  Waiting for band power data…")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(msg, inner);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(4)])
        .split(inner);

    let band_colors = [
        Color::Blue,    // theta
        Color::Green,   // alpha
        Color::Yellow,  // beta low
        Color::Red,     // beta high
        Color::Magenta, // gamma
    ];

    let legend_spans: Vec<Span<'_>> = [
        ("θ Theta", band_colors[0]),
        ("α Alpha", band_colors[1]),
        ("βL BetaL", band_colors[2]),
        ("βH BetaH", band_colors[3]),
        ("γ Gamma", band_colors[4]),
    ]
    .into_iter()
    .flat_map(|(label, color)| {
        [
            Span::styled(" █ ", Style::default().fg(color)),
            Span::styled(label, Style::default().fg(Color::DarkGray)),
        ]
    })
    .collect();
    frame.render_widget(Paragraph::new(Line::from(legend_spans)), chunks[0]);

    let num_ch = app.band_power_buffers.len();
    let channel_names: Vec<String> = app.headset_model.as_ref().map_or_else(
        || (0..num_ch).map(|i| format!("Ch{i}")).collect(),
        |m| m.channel_names().iter().map(|s| (*s).to_string()).collect(),
    );

    let groups: Vec<BarGroup<'_>> = app
        .band_power_buffers
        .iter()
        .enumerate()
        .map(|(i, buf)| {
            let label = channel_names
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("Ch{i}"));
            let latest = buf.back().copied().unwrap_or([0.0; 5]);
            let total: f32 = latest.iter().sum::<f32>().max(0.001);

            let bars: Vec<Bar<'_>> = latest
                .iter()
                .enumerate()
                .map(|(b, &val)| {
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let pct = (val / total * 100.0) as u64;
                    Bar::default()
                        .value(pct)
                        .style(Style::default().fg(band_colors[b]))
                        .value_style(
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        )
                })
                .collect();

            BarGroup::default()
                .label(Line::from(Span::styled(
                    label,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )))
                .bars(&bars)
        })
        .collect();

    let mut chart = BarChart::default()
        .bar_width(3)
        .bar_gap(0)
        .group_gap(3)
        .max(100);

    for group in groups {
        chart = chart.data(group);
    }

    frame.render_widget(chart, chunks[1]);
}
