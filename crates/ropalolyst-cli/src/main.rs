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
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};
use ropalolyst_core::{
    Analyzer, EventKind, GameEvent, LogTailer, RunStore, default_log_path, parse_log_line,
};

type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

struct AppState {
    tailer: Option<LogTailer>,
    log_status: String,
    analyzer: Analyzer,
    run_started: Option<Instant>,
    last_error: Option<String>,
    store: Option<RunStore>,
    store_error: Option<String>,
}

impl AppState {
    fn new() -> Self {
        let path = default_log_path();
        let (tailer, log_status) = match path {
            Some(path) => {
                let display = path.display().to_string();
                if path.exists() {
                    (Some(LogTailer::new_from_end(path)), format!("Watching {display}"))
                } else {
                    (Some(LogTailer::new(path)), format!("Waiting for {display}"))
                }
            }
            None => (None, "LOCALAPPDATA is not defined".to_owned()),
        };
        let store_path = default_log_path().map(|path| path.with_file_name("ropalolyst-analytics.sqlite3"));
        let (store, store_error) = match store_path {
            Some(path) => match RunStore::open(path) {
                Ok(store) => (Some(store), None),
                Err(error) => (None, Some(error.to_string())),
            },
            None => (None, Some("LOCALAPPDATA is not defined".to_owned())),
        };
        Self {
            tailer,
            log_status,
            analyzer: Analyzer::default(),
            run_started: None,
            last_error: None,
            store,
            store_error,
        }
    }

    fn poll(&mut self) {
        let Some(tailer) = self.tailer.as_mut() else { return };
        let path = tailer.path().display().to_string();
        let result = tailer.read_new_lines();
        match result {
            Ok(lines) => {
                for line in lines {
                    if let Some(event) = parse_log_line(&line) {
                        self.handle_event(event);
                    }
                }
                self.last_error = None;
                self.log_status = format!("Reading {path}");
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.log_status = format!("Waiting for {path}");
            }
            Err(error) => {
                self.last_error = Some(error.to_string());
            }
        }
    }

    fn handle_event(&mut self, event: GameEvent) {
        match &event.kind {
            EventKind::MissionStarted => self.run_started = Some(Instant::now()),
            EventKind::MissionSucceeded | EventKind::MissionFailed | EventKind::MissionEnding => {
                self.run_started = None;
            }
            _ => {}
        }
        let completed_before = self.analyzer.completed.len();
        self.analyzer.ingest(event);
        if self.analyzer.completed.len() > completed_before {
            if let (Some(store), Some(session)) = (&self.store, self.analyzer.completed.last()) {
                if let Err(error) = store.save(session) {
                    self.store_error = Some(error.to_string());
                }
            }
        }
    }

    fn elapsed(&self) -> String {
        self.run_started
            .map(|started| format_duration(started.elapsed().as_secs_f64()))
            .unwrap_or_else(|| "00:00.000".to_owned())
    }

