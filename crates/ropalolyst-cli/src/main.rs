use std::{
    io::{self, Stdout},
    time::Duration,
};

mod overlay;
use overlay::OverlayController;

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
    Analyzer, EventKind, GameEvent, LogTailer, MissionStatus, RunStore, TransmissionKind, average_phase_splits, default_log_path, parse_clock_anchor, parse_log_line, phase_splits,
};

type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

struct AppState {
    tailer: Option<LogTailer>,
    log_status: String,
    analyzer: Analyzer,
    last_error: Option<String>,
    store: Option<RunStore>,
    store_error: Option<String>,
    clock_anchor: Option<ropalolyst_core::LogClockAnchor>,
    overlay: OverlayController,
    initial_scan: bool,
}

impl AppState {
    fn new() -> Self {
        let path = default_log_path();
        let (tailer, log_status) = match path {
            Some(path) => {
                let display = path.display().to_string();
                if path.exists() {
                    (Some(LogTailer::new(path)), format!("Watching {display}"))
                } else {
                    (Some(LogTailer::new(path)), format!("Waiting for {display}"))
                }
            }
            None => (None, "未找到 LOCALAPPDATA 环境变量".to_owned()),
        };
        let store_path = default_log_path().map(|path| path.with_file_name("ropalolyst-analytics.sqlite3"));
        let (store, store_error) = match store_path {
            Some(path) => match RunStore::open(path) {
                Ok(store) => (Some(store), None),
                Err(error) => (None, Some(error.to_string())),
            },
            None => (None, Some("未找到 LOCALAPPDATA 环境变量".to_owned())),
        };
        Self {
            tailer,
            log_status,
            analyzer: Analyzer::default(),
            last_error: None,
            store,
            store_error,
            clock_anchor: None,
            overlay: OverlayController::new(),
            initial_scan: true,
        }
    }

