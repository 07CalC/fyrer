use std::collections::HashMap;

use ansi_to_tui::IntoText;
use anyhow::Result;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
};

use crate::events::LogStream;
use crate::tasks::TaskId;

use super::tasks::TaskStatus;

/// Generic interface implemented by every user-interface backend that renders
/// the orchestrator's state. This keeps the TUI fully decoupled from the
/// orchestrator, making it possible to swap in alternative backends (e.g. a
/// plain, non-interactive output) without touching orchestration logic.
pub trait Ui {
    /// Pushes a new log line for the given task. Each backend stores or
    /// displays the line however it sees fit.
    fn push_log(&mut self, task_id: &TaskId, line: String, stream: LogStream);

    /// Redraws the current task snapshot. Called by the orchestrator after
    /// every event so the UI always reflects the latest state.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend fails to draw to the terminal.
    fn render(&mut self, tasks: &[(TaskId, TaskStatus)]) -> Result<()>;

    /// Moves the selection highlight to the next item, if applicable.
    fn navigate_next(&mut self) {}

    /// Moves the selection highlight to the previous item, if applicable.
    fn navigate_previous(&mut self) {}

    /// Cleans up terminal state when the run ends.
    ///
    /// # Errors
    ///
    /// Returns an error if the terminal cannot be restored.
    fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    fn scroll_logs_up(&mut self) {}
    fn scroll_logs_down(&mut self) {}
    fn scroll_logs_up_by(&mut self, _n: usize) {}
    fn scroll_logs_down_by(&mut self, _n: usize) {}
}

type DefaultBackend = CrosstermBackend<std::io::Stdout>;

/// Cached ANSI-parsed text for a single task's logs, rebuilt only when new
/// lines arrive or the viewport width changes.
struct LogCache {
    /// The parsed ratatui [`Text`], ready to render.
    text: Text<'static>,
    /// Number of raw lines that were parsed to produce `text`.
    parsed_len: usize,
    /// Viewport width used when computing `wrapped_height`.
    viewport_width: usize,
    /// Total visual (wrapped) row count.
    wrapped_height: usize,
}

/// A full-screen, interactive terminal UI built on ratatui.
pub struct Tui {
    terminal: Terminal<DefaultBackend>,
    list_state: ListState,
    /// Raw log lines per task, pushed via [`Ui::push_log`].
    logs: HashMap<TaskId, Vec<String>>,
    /// Cached parsed text per task; invalidated when new lines arrive.
    cache: HashMap<TaskId, LogCache>,
    /// Scroll offset from the top of the selected task's log, per task index.
    positions: HashMap<usize, usize>,
    /// Whether the log view follows the newest output, per task index.
    following: HashMap<usize, bool>,
    /// Height of the log viewport in lines, updated on every draw.
    viewport: usize,
}

impl Tui {
    /// Initialises the terminal for the interactive UI.
    ///
    /// # Errors
    ///
    /// Returns an error if the terminal cannot be put into raw mode.
    pub fn new() -> Result<Self> {
        let terminal = ratatui::try_init()?;
        crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture)?;
        Ok(Self {
            terminal,
            list_state: ListState::default().with_selected(Some(0)),
            logs: HashMap::new(),
            cache: HashMap::new(),
            positions: HashMap::new(),
            following: HashMap::new(),
            viewport: 0,
        })
    }
}

impl Ui for Tui {
    fn push_log(&mut self, task_id: &TaskId, line: String, stream: LogStream) {
        let formatted = match stream {
            LogStream::Stdout => line,
            LogStream::Stderr => format!("\x1b[31m⚠ {line}\x1b[0m"),
        };
        self.logs
            .entry(task_id.clone())
            .or_default()
            .push(formatted);
    }

    fn render(&mut self, tasks: &[(TaskId, TaskStatus)]) -> Result<()> {
        for (idx, _) in tasks.iter().enumerate() {
            self.positions.entry(idx).or_insert(0);
            self.following.entry(idx).or_insert(true);
        }
        let selected_idx = self.list_state.selected().unwrap_or(0);
        let selected_task_id = tasks.get(selected_idx).map(|(id, _)| id);
        let snapshot: Vec<(String, TaskStatus)> = tasks
            .iter()
            .map(|(id, status)| (id.to_string(), status.clone()))
            .collect();
        self.render_layout(&snapshot, selected_task_id)?;
        Ok(())
    }

