use std::collections::HashMap;

use anyhow::Result;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Layout},
    widgets::{Block, List, ListItem, ListState, Paragraph},
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
}

type DefaultBackend = CrosstermBackend<std::io::Stdout>;

/// A full-screen, interactive terminal UI built on ratatui.
pub struct Tui {
    terminal: Terminal<DefaultBackend>,
    list_state: ListState,
}

impl Tui {
    /// Initialises the terminal for the interactive UI.
    ///
    /// # Errors
    ///
    /// Returns an error if the terminal cannot be put into raw mode.
    pub fn new() -> Result<Self> {
        let terminal = ratatui::try_init()?;
        Ok(Self {
            terminal,
            list_state: ListState::default().with_selected(Some(0)),
        })
    }
}

impl Ui for Tui {
    fn render(
        &mut self,
        tasks: &[(TaskId, TaskStatus)],
        logs: &HashMap<TaskId, Vec<String>>,
    ) -> Result<()> {
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
        self.terminal.draw(|f| {
            render(f, &snapshot, &selected_logs, &mut self.list_state);
        })?;
        Ok(())
    }

    fn navigate_next(&mut self) {
        self.list_state.select_next();
    }

    fn navigate_previous(&mut self) {
        self.list_state.select_previous();
    }

    fn shutdown(&mut self) -> Result<()> {
        ratatui::restore();
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

fn render(
    f: &mut ratatui::Frame,
    tasks: &[(String, TaskStatus)],
    logs: &[String],
    list_state: &mut ListState,
) {
    let [left, right] =
        Layout::horizontal([Constraint::Length(30), Constraint::Min(0)]).areas(f.area());

    let items: Vec<ListItem> = tasks
        .iter()
        .map(|(name, status)| {
            let symbol = match status {
                TaskStatus::Waiting => "○",
                TaskStatus::Running => "●",
                TaskStatus::Complete => "✓",
                TaskStatus::Failed { .. } => "✗",
                TaskStatus::Restarting => "↻",
            };
            ListItem::new(format!("{symbol} {name}"))
        })
        .collect();

    let list = List::new(items)
        .block(Block::bordered().title("tasks"))
        .highlight_style(ratatui::style::Style::default().bg(ratatui::style::Color::Blue))
        .highlight_symbol("> ");
    f.render_stateful_widget(list, left, list_state);

    let paragraph = Paragraph::new(logs.join("\n")).block(Block::bordered().title("logs"));
    f.render_widget(paragraph, right);
}

