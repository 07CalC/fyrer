use std::{collections::HashMap, time::Duration};

use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
};
use tokio::sync::{broadcast, mpsc};

use fyrer_core::{TaskId, status::TaskStatus};
use fyrer_engine::events::{EngineCommand, EngineEvent, LogStream, RunSummary};

use crate::reporter::Reporter;

type DefaultBackend = CrosstermBackend<std::io::Stdout>;

trait DurationHumanReadable {
    fn to_human_readable(&self) -> String;
}
impl DurationHumanReadable for std::time::Duration {
    fn to_human_readable(&self) -> String {
        if self.as_secs() >= 3600 {
            format!(
                "{}h {}m {:.1}s",
                self.as_secs() / 3600,
                (self.as_secs() % 3600) / 60,
                self.as_secs_f64() % 60.0
            )
        } else if self.as_secs() >= 60 {
            format!("{}m {:.1}s", self.as_secs() / 60, self.as_secs_f64() % 60.0)
        } else if self.as_millis() >= 1000 {
            format!("{:.2}s", self.as_secs_f64())
        } else if self.as_micros() >= 1000 {
            format!("{:.2}ms", self.as_micros() as f64 / 1000.0)
        } else {
            format!("{}μs", self.as_micros())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TuiTaskStatus {
    Waiting,
    Running,
    Complete,
    Failed { exit_code: i32 },
    CacheHit,
    Skipped,
    Restarting,
}

impl TuiTaskStatus {
    fn symbol(&self) -> &'static str {
        match self {
            Self::Waiting => "○",
            Self::Running => "●",
            Self::Complete => "✓",
            Self::Failed { .. } => "✗",
            Self::CacheHit => "⚡",
            Self::Skipped => "—",
            Self::Restarting => "↻",
        }
    }

    fn color(&self) -> Color {
        match self {
            Self::Waiting => Color::DarkGray,
            Self::Running => Color::Rgb(80, 220, 120),
            Self::Complete => Color::Rgb(80, 200, 200),
            Self::Failed { .. } => Color::Rgb(230, 80, 80),
            Self::CacheHit => Color::Rgb(180, 120, 255),
            Self::Skipped => Color::Rgb(100, 100, 100),
            Self::Restarting => Color::Rgb(240, 190, 60),
        }
    }
}

#[derive(Debug)]
enum TuiMode {
    Running,
    Summary(RunSummary),
    PostRun,
}

struct LogCache {
    text: Text<'static>,
    parsed_len: usize,
    viewport_width: usize,
    wrapped_height: usize,
}

pub struct Tui;

impl Tui {
    pub fn new() -> Self {
        Self
    }
}

impl Reporter for Tui {
    fn start(self, rx: broadcast::Receiver<EngineEvent>) -> tokio::task::JoinHandle<Result<()>> {
        self.start_with_control(rx, None)
    }

    /// Full-featured entry: pass the engine command channel to enable the
    /// `r` (restart selected task) and `K` (kill selected task) keybinds and
    /// graceful shutdown on quit.
    fn start_with_control(
        self,
        rx: broadcast::Receiver<EngineEvent>,
        cmd_tx: Option<mpsc::Sender<EngineCommand>>,
    ) -> tokio::task::JoinHandle<Result<()>> {
        tokio::task::spawn_blocking(move || TuiWorker::new(cmd_tx)?.run(rx))
    }
}

struct TuiWorker {
    terminal: Terminal<DefaultBackend>,
    task_order: Vec<TaskId>,
    statuses: HashMap<TaskId, TuiTaskStatus>,
    logs: HashMap<TaskId, Vec<String>>,
    cache: HashMap<TaskId, LogCache>,
    list_state: ListState,
    scroll_positions: HashMap<usize, usize>,
    following: HashMap<usize, bool>,
    viewport_height: usize,
    mode: TuiMode,
    cmd_tx: Option<mpsc::Sender<EngineCommand>>,
    dirty: bool,
    engine_closed: bool,
}

impl TuiWorker {
    fn new(cmd_tx: Option<mpsc::Sender<EngineCommand>>) -> Result<Self> {
        let terminal = ratatui::try_init()?;
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
        Ok(Self {
            terminal,
            task_order: Vec::new(),
            statuses: HashMap::new(),
            logs: HashMap::new(),
            cache: HashMap::new(),
            list_state: ListState::default().with_selected(Some(0)),
            scroll_positions: HashMap::new(),
            following: HashMap::new(),
            viewport_height: 0,
            mode: TuiMode::Running,
            cmd_tx,
            dirty: true,
            engine_closed: false,
        })
    }

    fn run(mut self, mut rx: broadcast::Receiver<EngineEvent>) -> Result<()> {
        let result = self.event_loop(&mut rx);
        self.shutdown();
        result
    }

    fn shutdown(&mut self) {
        let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
        ratatui::restore();
    }

    /// Ask the engine to shut down (kills all process groups). Best-effort.
    fn request_shutdown(&mut self) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.try_send(EngineCommand::Shutdown);
        }
    }