    fn navigate_next(&mut self) {
        self.list_state.select_next();
        self.follow_to_tail();
    }

    fn navigate_previous(&mut self) {
        self.list_state.select_previous();
        self.follow_to_tail();
    }

    fn shutdown(&mut self) -> Result<()> {
        crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture)?;
        ratatui::restore();
        Ok(())
    }

    fn scroll_logs_up(&mut self) {
        let page = self.viewport.saturating_sub(2).max(1);
        self.scroll_up_by(page);
    }

    fn scroll_logs_down(&mut self) {
        let page = self.viewport.saturating_sub(2).max(1);
        self.scroll_down_by(page);
    }

    fn scroll_logs_up_by(&mut self, n: usize) {
        self.scroll_up_by(n);
    }

    fn scroll_logs_down_by(&mut self, n: usize) {
        self.scroll_down_by(n);
    }
}

impl Tui {
    fn scroll_up_by(&mut self, n: usize) {
        let idx = self.list_state.selected().unwrap_or(0);
        let pos = self.positions.entry(idx).or_insert(0);
        *pos = pos.saturating_sub(n);
        self.following.insert(idx, false);
    }

    fn scroll_down_by(&mut self, n: usize) {
        let idx = self.list_state.selected().unwrap_or(0);
        let pos = self.positions.entry(idx).or_insert(0);
        *pos = pos.saturating_add(n);
    }

    fn follow_to_tail(&mut self) {
        let idx = self.list_state.selected().unwrap_or(0);
        self.following.insert(idx, true);
    }

    /// Compute the total number of visual (wrapped) rows a set of log lines
    /// will occupy when rendered into a viewport of the given width.
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

