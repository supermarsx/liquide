//! Terminal-based interactive dashboard using ratatui.

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Dataset, List, ListItem, Paragraph},
};
use std::io;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::collector::TelemetryCollector;
use crate::types::{HealthStatus, TelemetrySnapshot};

/// Run the TUI dashboard.
pub async fn run_tui(refresh_ms: u64, remote: Option<String>) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create collector
    let collector = if let Some(addr) = remote {
        TelemetryCollector::remote(addr)
    } else {
        TelemetryCollector::local()
    };

    // Channel for telemetry updates
    let (tx, mut rx) = mpsc::channel::<TelemetrySnapshot>(100);

    // Spawn collection task
    let collector_handle = tokio::spawn(async move {
        let _ = collector
            .collect_continuous(refresh_ms, move |snapshot| {
                tx.blocking_send(snapshot).is_ok()
            })
            .await;
    });

    // Current snapshot
    let mut snapshot = TelemetrySnapshot::default();
    let mut last_update = Instant::now();

    // Main loop
    let result = loop {
        // Draw UI
        terminal.draw(|f| draw_ui(f, &snapshot, last_update.elapsed()))?;

        // Handle events
        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') || key.code == KeyCode::Esc {
                    break Ok(());
                }
            }
        }

        // Update snapshot
        if let Ok(new_snapshot) = rx.try_recv() {
            snapshot = new_snapshot;
            last_update = Instant::now();
        }
    };

    // Cleanup
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    collector_handle.abort();

    result
}

/// Draw the TUI interface.
fn draw_ui(f: &mut Frame, snapshot: &TelemetrySnapshot, age: std::time::Duration) {
    let size = f.area();

    // Main layout: header, body, footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(size);

    // Header
    draw_header(f, chunks[0], snapshot, age);

    // Body: left panel (metrics) + right panel (graph)
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    draw_metrics_panel(f, body_chunks[0], snapshot);
    draw_graph_panel(f, body_chunks[1], snapshot);

    // Footer
    draw_footer(f, chunks[2]);
}

/// Draw the header with status and health.
fn draw_header(f: &mut Frame, area: Rect, snapshot: &TelemetrySnapshot, age: std::time::Duration) {
    let health_color = match snapshot.health {
        HealthStatus::Healthy => Color::Green,
        HealthStatus::Degraded => Color::Yellow,
        HealthStatus::Slow => Color::LightRed,
        HealthStatus::Critical => Color::Red,
    };

    let header = Paragraph::new(vec![
        Line::from(vec![Span::styled(
            "Liquide Telemetry Dashboard",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::raw("Status: "),
            Span::styled(
                format!("{:?}", snapshot.health),
                Style::default()
                    .fg(health_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" | FPS: {:.1}", snapshot.frames.fps)),
            Span::raw(format!(" | Age: {:.1}s", age.as_secs_f64())),
        ]),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White)),
    );

    f.render_widget(header, area);
}

/// Draw the metrics panel (left side).
fn draw_metrics_panel(f: &mut Frame, area: Rect, snapshot: &TelemetrySnapshot) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Min(5),
        ])
        .split(area);

    // Frame metrics
    draw_frame_metrics(f, chunks[0], snapshot);

    // Thread metrics
    draw_thread_metrics(f, chunks[1], snapshot);

    // Window list
    draw_window_list(f, chunks[2], snapshot);
}

/// Draw frame metrics block.
fn draw_frame_metrics(f: &mut Frame, area: Rect, snapshot: &TelemetrySnapshot) {
    let metrics = &snapshot.frames;

    let items = vec![
        ListItem::new(format!("FPS: {:.1}", metrics.fps)),
        ListItem::new(format!("Avg: {:.2}ms", metrics.avg_frame_time)),
        ListItem::new(format!("Min: {:.2}ms", metrics.min_frame_time)),
        ListItem::new(format!("Max: {:.2}ms", metrics.max_frame_time)),
        ListItem::new(format!("P95: {:.2}ms", metrics.p95_frame_time)),
        ListItem::new(format!("P99: {:.2}ms", metrics.p99_frame_time)),
    ];

    let list = List::new(items).block(
        Block::default()
            .title("Frame Metrics")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    f.render_widget(list, area);
}

/// Draw thread pool metrics.
fn draw_thread_metrics(f: &mut Frame, area: Rect, snapshot: &TelemetrySnapshot) {
    let threads = &snapshot.threads;

    let items = vec![
        ListItem::new(format!("Active: {}", threads.active_threads)),
        ListItem::new(format!("Idle: {}", threads.idle_threads)),
        ListItem::new(format!("Queue: {:.1}", threads.avg_queue_depth)),
        ListItem::new(format!("Tasks/s: {}", threads.tasks_per_second)),
    ];

    let list = List::new(items).block(
        Block::default()
            .title("Thread Pool")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    f.render_widget(list, area);
}

/// Draw window list.
fn draw_window_list(f: &mut Frame, area: Rect, snapshot: &TelemetrySnapshot) {
    let items: Vec<ListItem> = snapshot
        .windows
        .iter()
        .map(|(id, metrics)| {
            let indicator = if metrics.interactive { "* " } else { "  " };
            ListItem::new(format!(
                "{}Win {}: {:.2}ms ({} nodes)",
                indicator, id, metrics.avg_render_time, metrics.node_count
            ))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title("Windows")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green)),
    );

    f.render_widget(list, area);
}

/// Draw the graph panel (right side).
fn draw_graph_panel(f: &mut Frame, area: Rect, snapshot: &TelemetrySnapshot) {
    // Convert history to chart data
    let data: Vec<(f64, f64)> = snapshot
        .frames
        .history
        .iter()
        .enumerate()
        .map(|(i, &time)| (i as f64, time))
        .collect();

    let dataset = Dataset::default()
        .name("Frame Time")
        .marker(symbols::Marker::Braille)
        .style(Style::default().fg(Color::Cyan))
        .data(&data);

    let x_max = data.len().max(1) as f64;
    let y_max = snapshot
        .frames
        .history
        .iter()
        .cloned()
        .fold(0.0f64, f64::max)
        .max(20.0);

    let chart = Chart::new(vec![dataset])
        .block(
            Block::default()
                .title("Frame Time History")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::White)),
        )
        .x_axis(
            Axis::default()
                .title("Frame")
                .bounds([0.0, x_max])
                .labels(vec![
                    Span::raw("0"),
                    Span::raw(format!("{:.0}", x_max / 2.0)),
                    Span::raw(format!("{:.0}", x_max)),
                ]),
        )
        .y_axis(
            Axis::default()
                .title("Time (ms)")
                .bounds([0.0, y_max])
                .labels(vec![
                    Span::raw("0"),
                    Span::raw(format!("{:.1}", y_max / 2.0)),
                    Span::raw(format!("{:.1}", y_max)),
                ]),
        );

    f.render_widget(chart, area);
}

/// Draw the footer with controls.
fn draw_footer(f: &mut Frame, area: Rect) {
    let footer = Paragraph::new("Press 'q' or ESC to quit | Arrow keys: navigate")
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(footer, area);
}