    fn restart_selected(&mut self) -> Result<()> {
        let Some(id) = self.task_order.get(self.list_state.selected().unwrap_or(0)).cloned()
        else {
            return Ok(());
        };
        match &self.cmd_tx {
            Some(tx) => {
                let _ = tx.try_send(EngineCommand::Restart(vec![id.clone()]));
                self.push_system_log(&id, "↻ restart requested".to_string());
            }
            None => {
                self.push_system_log(&id, "↻ restart unavailable (no control channel)".to_string());
            }
        }
        self.dirty = true;
        Ok(())
    }

    fn kill_selected(&mut self) -> Result<()> {
        let Some(id) = self.task_order.get(self.list_state.selected().unwrap_or(0)).cloned()
        else {
            return Ok(());
        };
        match &self.cmd_tx {
            Some(tx) => {
                let _ = tx.try_send(EngineCommand::Kill(vec![id.clone()]));
                self.push_system_log(&id, "⏻ kill requested".to_string());
            }
            None => {
                self.push_system_log(&id, "⏻ kill unavailable (no control channel)".to_string());
            }
        }
        self.dirty = true;
        Ok(())
    }

    fn event_loop(&mut self, rx: &mut broadcast::Receiver<EngineEvent>) -> Result<()> {
        loop {
            // 1. Drain all pending engine events (non-blocking).
            loop {
                match rx.try_recv() {
                    Ok(ev) => {
                        if self.handle_event(ev)? {
                            return Ok(());
                        }
                    }
                    Err(broadcast::error::TryRecvError::Empty) => break,
                    Err(broadcast::error::TryRecvError::Lagged(n)) => {
                        let msg =
                            format!("\x1b[33m⚠ fyrer: {n} event(s) were dropped\x1b[0m");
                        for logs in self.logs.values_mut() {
                            logs.push(msg.clone());
                        }
                        self.cache.clear();
                        self.dirty = true;
                    }
                    Err(broadcast::error::TryRecvError::Closed) => {
                        // All engine senders dropped. If we never saw RunFinished
                        // (e.g. engine died unexpectedly), fall through to browse mode.
                        if !self.engine_closed {
                            self.engine_closed = true;
                            if matches!(self.mode, TuiMode::Running) {
                                self.mode = TuiMode::PostRun;
                                self.dirty = true;
                            }
                        }
                        break;
                    }
                }
            }

            // 2. Poll keyboard / mouse with a short timeout so we stay responsive
            //    even while the engine is quiet.
            while crossterm::event::poll(Duration::from_millis(20))? {
                match crossterm::event::read()? {
                    Event::Key(key) => {
                        // Ignore Release/Repeat to avoid double-firing on some terminals
                        if key.kind == KeyEventKind::Press
                            && self.handle_key(key)?
                        {
                            return Ok(());
                        }
                    }
                    Event::Mouse(mouse) => match mouse.kind {
                        MouseEventKind::ScrollUp => self.scroll_up_lines(3),
                        MouseEventKind::ScrollDown => self.scroll_down_lines(3),
                        _ => continue,
                    },
                    crossterm::event::Event::Resize(_, _) => {}
                    _ => continue,
                }
                self.dirty = true;
            }

            // 3. Render when something changed.
            if self.dirty {
                self.render()?;
                self.dirty = false;
            }
        }
    }

