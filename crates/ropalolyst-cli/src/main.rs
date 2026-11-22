use std::{
    io::{self, Stdout},
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline},
};

type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let mut terminal = setup_terminal()?;
    let result = run_app(&mut terminal);
    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> Result<AppTerminal> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;
    terminal.clear()?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut AppTerminal) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn run_app(terminal: &mut AppTerminal) -> Result<()> {
    let started = Instant::now();
    loop {
        terminal.draw(|frame| draw(frame, started.elapsed()))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press
                    && matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn draw(frame: &mut Frame<'_>, uptime: Duration) {
    let area = frame.area();
    let outer = Block::default()
        .title(Span::styled(
            " ROPALOLYST ANALYTICS ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(Line::from(vec![
            Span::styled(" Q ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" quit  "),
            Span::styled("● ", Style::default().fg(Color::Green)),
            Span::raw(" console ready "),
        ]).alignment(Alignment::Right))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(inner);

    draw_header(frame, rows[0]);
    draw_metrics(frame, rows[1], uptime);
    draw_main(frame, rows[2]);
    draw_footer(frame, rows[3]);
}

fn draw_header(frame: &mut Frame<'_>, area: Rect) {
    let line = Paragraph::new(Line::from(vec![
        Span::styled("WARFRAME", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
        Span::styled("  /  ", Style::default().fg(Color::DarkGray)),
        Span::styled("SOL NODE 740", Style::default().fg(Color::White)),
        Span::styled("  /  ", Style::default().fg(Color::DarkGray)),
        Span::styled("THE ROPALOLYST", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Span::raw("                                      "),
        Span::styled("● ", Style::default().fg(Color::Yellow)),
        Span::styled("WAITING FOR GAME LOG", Style::default().fg(Color::Yellow)),
    ]))
    .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(line, area);
}

fn draw_metrics(frame: &mut Frame<'_>, area: Rect, uptime: Duration) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(33), Constraint::Percentage(33)])
        .split(area);

    metric(frame, columns[0], "CURRENT RUN", "—", "No active mission", Color::Gray);
    metric(frame, columns[1], "ELAPSED", &format_duration(uptime), "Timer starts at SS_STARTED", Color::Cyan);
    metric(frame, columns[2], "ENCOUNTER", "STANDBY", "Ropalolyst not detected", Color::Yellow);
}

fn metric(frame: &mut Frame<'_>, area: Rect, title: &str, value: &str, hint: &str, color: Color) {
    let block = Block::default()
        .title(Span::styled(format!(" {title} "), Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);
    frame.render_widget(
        Paragraph::new(value).style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
        parts[0],
    );
    frame.render_widget(
        Paragraph::new(hint).style(Style::default().fg(Color::DarkGray)),
        parts[1],
    );
}

fn draw_main(frame: &mut Frame<'_>, area: Rect) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);

    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(7)])
        .split(columns[0]);

    let phase = Paragraph::new("Awaiting mission start")
        .style(Style::default().fg(Color::Gray))
        .block(panel("ENCOUNTER PROGRESS"));
    frame.render_widget(phase, left_rows[0]);

    let events = vec![
        ListItem::new(Line::from(vec![
            Span::styled("  ○  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Mission started", Style::default().fg(Color::Gray)),
            Span::raw("                         "),
            Span::styled("--:--.---", Style::default().fg(Color::DarkGray)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("  ○  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Boss arena intro", Style::default().fg(Color::Gray)),
            Span::raw("                       "),
            Span::styled("--:--.---", Style::default().fg(Color::DarkGray)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("  ○  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Boss fight started", Style::default().fg(Color::Gray)),
            Span::raw("                     "),
            Span::styled("--:--.---", Style::default().fg(Color::DarkGray)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("  ○  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Crash sequence 1 / 3", Style::default().fg(Color::Gray)),
            Span::raw("                   "),
            Span::styled("--:--.---", Style::default().fg(Color::DarkGray)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("  ○  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Crash sequence 2 / 3", Style::default().fg(Color::Gray)),
            Span::raw("                   "),
            Span::styled("--:--.---", Style::default().fg(Color::DarkGray)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("  ○  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Crash sequence 3 / 3", Style::default().fg(Color::Gray)),
            Span::raw("                   "),
            Span::styled("--:--.---", Style::default().fg(Color::DarkGray)),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled("  ○  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Boss defeated", Style::default().fg(Color::Gray)),
            Span::raw("                            "),
            Span::styled("--:--.---", Style::default().fg(Color::DarkGray)),
        ])),
    ];
    frame.render_widget(
        List::new(events).block(panel("RUN TIMELINE  ·  VERIFIED EVENTS ONLY")),
        left_rows[1],
    );

    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(5)])
        .split(columns[1]);

    let status = vec![
        Line::from(vec![Span::styled("LOG SOURCE  ", Style::default().fg(Color::DarkGray)), Span::styled("Not configured", Style::default().fg(Color::Yellow))]),
        Line::from(vec![Span::styled("FILE WATCH   ", Style::default().fg(Color::DarkGray)), Span::styled("Inactive", Style::default().fg(Color::Gray))]),
        Line::from(vec![Span::styled("MISSION      ", Style::default().fg(Color::DarkGray)), Span::styled("—", Style::default().fg(Color::Gray))]),
        Line::from(vec![Span::styled("LAST EVENT   ", Style::default().fg(Color::DarkGray)), Span::styled("—", Style::default().fg(Color::Gray))]),
    ];
    frame.render_widget(Paragraph::new(status).block(panel("PARSER STATUS")), right_rows[0]);

    let chart = Sparkline::default()
        .block(panel("RECENT RUN PACE"))
        .data(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
        .max(10)
        .style(Style::default().fg(Color::DarkGray))
        .bar_set(symbols::bar::NINE_LEVELS);
    frame.render_widget(chart, right_rows[1]);

    let _ = Gauge::default()
        .block(panel("PHASE COMPLETION"))
        .gauge_style(Style::default().fg(Color::Cyan))
        .ratio(0.0);
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect) {
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("● ", Style::default().fg(Color::Green)),
        Span::raw("CORE ONLINE"),
        Span::styled("   │   ", Style::default().fg(Color::DarkGray)),
        Span::styled("LOG PARSER", Style::default().fg(Color::DarkGray)),
        Span::raw("  not connected"),
        Span::styled("   │   ", Style::default().fg(Color::DarkGray)),
        Span::styled("F1", Style::default().fg(Color::Yellow)),
        Span::raw(" help"),
        Span::styled("   │   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit"),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(footer, area);
}

fn panel(title: &'static str) -> Block<'static> {
    Block::default()
        .title(Span::styled(format!(" {title} "), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
}

fn format_duration(duration: Duration) -> String {
    let total_ms = duration.as_millis();
    let minutes = total_ms / 60_000;
    let seconds = (total_ms / 1_000) % 60;
    let millis = total_ms % 1_000;
    format!("{minutes:02}:{seconds:02}.{millis:03}")
}