    fn poll(&mut self) {
        let Some(tailer) = self.tailer.as_mut() else { return };
        let path = tailer.path().display().to_string();
        let result = tailer.read_new_lines();
        match result {
            Ok(lines) => {
                for line in lines {
                    if let Some(anchor) = parse_clock_anchor(&line) {
                        self.clock_anchor = Some(anchor);
                    }
                    if let Some(event) = parse_log_line(&line) {
                        self.handle_event(event);
                    }
                }
                self.last_error = None;
                self.log_status = format!("Reading {path}");
                if self.initial_scan && !lines.is_empty() {
                    if self.analyzer.active.is_none() {
                        self.overlay.hide();
                    }
                    self.initial_scan = false;
                }
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
        let kind = event.kind.clone();
        let started_at_utc = self.clock_anchor
            .and_then(|anchor| anchor.to_utc(event.timestamp))
            .unwrap_or(event.observed_at);
        let completed_before = self.analyzer.completed.len();

        if kind == EventKind::MissionStarted {
            // Map the game's relative log clock to UTC, then run the overlay from a monotonic clock.
            self.overlay.start(started_at_utc);
        }

        self.analyzer.ingest(event);

        if self.analyzer.completed.len() > completed_before {
            let generation = self.tailer.as_ref()
                .map(LogTailer::generation_id)
                .unwrap_or_else(|| "unknown".to_owned());
            if let (Some(store), Some(session)) = (&self.store, self.analyzer.completed.last()) {
                if let Err(error) = store.save(session, &generation) {
                    self.store_error = Some(error.to_string());
                }
            }
            if let Some(session) = self.analyzer.completed.last() {
                match session.status {
                    MissionStatus::Succeeded => {
                        if let Some(seconds) = session.duration_seconds() {
                            self.overlay.finish(Duration::from_secs_f64(seconds.max(0.0)));
                        }
                    }
                    MissionStatus::Failed | MissionStatus::Aborted => self.overlay.hide(),
                    MissionStatus::InProgress => {}
                }
            }
        }
    }
    fn elapsed(&self) -> String {
        self.analyzer.active.as_ref()
            .and_then(|session| self.analyzer.last_event.as_ref().map(|event| event.timestamp.0 - session.started_at.0))
            .or_else(|| self.analyzer.completed.last().and_then(|session| session.duration_seconds()))
            .map(format_duration)
            .unwrap_or_else(|| "00:00.000".to_owned())
    }

    fn encounter(&self) -> (String, Color) {
        let kind = self.analyzer.timeline.iter().rev().find_map(|event| match &event.kind {
            EventKind::BossTransmission { kind: TransmissionKind::LaserKill } =>
                Some(("激光处决结果（候选）".to_owned(), Color::Green)),
            EventKind::BossCrashCinematicStopped { tower_id } => Some((
                format!("电塔 {} 撞击动画结束", tower_id),
                Color::Yellow,
            )),
            EventKind::BossTowerResetStarted { cinematic_id } =>
                Some((format!("电塔复位动画开始（实例 {cinematic_id}）"), Color::Yellow)),
            EventKind::BossTowerResetEnded { cinematic_id } =>
                Some((format!("电塔复位动画结束（实例 {cinematic_id}）"), Color::Yellow)),
            EventKind::BossCrashStarted { tower_id, cycle } => Some((
                format!("第 {cycle} 轮撞塔 · 电塔 {tower_id}"),
                Color::Yellow,
            )),
            EventKind::BossLandingStarted { .. } =>
                Some(("Boss 击杀后落地动画".to_owned(), Color::Green)),
            EventKind::BossLandingEnded { .. } =>
                Some(("Boss 落地动画结束".to_owned(), Color::Green)),
            EventKind::BossTransmission { kind } =>
                Some((transmission_label(*kind).to_owned(), Color::Cyan)),
            EventKind::BossArenaIntroStarted =>
                Some(("Boss 登场动画".to_owned(), Color::Cyan)),
            EventKind::BossArenaIntroEnded =>
                Some(("Boss 登场动画结束".to_owned(), Color::Cyan)),
            _ => None,
        });
        kind.unwrap_or_else(|| ("尚未检测到".to_owned(), Color::Gray))
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
            " 蝠力使竞速分析 ",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ))
        .title_bottom(
            Line::from(vec![
                Span::styled(" Q ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" 退出  "),
                Span::styled("● ", Style::default().fg(Color::Green)),
                Span::raw(" 实时日志监控 "),
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
        Span::styled("Warframe", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
        Span::styled("  /  ", Style::default().fg(Color::DarkGray)),
        Span::styled("木星", Style::default().fg(Color::White)),
        Span::styled("  /  ", Style::default().fg(Color::DarkGray)),
        Span::styled("蝠力使", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
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
        .map(|session| mission_status_label(session.status))
        .unwrap_or("等待任务");
    let (encounter, color) = app.encounter();
    metric(frame, columns[0], "当前任务", current, "任务状态", Color::Gray);
    metric(frame, columns[1], "任务计时", &app.elapsed(), "基于游戏日志时间戳", Color::Cyan);
    metric(frame, columns[2], "Boss 阶段", &encounter, "日志事件识别", color);
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

    let current_session = app.analyzer.active.as_ref()
        .or_else(|| app.analyzer.completed.last());
    let splits = current_session.map(phase_splits).unwrap_or_default();
    let history_sessions = app.store.as_ref()
        .and_then(|store| store.successful_sessions().ok())
        .unwrap_or_else(|| app.analyzer.completed.clone());
    let averages = average_phase_splits(&history_sessions);
    let mut split_rows = vec![ListItem::new(Line::from(vec![
        Span::styled(pad_right_display("阶段", 14), Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Span::styled(pad_left_display("累计时间", 12), Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Span::styled(pad_left_display("阶段耗时", 12), Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
        Span::styled(pad_left_display("较平均", 10), Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    ]))];
    for split in &splits {
        let average = averages.iter().find(|(key, _, _)| key == &split.key);
        let (delta_text, delta_color) = match average {
            Some((_, average_seconds, _)) => {
                let delta = split.split_seconds - average_seconds;
                (format!("{delta:+.3}s"), if delta <= 0.0 { Color::Green } else { Color::Red })
            }
            None => ("—".to_owned(), Color::DarkGray),
        };
        split_rows.push(ListItem::new(Line::from(vec![
            Span::styled(pad_right_display(&split.label, 14), Style::default().fg(Color::White)),
            Span::styled(pad_left_display(&format_duration(split.cumulative_seconds), 12), Style::default().fg(Color::Cyan)),
            Span::styled(pad_left_display(&format_duration(split.split_seconds), 12), Style::default().fg(Color::Yellow)),
            Span::styled(pad_left_display(&delta_text, 10), Style::default().fg(delta_color)),
        ])));
    }
    if splits.is_empty() {
        split_rows.push(ListItem::new("等待可识别的阶段标记…"));
    }
    frame.render_widget(
        List::new(split_rows).block(panel("竞速分段 · 累计 / 阶段耗时 / 较历史平均")),
        columns[0],
    );

    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(4)])
        .split(columns[1]);

    let status_color = if app.last_error.is_some() { Color::Red } else { Color::Green };
    let status_text = app.last_error.as_deref().unwrap_or(&app.log_status);
    let status = vec![
        Line::from(vec![Span::styled("日志文件  ", Style::default().fg(Color::DarkGray)), Span::styled("EE.log", Style::default().fg(Color::White))]),
        Line::from(vec![Span::styled("监控状态     ", Style::default().fg(Color::DarkGray)), Span::styled("正在监控", Style::default().fg(Color::Green))]),
        Line::from(vec![Span::styled("文件状态        ", Style::default().fg(Color::DarkGray)), Span::styled(status_text, Style::default().fg(status_color))]),
        Line::from(vec![Span::styled("最近事件  ", Style::default().fg(Color::DarkGray)), Span::styled(app.analyzer.last_event.as_ref().map(|e| format!("{:?}", e.kind)).unwrap_or_else(|| "—".to_owned()), Style::default().fg(Color::Gray))]),
    ];
    frame.render_widget(Paragraph::new(status).block(panel("日志解析状态")), right_rows[0]);

    let best = app.store.as_ref().and_then(|store| store.best_success_seconds().ok().flatten())
        .or_else(|| app.analyzer.best_success_seconds()).map(format_duration).unwrap_or_else(|| "—".to_owned());
    let average = app.store.as_ref().and_then(|store| store.average_success_seconds().ok().flatten())
        .or_else(|| app.analyzer.average_success_seconds()).map(format_duration).unwrap_or_else(|| "—".to_owned());
    let saved_count = app.store.as_ref().and_then(|store| store.completed_count().ok()).unwrap_or(0);
    let history = Paragraph::new(vec![
        Line::from(Span::styled(format!("已保存成绩      {saved_count}"), Style::default().fg(Color::White))),
        Line::from(Span::styled(format!("最佳成绩       {best}"), Style::default().fg(Color::Green))),
        Line::from(Span::styled(format!("平均成绩    {average}"), Style::default().fg(Color::Cyan))),
        Line::from(Span::styled(
            app.store_error.as_deref().unwrap_or("本地成绩库已就绪"),
            Style::default().fg(if app.store_error.is_some() { Color::Red } else { Color::DarkGray }),
        )),
    ]).block(panel("成绩统计"));
    frame.render_widget(history, right_rows[1]);
}

fn draw_footer(frame: &mut Frame<'_>, area: Rect, app: &AppState) {
    let status = if app.last_error.is_some() { "日志读取错误" } else { "核心运行中" };
    let color = if app.last_error.is_some() { Color::Red } else { Color::Green };
    let footer = Paragraph::new(Line::from(vec![
        Span::styled("● ", Style::default().fg(color)),
        Span::raw(status),
        Span::styled("   │   ", Style::default().fg(Color::DarkGray)),
        Span::raw("自动定位 EE.log"),
        Span::styled("   │   ", Style::default().fg(Color::DarkGray)),
        Span::styled("Q", Style::default().fg(Color::Yellow)),
        Span::raw(" 退出"),
    ]))
    .alignment(Alignment::Center)
    .block(Block::default().borders(Borders::TOP).border_style(Style::default().fg(Color::DarkGray)));
    frame.render_widget(footer, area);
}


fn mission_status_label(status: ropalolyst_core::MissionStatus) -> &'static str {
    match status {
        ropalolyst_core::MissionStatus::InProgress => "进行中",
        ropalolyst_core::MissionStatus::Succeeded => "成功",
        ropalolyst_core::MissionStatus::Failed => "失败",
        ropalolyst_core::MissionStatus::Aborted => "已中止",
    }
}

fn transmission_label(kind: TransmissionKind) -> &'static str {
    match kind {
        TransmissionKind::ShieldHint => "护盾提示（不代表破盾）",
        TransmissionKind::FirstShieldDown => "首次破盾信号",
        TransmissionKind::ConduitChargeReminder => "电塔充能提醒（非完成）",
        TransmissionKind::WeakpointHint => "弱点提示（非永久计数）",
        TransmissionKind::FinalWeakpointHint => "最后弱点提示",
        TransmissionKind::FinalMountAttempt => "最后一轮骑乘尝试",
        TransmissionKind::StunHint => "Boss 眩晕提示",
        TransmissionKind::LaserHit => "平台激光命中结果",
        TransmissionKind::LaserMiss => "平台激光未命中结果",
        TransmissionKind::LaserKill => "平台激光处决结果（候选）",
    }
}

fn panel(title: &'static str) -> Block<'static> {
    Block::default()
        .title(Span::styled(format!(" {title} "), Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
}


/// Ratatui string formatting counts Unicode scalar values, not terminal cells.
/// These labels are Chinese/ASCII only, so a two-cell width for non-ASCII chars
/// keeps all four columns visually aligned in a terminal.
fn display_width(value: &str) -> usize {
    value.chars().map(|ch| if ch.is_ascii() { 1 } else { 2 }).sum()
}

fn pad_right_display(value: &str, width: usize) -> String {
    format!("{value}{}", " ".repeat(width.saturating_sub(display_width(value))))
}

fn pad_left_display(value: &str, width: usize) -> String {
    format!("{}{}", " ".repeat(width.saturating_sub(display_width(value))), value)
}

fn format_duration(seconds: f64) -> String {
    let total_ms = (seconds.max(0.0) * 1000.0) as u64;
    let minutes = total_ms / 60_000;
    let secs = (total_ms / 1000) % 60;
    let millis = total_ms % 1000;
    format!("{minutes:02}:{secs:02}.{millis:03}")
}
