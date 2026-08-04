use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};

use crate::tasks::TaskStatus;

pub fn render(
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
        .highlight_symbol("> ");
    f.render_stateful_widget(list, left, list_state);

    let paragraph = Paragraph::new(logs.join("\n")).block(Block::bordered().title("logs"));
    f.render_widget(paragraph, right);
}