    fn handle_event(&mut self, event: EngineEvent) -> Result<bool> {
        match event {
            EngineEvent::RunStarted { planned, .. } => {
                for id in planned {
                    self.register_task(id, TuiTaskStatus::Waiting);
                }
                self.dirty = true;
            }
            EngineEvent::TaskReady(_) => {}

            EngineEvent::TaskStarted { id, .. } => {
                // A (re)start means work is happening again — leave any
                // summary/post-run view, drop the ↻ restarting marker and go
                // back to the live ● running state.
                if !matches!(self.mode, TuiMode::Running) {
                    self.mode = TuiMode::Running;
                }
                self.register_task(id.clone(), TuiTaskStatus::Running);
                self.set_status(&id, TuiTaskStatus::Running);
                self.dirty = true;
            }

            EngineEvent::TaskLog { key, line, stream } => {
                self.push_log(&key.task, line, stream);
                self.dirty = true;
            }

            EngineEvent::TaskFinished {
                id,
                outcome,
                final_status,
            } => match final_status {
                TaskStatus::Succeeded { .. } => {
                    self.set_status(&id, TuiTaskStatus::Complete);
                }
                TaskStatus::Cached { .. } => {
                    self.set_status(&id, TuiTaskStatus::CacheHit);
                }
                TaskStatus::Failed { .. } => {
                    self.set_status(
                        &id,
                        TuiTaskStatus::Failed {
                            exit_code: outcome.exit_code,
                        },
                    );
                }
                TaskStatus::Skipped { reason } => {
                    let why = match reason {
                        fyrer_core::status::SkipReason::UpstreamFailed => "upstream failed",
                        fyrer_core::status::SkipReason::UpstreamSkipped => "upstream skipped",
                    };
                    self.set_status(&id, TuiTaskStatus::Skipped);
                    self.push_system_log(&id, format!("— skipped ({why})"));
                }
                _ => {}
            },

            EngineEvent::TaskCacheHit { id } => {
                self.register_task(id.clone(), TuiTaskStatus::CacheHit);
                self.push_system_log(&id, "⚡ cache hit — outputs restored".to_string());
                self.dirty = true;
            }
            EngineEvent::TaskSkipped { id, reason } => {
                let why = match reason {
                    fyrer_core::status::SkipReason::UpstreamFailed => "upstream failed",
                    fyrer_core::status::SkipReason::UpstreamSkipped => "upstream skipped",
                };
                self.register_task(id.clone(), TuiTaskStatus::Skipped);
                self.push_system_log(&id, format!("— skipped ({why})"));
                self.dirty = true;
            }
            EngineEvent::FilesChanged { id, paths } => {
                // Show which files triggered the restart (cap the list).
                let names: Vec<String> =
                    paths.iter().take(3).map(|p| short_name(p)).collect();
                let mut msg = match names.len() {
                    0 => "input files changed".to_string(),
                    _ => format!("changed: {}", names.join(", ")),
                };
                if paths.len() > 3 {
                    msg.push_str(&format!(" +{} more", paths.len() - 3));
                }
                self.set_status(&id, TuiTaskStatus::Restarting);
                self.push_system_log(&id, format!("✎ {msg}"));
                self.dirty = true;
            }

            EngineEvent::TaskRestarting { id, killed_attempt } => {
                if !matches!(self.mode, TuiMode::Running) {
                    self.mode = TuiMode::Running;
                }
                self.set_status(&id, TuiTaskStatus::Restarting);
                self.push_system_log(&id, format!("↻ attempt {} killed — restarting", killed_attempt));
                self.dirty = true;
            }
            EngineEvent::DependentsStale { ids } => {
                for id in ids {
                    self.push_system_log(&id, "◌ stale (dependency restarted)".to_string());
                }
                self.dirty = true;
            }

            // All tasks terminal — show the summary popup right away. This is
            // what the user sees while the engine parks in interactive mode;
            // RunFinished only arrives later, when the engine actually exits.
            EngineEvent::RunCompleted(summary) => {
                if !matches!(self.mode, TuiMode::PostRun) {
                    self.mode = TuiMode::Summary(summary);
                    self.dirty = true;
                }
            }

            EngineEvent::RunFinished(summary) => {
                // Engine exited. Only surface this if we never showed
                // RunCompleted (e.g. mid-run shutdown); never clobber a
                // summary/post-run view the user is already looking at.
                if matches!(self.mode, TuiMode::Running) {
                    self.mode = TuiMode::Summary(summary);
                    self.dirty = true;
                }
            }

            EngineEvent::NonFatalError { task_id, error } => {
                if let Some(id) = task_id {
                    self.push_system_log(&id, format!("✗ {error}"));
                }
                self.dirty = true;
            }
            EngineEvent::Warning { task_id, message } => {
                if let Some(id) = task_id {
                    self.push_system_log(&id, format!("⚠ {message}"));
                }
                self.dirty = true;
            }
        }
        Ok(false)
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.request_shutdown();
                return Ok(true);
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.request_shutdown();
                return Ok(true);
            }