    fn encounter(&self) -> (&'static str, Color) {
        let kind = self.analyzer.timeline.iter().rev().find_map(|event| match &event.kind {
            EventKind::BossDefeatedCandidate => Some(("DEFEAT CANDIDATE", Color::Green)),
            EventKind::BossCrashEnded { .. } => Some(("CRASH ANIMATION", Color::Yellow)),
            EventKind::BossCrashStarted { sequence } => Some((match sequence {
                1 => "CRASH 1 / 3",
                2 => "CRASH 2 / 3",
                3 => "CRASH 3 / 3",
                _ => "CRASH",
            }, Color::Yellow)),
            EventKind::BossFightStarted => Some(("BOSS FIGHT", Color::Magenta)),
            EventKind::BossArenaIntroStarted => Some(("ARENA INTRO", Color::Cyan)),
            _ => None,
        });
        kind.unwrap_or(("NOT DETECTED", Color::Gray))
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    let mut terminal = setup_terminal()?;
    let mut app = AppState::new();
    let result = run_app(&mut terminal, &mut app);
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

fn run_app(terminal: &mut AppTerminal, app: &mut AppState) -> Result<()> {
    loop {
        app.poll();
        terminal.draw(|frame| draw(frame, app))?;

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

fn draw(frame: &mut Frame<'_>, app: &AppState) {
    let area = frame.area();
    let outer = Block::default()
        .title(Span::styled(
            " ROPALOLYST ANALYTICS ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(
            Line::from(vec![
                Span::styled(" Q ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" quit  "),
                Span::styled("● ", Style::default().fg(Color::Green)),
                Span::raw(" live log monitor "),
            ])
            .alignment(Alignment::Right),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(5),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(inner);

    draw_header(frame, rows[0], app);
    draw_metrics(frame, rows[1], app);
    draw_main(frame, rows[2], app);
    draw_footer(frame, rows[3], app);
}

fn draw_header(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let (encounter, color) = app.encounter();
    let line = Paragraph::new(Line::from(vec![
        Span::styled("WARFRAME", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
        Span::styled("  /  ", Style::default().fg(Color::DarkGray)),
        Span::styled("JUPITER", Style::default().fg(Color::White)),
        Span::styled("  /  ", Style::default().fg(Color::DarkGray)),
        Span::styled("THE ROPALOLYST", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Span::raw("                         "),
        Span::styled("● ", Style::default().fg(color)),
        Span::styled(encounter, Style::default().fg(color)),
    ]))
    .block(Block::default().borders(Borders::BOTTOM).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(line, area);
}

fn draw_metrics(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(34), Constraint::Percentage(33), Constraint::Percentage(33)])
        .split(area);
    let current = app.analyzer.active.as_ref()
        .map(|session| format!("{:?}", session.status))
        .unwrap_or_else(|| "IDLE".to_owned());
    let (encounter, color) = app.encounter();
    metric(frame, columns[0], "CURRENT RUN", &current, "Mission session", Color::Gray);
    metric(frame, columns[1], "RUN TIMER", &app.elapsed(), "Wall clock since SS_STARTED", Color::Cyan);
    metric(frame, columns[2], "ENCOUNTER", encounter, "Log signature detection", color);
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
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner);
    frame.render_widget(Paragraph::new(value).style(Style::default().fg(color).add_modifier(Modifier::BOLD)), parts[0]);
    frame.render_widget(Paragraph::new(hint).style(Style::default().fg(Color::DarkGray)), parts[1]);
}

fn draw_main(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(area);

    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(columns[0]);

    let phase = app.analyzer.active.as_ref()
        .map(|session| format!("Mission active · {} events", session.events.len()))
        .unwrap_or_else(|| "Waiting for SS_STARTED".to_owned());
    frame.render_widget(
        Paragraph::new(phase).style(Style::default().fg(Color::Gray)).block(panel("MISSION SESSION")),
        left_rows[0],
    );

    let events: Vec<ListItem> = app.analyzer.timeline.iter().rev().take(12).map(|event| {
        let (label, color) = event_label(&event.kind);
        let confidence = match event.confidence {
            ropalolyst_core::EventConfidence::Confirmed => "CONF",
            ropalolyst_core::EventConfidence::StrongCandidate => "CAND",
            ropalolyst_core::EventConfidence::Experimental => "EXPT",
        };
        ListItem::new(Line::from(vec![
            Span::styled(format!("{:>8.3}  ", event.timestamp.0), Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{confidence}  "), Style::default().fg(Color::Yellow)),
            Span::styled(label, Style::default().fg(color)),
        ]))
    }).collect();
    let event_list = if events.is_empty() {
        vec![ListItem::new("No recognized events yet")]
    } else { events };
    frame.render_widget(List::new(event_list).block(panel("LIVE EVENT TIMELINE")), left_rows[1]);

    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(4)])
        .split(columns[1]);

    let status_color = if app.last_error.is_some() { Color::Red } else { Color::Green };
    let status_text = app.last_error.as_deref().unwrap_or(&app.log_status);
    let status = vec![
        Line::from(vec![Span::styled("LOG SOURCE  ", Style::default().fg(Color::DarkGray)), Span::styled("EE.log", Style::default().fg(Color::White))]),
        Line::from(vec![Span::styled("WATCHER     ", Style::default().fg(Color::DarkGray)), Span::styled("Polling", Style::default().fg(Color::Green))]),
        Line::from(vec![Span::styled("FILE        ", Style::default().fg(Color::DarkGray)), Span::styled(status_text, Style::default().fg(status_color))]),
        Line::from(vec![Span::styled("LAST EVENT  ", Style::default().fg(Color::DarkGray)), Span::styled(app.analyzer.last_event.as_ref().map(|e| format!("{:?}", e.kind)).unwrap_or_else(|| "—".to_owned()), Style::default().fg(Color::Gray))]),
    ];
    frame.render_widget(Paragraph::new(status).block(panel("PARSER STATUS")), right_rows[0]);

    let best = app.store.as_ref().and_then(|store| store.best_success_seconds().ok().flatten())
        .or_else(|| app.analyzer.best_success_seconds()).map(format_duration).unwrap_or_else(|| "—".to_owned());
    let average = app.store.as_ref().and_then(|store| store.average_success_seconds().ok().flatten())
        .or_else(|| app.analyzer.average_success_seconds()).map(format_duration).unwrap_or_else(|| "—".to_owned());
    let saved_count = app.store.as_ref().and_then(|store| store.completed_count().ok()).unwrap_or(0);
    let history = Paragraph::new(vec![
        Line::from(Span::styled(format!("Saved runs      {saved_count}"), Style::default().fg(Color::White))),
        Line::from(Span::styled(format!("Best time       {best}"), Style::default().fg(Color::Green))),
        Line::from(Span::styled(format!("Average time    {average}"), Style::default().fg(Color::Cyan))),
        Line::from(Span::styled(
            app.store_error.as_deref().unwrap_or("SQLite history ready"),
            Style::default().fg(if app.store_error.is_some() { Color::Red } else { Color::DarkGray }),
        )),
    ]).block(panel("RUN STATISTICS"));
    frame.render_widget(history, right_rows[1]);
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let status = if app.last_error.is_some() { "LOG READ ERROR" } else { "CORE ONLINE" };
    let color = if app.last_error.is_some() { Color::Red } else { Color::Green };
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("● ", Style::default().fg(color)),
        Span::raw(status),
        Span::styled("   │   ", Style::default().fg(Color::DarkGray)),
        Span::raw("AUTO-DISCOVER EE.log"),
        Span::styled("   │   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit"),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(footer, area);
}

fn event_label(kind: &EventKind) -> (&'static str, Color) {
    match kind {
        EventKind::MissionStarted => ("Mission started", Color::Green),
        EventKind::MissionSucceeded => ("Mission succeeded", Color::Green),
        EventKind::MissionFailed => ("Mission failed", Color::Red),
        EventKind::MissionEnding => ("Mission ending / abort candidate", Color::Yellow),
        EventKind::BossArenaIntroStarted => ("Boss arena intro", Color::Cyan),
        EventKind::BossArenaIntroEnded => ("Boss arena intro ended", Color::Cyan),
        EventKind::BossFightStarted => ("Boss fight started", Color::Magenta),
        EventKind::BossCrashStarted { sequence } => (match sequence {
            1 => "Crash sequence 1 started",
            2 => "Crash sequence 2 started",
            3 => "Crash sequence 3 started",
            _ => "Crash sequence started",
        }, Color::Yellow),
        EventKind::BossCrashEnded { .. } => ("Crash animation ended", Color::Yellow),
        EventKind::BossDefeatedCandidate => ("Boss defeated candidate", Color::Green),
        EventKind::Unknown => ("Unknown", Color::Gray),
    }
}

fn panel(title: &'static str) -> Block<'static> {
    Block::default()
        .title(Span::styled(format!(" {title} "), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
}

fn format_duration(seconds: f64) -> String {
    let total_ms = (seconds.max(0.0) * 1000.0) as u64;
    let minutes = total_ms / 60_000;
    let secs = (total_ms / 1000) % 60;
    let millis = total_ms % 1000;
    format!("{minutes:02}:{secs:02}.{millis:03}")
}
