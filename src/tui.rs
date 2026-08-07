use std::collections::HashMap;

use ansi_to_tui::IntoText;
use anyhow::Result;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
};

use crate::tasks::TaskId;

use super::tasks::TaskStatus;

/// Generic interface implemented by every user-interface backend that renders
/// the orchestrator's state. This keeps the TUI fully decoupled from the
/// orchestrator, making it possible to swap in alternative backends (e.g. a
/// plain, non-interactive output) without touching orchestration logic.
pub trait Ui {
    /// Redraws the current task snapshot. Called by the orchestrator after
    /// every event so the UI always reflects the latest state.
    ///
    /// # Errors
    ///
    /// Returns an error if the backend fails to draw to the terminal.
    fn render(
        &mut self,
        tasks: &[(TaskId, TaskStatus)],
        logs: &HashMap<TaskId, Vec<String>>,
    ) -> Result<()>;

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

/// A full-screen, interactive terminal UI built on ratatui.
pub struct Tui {
    terminal: Terminal<DefaultBackend>,
    list_state: ListState,
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
        crossterm::execute!(
            std::io::stdout(),
            crossterm::event::EnableMouseCapture
        )?;
        Ok(Self {
            terminal,
            list_state: ListState::default().with_selected(Some(0)),
            positions: HashMap::new(),
            following: HashMap::new(),
            viewport: 0,
        })
    }
}

impl Ui for Tui {
    fn render(
        &mut self,
        tasks: &[(TaskId, TaskStatus)],
        logs: &HashMap<TaskId, Vec<String>>,
    ) -> Result<()> {
        for (idx, _) in tasks.iter().enumerate() {
            self.positions.entry(idx).or_insert(0);
            self.following.entry(idx).or_insert(true);
        }
        let selected_idx = self.list_state.selected().unwrap_or(0);
        let selected_logs = tasks
            .get(selected_idx)
            .and_then(|(id, _)| logs.get(id))
            .cloned()
            .unwrap_or_default();
        let snapshot: Vec<(String, TaskStatus)> = tasks
            .iter()
            .map(|(id, status)| (id.to_string(), status.clone()))
            .collect();
        self.render_layout(&snapshot, &selected_logs)?;
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
        crossterm::execute!(
            std::io::stdout(),
            crossterm::event::DisableMouseCapture
        )?;
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

    /// Parse each log line individually via `ansi_to_tui` and concatenate the
    /// results. This avoids cross-line ANSI breakage and gracefully handles
    /// malformed sequences.
    fn parse_logs(logs: &[String]) -> Text<'static> {
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(logs.len());
        for raw in logs {
            match raw.as_str().into_text() {
                Ok(parsed) => lines.extend(parsed.lines),
                Err(_) => lines.push(Line::raw(raw.clone())),
            }
        }
        Text::from(lines)
    }

    /// Draws the task list and the selected task's logs.
    ///
    /// # Errors
    ///
    /// Returns an error if the terminal fails to draw.
    #[allow(clippy::cast_possible_truncation)]
    pub fn render_layout(&mut self, tasks: &[(String, TaskStatus)], logs: &[String]) -> Result<()> {
        self.terminal
            .draw(|f| {
                // ── Top-level layout: main area + 1-row keybinds bar ──
                let [main_area, footer_area] =
                    Layout::vertical([Constraint::Min(0), Constraint::Length(1)])
                        .areas(f.area());

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
                    .highlight_style(
                        Style::default()
                            .bg(Color::White)
                            .fg(Color::Black),
                    )
                    .highlight_symbol("> ");
                f.render_stateful_widget(list, left, &mut self.list_state);

                // ── Log pane ──
                let selected_idx = self.list_state.selected().unwrap_or(0);
                // Account for the block border (2 rows) when computing viewport height.
                self.viewport = usize::from(right.height.saturating_sub(2));
                let viewport_width = usize::from(right.width.saturating_sub(2));

                // Parse ANSI per-line for robust rendering.
                let text = Self::parse_logs(logs);

                // Compute total wrapped lines for accurate scroll clamping.
                let total_wrapped = Self::wrapped_line_count(&text, viewport_width);
                let max_offset = total_wrapped.saturating_sub(self.viewport);

                let pos = self.positions.entry(selected_idx).or_insert(0);
                if *self.following.entry(selected_idx).or_insert(true) {
                    *pos = max_offset;
                } else {
                    *pos = (*pos).min(max_offset);
                    if *pos >= max_offset {
                        self.following.insert(selected_idx, true);
                    }
                }

                let log_bg = Color::Rgb(15, 15, 20);
                let log_block = Block::bordered()
                    .style(Style::default().bg(log_bg));

                let paragraph = Paragraph::new(text)
                    .style(Style::default().bg(log_bg))
                    .block(log_block)
                    .scroll((u16::try_from(*pos).unwrap_or(u16::MAX), 0))
                    .wrap(Wrap { trim: false });
                f.render_widget(paragraph, right);

                let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .begin_symbol(None)
                    .end_symbol(None);
                let mut scrollbar_state = ScrollbarState::new(total_wrapped)
                    .position(*pos)
                    .viewport_content_length(self.viewport);
                f.render_stateful_widget(scrollbar, right, &mut scrollbar_state);

                // ── Keybinds footer ──
                let keybinds = Line::from(vec![
                    Span::styled(" q", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled(" quit", Style::default().fg(Color::DarkGray)),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 60))),
                    Span::styled("j/↓", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled(" next", Style::default().fg(Color::DarkGray)),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 60))),
                    Span::styled("k/↑", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled(" prev", Style::default().fg(Color::DarkGray)),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 60))),
                    Span::styled("u", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled(" pg up", Style::default().fg(Color::DarkGray)),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 60))),
                    Span::styled("d", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled(" pg dn", Style::default().fg(Color::DarkGray)),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(60, 60, 60))),
                    Span::styled("scroll", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled(" mouse", Style::default().fg(Color::DarkGray)),
                ]);
                let footer = Paragraph::new(keybinds)
                    .style(Style::default().bg(Color::Rgb(25, 25, 30)));
                f.render_widget(footer, footer_area);
            })
            .map_err(|e| anyhow::anyhow!("Failed to draw terminal: {e}"))?;
        Ok(())
    }
}

/// A minimal, non-interactive backend that prints each task's output to
/// stdout as it arrives. Used when the interactive TUI is disabled.
#[derive(Default)]
pub struct PlainUi {
    printed: HashMap<TaskId, usize>,
}

impl Ui for PlainUi {
    fn render(
        &mut self,
        _tasks: &[(TaskId, TaskStatus)],
        logs: &HashMap<TaskId, Vec<String>>,
    ) -> Result<()> {
        for (task_id, lines) in logs {
            let start = *self.printed.get(task_id).unwrap_or(&0);
            for line in &lines[start..] {
                println!("[{task_id}] {line}");
            }
            self.printed.insert(task_id.clone(), lines.len());
        }
        Ok(())
    }
}