            KeyCode::Enter => {
                if let TuiMode::Summary(_) = &self.mode {
                    self.mode = TuiMode::PostRun;
                    self.dirty = true;
                }
            }
            KeyCode::Esc => {}

            KeyCode::Char('j') | KeyCode::Down => {
                self.navigate_next();
                self.dirty = true;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.navigate_previous();
                self.dirty = true;
            }

            KeyCode::Char('u') | KeyCode::PageUp => {
                self.scroll_up_page();
                self.dirty = true;
            }
            KeyCode::Char('d') | KeyCode::PageDown => {
                self.scroll_down_page();
                self.dirty = true;
            }

            KeyCode::Char('G') => {
                self.go_to_bottom();
                self.dirty = true;
            }
            KeyCode::Char('g') => {
                self.go_to_top();
                self.dirty = true;
            }

            KeyCode::Char('r') => {
                self.restart_selected()?;
            }
            KeyCode::Char('K') => {
                self.kill_selected()?;
            }

            _ => {}
        }
        Ok(false)
    }

    fn register_task(&mut self, task_id: TaskId, status: TuiTaskStatus) {
        if !self.statuses.contains_key(&task_id) {
            let idx = self.task_order.len();
            self.task_order.push(task_id.clone());
            self.scroll_positions.entry(idx).or_insert(0);
            self.following.insert(idx, true);
        }
        self.statuses.insert(task_id, status);
    }

    fn set_status(&mut self, task_id: &TaskId, status: TuiTaskStatus) {
        if let Some(s) = self.statuses.get_mut(task_id) {
            *s = status;
        } else {
            self.register_task(task_id.clone(), status);
        }
        self.dirty = true;
    }

    fn push_log(&mut self, task_id: &TaskId, line: String, stream: LogStream) {
        let formatted = match stream {
            LogStream::Stdout => line,
            LogStream::Stderr => format!("\x1b[31m{line}\x1b[0m"),
            LogStream::System => return,
        };
        self.logs
            .entry(task_id.clone())
            .or_default()
            .push(formatted);
        self.cache.remove(task_id);
    }

    fn push_system_log(&mut self, task_id: &TaskId, msg: String) {
        let formatted = format!("\x1b[90m{msg}\x1b[0m");
        self.logs
            .entry(task_id.clone())
            .or_default()
            .push(formatted);
        self.cache.remove(task_id);
    }

    fn navigate_next(&mut self) {
        let len = self.task_order.len();
        if len == 0 {
            return;
        }
        let idx = self.list_state.selected().unwrap_or(0);
        let next = (idx + 1) % len;
        self.list_state.select(Some(next));
        self.following.insert(next, true);
    }

    fn navigate_previous(&mut self) {
        let len = self.task_order.len();
        if len == 0 {
            return;
        }
        let idx = self.list_state.selected().unwrap_or(0);
        let prev = if idx == 0 { len - 1 } else { idx - 1 };
        self.list_state.select(Some(prev));
        self.following.insert(prev, true);
    }

    fn scroll_up_page(&mut self) {
        let page = self.viewport_height.saturating_sub(2).max(1);
        self.scroll_up_lines(page);
    }

    fn scroll_down_page(&mut self) {
        let page = self.viewport_height.saturating_sub(2).max(1);
        self.scroll_down_lines(page);
    }

    fn scroll_up_lines(&mut self, n: usize) {
        let idx = self.list_state.selected().unwrap_or(0);
        let pos = self.scroll_positions.entry(idx).or_insert(0);
        *pos = pos.saturating_sub(n);
        self.following.insert(idx, false);
    }

    fn scroll_down_lines(&mut self, n: usize) {
        let idx = self.list_state.selected().unwrap_or(0);
        let pos = self.scroll_positions.entry(idx).or_insert(0);
        *pos = pos.saturating_add(n);
    }

    fn go_to_top(&mut self) {
        let idx = self.list_state.selected().unwrap_or(0);
        self.scroll_positions.insert(idx, 0);
        self.following.insert(idx, false);
    }

    fn go_to_bottom(&mut self) {
        let idx = self.list_state.selected().unwrap_or(0);
        self.following.insert(idx, true);
    }

    fn get_or_parse(
        cache: &mut HashMap<TaskId, LogCache>,
        logs: &HashMap<TaskId, Vec<String>>,
        task_id: &TaskId,
        viewport_width: usize,
    ) -> (Text<'static>, usize) {
        let lines = logs.get(task_id).map_or(&[][..], Vec::as_slice);
        let current_len = lines.len();

        if let Some(c) = cache.get(task_id) {
            if c.parsed_len == current_len && c.viewport_width == viewport_width {
                return (c.text.clone(), c.wrapped_height);
            }
        }

        let joined = lines.join("\n");
        let mut text = joined
            .as_str()
            .into_text()
            .unwrap_or_else(|_| Text::raw(joined.clone()));

        Self::strip_reset_bg(&mut text);

        let wrapped_height = Self::wrapped_line_count(&text, viewport_width);

        cache.insert(
            task_id.clone(),
            LogCache {
                text: text.clone(),
                parsed_len: current_len,
                viewport_width,
                wrapped_height,
            },
        );
        (text, wrapped_height)
    }

    fn strip_reset_bg(text: &mut Text<'_>) {
        for line in &mut text.lines {
            if line.style.bg == Some(Color::Reset) {
                line.style.bg = None;
            }
            for span in &mut line.spans {
                if span.style.bg == Some(Color::Reset) {
                    span.style.bg = None;
                }
            }
        }
    }

    fn wrapped_line_count(text: &Text<'_>, viewport_width: usize) -> usize {
        if viewport_width == 0 {
            return 0;
        }
        text.lines
            .iter()
            .map(|line| {
                let w = line.width();
                if w == 0 {
                    1
                } else {
                    w.div_ceil(viewport_width)
                }
            })
            .sum()
    }

    fn render(&mut self) -> Result<()> {
        let task_order = self.task_order.clone();
        let statuses = self.statuses.clone();

        let size = self.terminal.size().unwrap_or_default();
        let left_width: u16 = 28;
        let log_width = usize::from(size.width.saturating_sub(left_width + 2)).max(1);
        let log_height = usize::from(size.height.saturating_sub(4)).max(1);

        self.viewport_height = log_height;

        let selected_idx = self.list_state.selected().unwrap_or(0);
        let selected_task_id = task_order.get(selected_idx).cloned();

        let (log_text, total_wrapped) = if let Some(ref tid) = selected_task_id {
            Self::get_or_parse(&mut self.cache, &self.logs, tid, log_width)
        } else {
            (Text::default(), 0)
        };

        let max_offset = total_wrapped.saturating_sub(log_height);
        let pos = self.scroll_positions.entry(selected_idx).or_insert(0);
        let following = *self.following.entry(selected_idx).or_insert(true);
        if following {
            *pos = max_offset;
        } else {
            *pos = (*pos).min(max_offset);
            if *pos >= max_offset {
                self.following.insert(selected_idx, true);
            }
        }
        let scroll_pos = *pos;

        let summary_snapshot: Option<RunSummary> = match &self.mode {
            TuiMode::Summary(s) => Some(s.clone()),
            _ => None,
        };
        let is_post_run = matches!(self.mode, TuiMode::PostRun);
        let has_control = self.cmd_tx.is_some();

        let mut list_state = self.list_state.clone();

        self.terminal
            .draw(|f| {
                let area = f.area();

                let [main_area, footer_area] =
                    Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);

                let [left_area, right_area] =
                    Layout::horizontal([Constraint::Length(left_width), Constraint::Min(0)])
                        .areas(main_area);

                const BG: Color = Color::Rgb(10, 10, 14);
                const PANEL_BG: Color = Color::Rgb(16, 16, 22);
                const BORDER: Color = Color::Rgb(45, 45, 65);
                const ACCENT: Color = Color::Rgb(100, 160, 255);
                const DIM: Color = Color::Rgb(70, 70, 90);
                const SEP: Color = Color::Rgb(40, 40, 55);

                let items: Vec<ListItem> = task_order
                    .iter()
                    .map(|id| {
                        let status = statuses.get(id).unwrap_or(&TuiTaskStatus::Waiting);
                        let sym = status.symbol();
                        let col = status.color();
                        let name = id.to_string();
                        let display_name = if name.len() > 19 && name.is_char_boundary(19) {
                            format!("{}…", &name[..19])
                        } else if name.chars().count() > 20 {
                            let truncated: String = name.chars().take(19).collect();
                            format!("{truncated}…")
                        } else {
                            name
                        };
                        ListItem::new(Line::from(vec![
                            Span::styled(
                                format!("{sym} "),
                                Style::default().fg(col).add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(display_name, Style::default().fg(Color::White)),
                        ]))
                    })
                    .collect();

                let list = List::new(items)
                    .block(
                        Block::bordered()
                            .title(Span::styled(
                                " Tasks ",
                                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                            ))
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(BORDER))
                            .bg(PANEL_BG),
                    )
                    .highlight_style(
                        Style::default()
                            .bg(Color::Rgb(30, 45, 80))
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    )
                    .highlight_symbol("▶ ");
                f.render_stateful_widget(list, left_area, &mut list_state);

                let log_title = if let Some(ref id) = selected_task_id {
                    let follow_indicator = if following {
                        Span::styled(
                            " ↓follow",
                            Style::default()
                                .fg(Color::Rgb(80, 220, 120))
                                .add_modifier(Modifier::ITALIC),
                        )
                    } else {
                        Span::styled(
                            " scroll",
                            Style::default()
                                .fg(Color::Rgb(200, 150, 60))
                                .add_modifier(Modifier::ITALIC),
                        )
                    };
                    Line::from(vec![
                        Span::styled(
                            format!(" Logs: {id} "),
                            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                        ),
                        follow_indicator,
                    ])
                } else {
                    Line::from(Span::styled(
                        " Logs ",
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ))
                };

                let paragraph = Paragraph::new(log_text.clone())
                    .style(Style::default().bg(BG))
                    .block(
                        Block::bordered()
                            .title(log_title)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(BORDER))
                            .bg(BG),
                    )
                    .scroll((u16::try_from(scroll_pos).unwrap_or(u16::MAX), 0))
                    .wrap(Wrap { trim: false });
                f.render_widget(paragraph, right_area);

                let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None)
                    .thumb_style(Style::default().fg(ACCENT))
                    .track_style(Style::default().fg(DIM));
                let mut scrollbar_state = ScrollbarState::new(total_wrapped)
                    .position(scroll_pos)
                    .viewport_content_length(log_height);
                f.render_stateful_widget(scrollbar, right_area, &mut scrollbar_state);

                let footer_spans: Vec<Span> = if is_post_run {
                    vec![
                        bold_span(" q", Color::White),
                        dim_span(" quit", DIM),
                        sep_span(SEP),
                        bold_span(" j/k", Color::White),
                        dim_span(" nav", DIM),
                        sep_span(SEP),
                        bold_span(" u/d", Color::White),
                        dim_span(" page", DIM),
                        sep_span(SEP),
                        bold_span(" G", Color::White),
                        dim_span(" tail  ", DIM),
                        Span::styled(
                            "post-run browse",
                            Style::default()
                                .fg(Color::Rgb(200, 150, 60))
                                .add_modifier(Modifier::ITALIC),
                        ),
                    ]
                } else if has_control {
                    vec![
                        bold_span(" q", Color::White),
                        dim_span(" quit", DIM),
                        sep_span(SEP),
                        bold_span(" j/↓ k/↑", Color::White),
                        dim_span(" select", DIM),
                        sep_span(SEP),
                        bold_span(" u/d", Color::White),
                        dim_span(" page", DIM),
                        sep_span(SEP),
                        bold_span(" g/G", Color::White),
                        dim_span(" top/tail", DIM),
                        sep_span(SEP),
                        bold_span(" r", Color::Rgb(240, 190, 60)),
                        dim_span(" restart", DIM),
                        sep_span(SEP),
                        bold_span(" K", Color::Rgb(230, 80, 80)),
                        dim_span(" kill", DIM),
                        sep_span(SEP),
                        bold_span(" scroll", Color::White),
                        dim_span(" mouse", DIM),
                    ]
                } else {
                    vec![
                        bold_span(" q", Color::White),
                        dim_span(" quit", DIM),
                        sep_span(SEP),
                        bold_span(" j/↓", Color::White),
                        dim_span(" next", DIM),
                        sep_span(SEP),
                        bold_span(" k/↑", Color::White),
                        dim_span(" prev", DIM),
                        sep_span(SEP),
                        bold_span(" u/d", Color::White),
                        dim_span(" page scroll", DIM),
                        sep_span(SEP),
                        bold_span(" G", Color::White),
                        dim_span(" tail", DIM),
                        sep_span(SEP),
                        bold_span(" scroll", Color::White),
                        dim_span(" mouse", DIM),
                    ]
                };
                let footer =
                    Paragraph::new(Line::from(footer_spans)).style(Style::default().bg(PANEL_BG));
                f.render_widget(footer, footer_area);

                if let Some(ref summary) = summary_snapshot {
                    render_summary_popup(f, area, summary);
                }
            })
            .map_err(|e| anyhow::anyhow!("Failed to draw terminal: {e}"))?;

        Ok(())
    }
}