    /// Returns the cached parsed [`Text`] and its wrapped height for the given
    /// task, re-parsing only when new lines have been pushed or the viewport
    /// width has changed.
    fn get_or_parse(
        cache: &mut HashMap<TaskId, LogCache>,
        logs: &HashMap<TaskId, Vec<String>>,
        task_id: &TaskId,
        viewport_width: usize,
    ) -> (Text<'static>, usize) {
        let lines = logs.get(task_id).map_or(&[][..], Vec::as_slice);
        let current_len = lines.len();

        if let Some(cached) = cache.get(task_id)
            && cached.parsed_len == current_len
            && cached.viewport_width == viewport_width
        {
            return (cached.text.clone(), cached.wrapped_height);
        }

        // Re-parse: join all lines and parse as a single ANSI stream so that
        // colour state carries across line boundaries.
        let joined = lines.join("\n");
        let mut text = joined
            .as_str()
            .into_text()
            .unwrap_or_else(|_| Text::raw(joined));

        // Strip `Color::Reset` backgrounds so spans inherit the paragraph's
        // dark background instead of punching through to the terminal default.
        // Explicit ANSI background colours are left untouched.
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

    /// Replaces `Color::Reset` backgrounds with `None` so the span inherits
    /// the parent widget's background style. Specific colours set by ANSI
    /// escape codes are kept as-is.
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

    /// Draws the task list and the selected task's logs.
    ///
    /// # Errors
    ///
    /// Returns an error if the terminal fails to draw.
    #[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
    pub fn render_layout(
        &mut self,
        tasks: &[(String, TaskStatus)],
        selected_task_id: Option<&TaskId>,
    ) -> Result<()> {
        // Pre-compute the parsed text outside the draw closure so we don't
        // need to borrow `self` inside the closure (which also borrows
        // `self.terminal`).
        let viewport_width_estimate = self
            .terminal
            .size()
            .map_or(80, |s| usize::from(s.width.saturating_sub(26)));

        let (text, total_wrapped) = if let Some(tid) = selected_task_id {
            Self::get_or_parse(&mut self.cache, &self.logs, tid, viewport_width_estimate)
        } else {
            (Text::default(), 0)
        };

        let selected_idx = self.list_state.selected().unwrap_or(0);

        // Clamp / follow scroll position.
        let max_offset = total_wrapped.saturating_sub(self.viewport.max(1));
        let pos = self.positions.entry(selected_idx).or_insert(0);
        if *self.following.entry(selected_idx).or_insert(true) {
            *pos = max_offset;
        } else {
            *pos = (*pos).min(max_offset);
            if *pos >= max_offset {
                self.following.insert(selected_idx, true);
            }
        }
        let scroll_pos = *pos;

        self.terminal
            .draw(|f| {
                // ── Top-level layout: main area + 1-row keybinds bar ──
                let [main_area, footer_area] =
                    Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(f.area());

                let [left, right] =
                    Layout::horizontal([Constraint::Length(25), Constraint::Min(0)])
                        .areas(main_area);

                // ── Task list with coloured status symbols ──
                let items: Vec<ListItem> = tasks
                    .iter()
                    .map(|(name, status)| {
                        let (symbol, color) = match status {
                            TaskStatus::Waiting => ("○", Color::DarkGray),
                            TaskStatus::Running => ("●", Color::Green),
                            TaskStatus::Complete => ("✓", Color::Cyan),
                            TaskStatus::Failed { .. } => ("✗", Color::Red),
                            TaskStatus::Restarting => ("↻", Color::Yellow),
                        };
                        ListItem::new(Line::from(vec![
                            Span::styled(format!("{symbol} "), Style::default().fg(color)),
                            Span::raw(name.clone()),
                        ]))
                    })
                    .collect();

                let list = List::new(items)
                    .block(Block::bordered().title("Tasks"))
                    .highlight_style(Style::default().bg(Color::White).fg(Color::Black))
                    .highlight_symbol("> ");
                f.render_stateful_widget(list, left, &mut self.list_state);

                // ── Log pane ──
                let [log_area, scrollbar_area] =
                    Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).areas(right);

                let log_bg = Color::Rgb(15, 15, 20);

                // Update viewport height for future scroll calculations.
                self.viewport = usize::from(log_area.height);

                // Fill background to clear stale characters.
                // f.render_widget(
                //     Block::default()
                //         .style(Style::default().bg(log_bg))
                //         .borders(Borders::default()),
                //     right,
                // );

                let paragraph = Paragraph::new(text.clone())
                    .style(Style::default().bg(log_bg))
                    .block(Block::bordered().title("Logs").bg(log_bg))
                    .scroll((u16::try_from(scroll_pos).unwrap_or(u16::MAX), 0))
                    .wrap(Wrap { trim: false });
                f.render_widget(paragraph, log_area);

                let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None);
                let mut scrollbar_state = ScrollbarState::new(total_wrapped)
                    .position(scroll_pos)
                    .viewport_content_length(self.viewport);
                f.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);

                // ── Keybinds footer ──
                let keybinds = Line::from(vec![
                    Span::styled(
                        " q",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" quit", Style::default().fg(Color::DarkGray)),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 60))),
                    Span::styled(
                        "j/↓",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" next", Style::default().fg(Color::DarkGray)),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 60))),
                    Span::styled(
                        "k/↑",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" prev", Style::default().fg(Color::DarkGray)),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 60))),
                    Span::styled(
                        "u",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" pg up", Style::default().fg(Color::DarkGray)),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 60))),
                    Span::styled(
                        "d",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" pg dn", Style::default().fg(Color::DarkGray)),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 60))),
                    Span::styled(
                        "scroll",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" mouse", Style::default().fg(Color::DarkGray)),
                ]);
                let footer =
                    Paragraph::new(keybinds).style(Style::default().bg(Color::Rgb(25, 25, 30)));
                f.render_widget(footer, footer_area);
            })
            .map_err(|e| anyhow::anyhow!("Failed to draw terminal: {e}"))?;
        Ok(())
    }
}

/// A minimal, non-interactive backend that prints each task's output to
/// stdout as it arrives. Used when the interactive TUI is disabled.
#[derive(Default)]
pub struct PlainUi;

impl Ui for PlainUi {
    fn push_log(&mut self, task_id: &TaskId, line: String, stream: LogStream) {
        match stream {
            LogStream::Stdout => println!("[{task_id}] {line}"),
            LogStream::Stderr => println!("[{task_id}] \x1b[31m⚠ {line}\x1b[0m"),
        }
    }

    fn render(&mut self, _tasks: &[(TaskId, TaskStatus)]) -> Result<()> {
        Ok(())
    }
}
