use ratatui::{
    layout::{Constraint, Rect},
    widgets::Cell,
    Frame,
};
use crate::tui::app::{App, SortColumn};
use super::common::{format_uuid_short, render_sortable_table, sort_indicator, TableConfig};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let sorted_top_ups = app.get_sorted_top_ups();
    let total: f64 = sorted_top_ups.iter().map(|t| t.amount).sum();

    let headers = vec![
        format!("ID{}", sort_indicator(app.top_up_sort_column, SortColumn::Id, app.top_up_sort_order)),
        format!("Category{}", sort_indicator(app.top_up_sort_column, SortColumn::CategoryId, app.top_up_sort_order)),
        format!("Amount{}", sort_indicator(app.top_up_sort_column, SortColumn::Amount, app.top_up_sort_order)),
        format!("Date{}", sort_indicator(app.top_up_sort_column, SortColumn::Date, app.top_up_sort_order)),
        format!("Comment{}", sort_indicator(app.top_up_sort_column, SortColumn::Comment, app.top_up_sort_order)),
    ];

    let config = TableConfig {
        title: format!("Top-Ups (Total: {} | Sum: {:.2})", sorted_top_ups.len(), total),
        widths: vec![
            Constraint::Length(6),
            Constraint::Length(20),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Min(20),
        ],
    };

    // Clone data needed for the closure
    let top_up_categories = app.top_up_categories.clone();

    render_sortable_table(
        frame,
        area,
        config,
        headers,
        sorted_top_ups.iter(),
        |top_up| {
            let category_name = top_up_categories
                .iter()
                .find(|c| c.id.as_slice() == top_up.category_id.as_slice())
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            vec![
                Cell::from(format_uuid_short(&top_up.id)),
                Cell::from(category_name),
                Cell::from(format!("{:.2}", top_up.amount)),
                Cell::from(top_up.date.format("%Y-%m-%d").to_string()),
                Cell::from(top_up.comment.clone().unwrap_or_default()),
            ]
        },
        app.scroll_offset,
    );
}