fn bold_span(text: &'static str, color: Color) -> Span<'static> {
    Span::styled(text, Style::default().fg(color).add_modifier(Modifier::BOLD))
}

/// Last path segment for compact display in log lines.
fn short_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn dim_span(text: &'static str, color: Color) -> Span<'static> {
    Span::styled(text, Style::default().fg(color))
}

fn sep_span(color: Color) -> Span<'static> {
    Span::styled(" │ ", Style::default().fg(color))
}

fn render_summary_popup(f: &mut ratatui::Frame, area: ratatui::layout::Rect, summary: &RunSummary) {
    let popup_width: u16 = 54;
    let popup_height: u16 = 14;
    let x = area.x + area.width.saturating_sub(popup_width) / 2;
    let y = area.y + area.height.saturating_sub(popup_height) / 2;
    let popup_area = ratatui::layout::Rect {
        x,
        y,
        width: popup_width.min(area.width),
        height: popup_height.min(area.height),
    };

    const BG: Color = Color::Rgb(18, 18, 28);
    const ACCENT: Color = Color::Rgb(100, 160, 255);
    const BORDER: Color = Color::Rgb(80, 100, 180);

    let all_ok = summary.failed == 0;
    let header_color = if all_ok {
        Color::Rgb(80, 220, 120)
    } else {
        Color::Rgb(230, 80, 80)
    };
    let header_icon = if all_ok { "✓" } else { "✗" };
    let header_msg = if all_ok {
        "Run complete"
    } else {
        "Run finished with failures"
    };

    let duration_str = summary.duration.to_human_readable();

    let lines = vec![
        Line::from(vec![
            Span::styled(
                format!("  {header_icon} "),
                Style::default()
                    .fg(header_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                header_msg,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  + ", Style::default().fg(Color::Rgb(80, 220, 120))),
            Span::styled(
                format!("Successful  {:>3}", summary.successful),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  x ", Style::default().fg(Color::Rgb(230, 80, 80))),
            Span::styled(
                format!("Failed      {:>3}", summary.failed),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  * ", Style::default().fg(Color::Rgb(180, 120, 255))),
            Span::styled(
                format!("Cached      {:>3}", summary.cached),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  - ", Style::default().fg(Color::Rgb(150, 150, 170))),
            Span::styled(
                format!("Skipped     {:>3}", summary.skipped),
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(
                format!("Total       {:>3}", summary.total),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Duration    ",
                Style::default().fg(Color::Rgb(150, 150, 170)),
            ),
            Span::styled(duration_str, Style::default().fg(ACCENT)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  [Enter]",
                Style::default()
                    .fg(Color::Rgb(80, 220, 120))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" logs    ", Style::default().fg(Color::Rgb(150, 150, 170))),
            Span::styled(
                "[j/k]",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(" tasks    ", Style::default().fg(Color::Rgb(150, 150, 170))),
            Span::styled(
                "[q]",
                Style::default()
                    .fg(Color::Rgb(230, 80, 80))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" quit", Style::default().fg(Color::Rgb(150, 150, 170))),
        ]),
    ];

    f.render_widget(Clear, popup_area);

    let popup = Paragraph::new(Text::from(lines))
        .block(
            Block::bordered()
                .title(Span::styled(
                    " Run Summary ",
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ))
                .title_alignment(Alignment::Center)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(BORDER))
                .bg(BG),
        )
        .style(Style::default().bg(BG));
    f.render_widget(popup, popup_area);
}
