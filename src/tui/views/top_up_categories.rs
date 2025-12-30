use ratatui::{
    layout::{Constraint, Rect},
    widgets::Cell,
    Frame,
};
use crate::tui::app::App;
use super::common::{format_uuid_short, render_simple_table, TableConfig};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let config = TableConfig {
        title: format!("Top-Up Categories (Total: {})", app.top_up_categories.len()),
        widths: vec![
            Constraint::Length(4),
            Constraint::Length(10),
            Constraint::Min(30),
        ],
    };

    render_simple_table(
        frame,
        area,
        config,
        &["#", "ID", "Name"],
        app.top_up_categories.iter().enumerate(),
        |(idx, cat)| {
            vec![
                Cell::from((idx + 1).to_string()),
                Cell::from(format_uuid_short(&cat.id)),
                Cell::from(cat.name.clone()),
            ]
        },
        app.scroll_offset,
    );
}
