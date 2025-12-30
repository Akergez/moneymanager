use ratatui::{
    layout::{Constraint, Rect},
    widgets::Cell,
    Frame,
};
use crate::tui::app::{App, SortColumn};
use super::common::{format_uuid_short, render_sortable_table, sort_indicator, TableConfig};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let sorted_expenses = app.get_sorted_expenses();
    let total: f64 = sorted_expenses.iter().map(|e| e.amount).sum();

    let headers = vec![
        format!("ID{}", sort_indicator(app.expense_sort_column, SortColumn::Id, app.expense_sort_order)),
        format!("Category{}", sort_indicator(app.expense_sort_column, SortColumn::CategoryId, app.expense_sort_order)),
        format!("Amount{}", sort_indicator(app.expense_sort_column, SortColumn::Amount, app.expense_sort_order)),
        format!("Date{}", sort_indicator(app.expense_sort_column, SortColumn::Date, app.expense_sort_order)),
        format!("Comment{}", sort_indicator(app.expense_sort_column, SortColumn::Comment, app.expense_sort_order)),
    ];

    let config = TableConfig {
        title: format!("Expenses (Total: {} | Sum: {:.2})", sorted_expenses.len(), total),
        widths: vec![
            Constraint::Length(6),
            Constraint::Length(20),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Min(20),
        ],
    };

    // Clone data needed for the closure
    let categories = app.categories.clone();

    render_sortable_table(
        frame,
        area,
        config,
        headers,
        sorted_expenses.iter(),
        |exp| {
            let category_name = categories
                .iter()
                .find(|c| c.id.as_slice() == exp.category_id.as_slice())
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            vec![
                Cell::from(format_uuid_short(&exp.id)),
                Cell::from(category_name),
                Cell::from(format!("{:.2}", exp.amount)),
                Cell::from(exp.date.format("%Y-%m-%d").to_string()),
                Cell::from(exp.comment.clone().unwrap_or_default()),
            ]
        },
        app.scroll_offset,
    );
}
