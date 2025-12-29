use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};
use crate::tui::app::{App, SortColumn, SortOrder};

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let sort_indicator = |col: SortColumn| -> &str {
        if app.top_up_sort_column == col {
            match app.top_up_sort_order {
                SortOrder::Ascending => " ▲",
                SortOrder::Descending => " ▼",
            }
        } else {
            ""
        }
    };

    let header_cells = vec![
        format!("ID{}", sort_indicator(SortColumn::Id)),
        format!("Category{}", sort_indicator(SortColumn::CategoryId)),
        format!("Amount{}", sort_indicator(SortColumn::Amount)),
        format!("Date{}", sort_indicator(SortColumn::Date)),
        format!("Comment{}", sort_indicator(SortColumn::Comment)),
    ]
    .iter()
    .map(|h| Cell::from(h.clone()).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)))
    .collect::<Vec<_>>();
    
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let sorted_top_ups = app.get_sorted_top_ups();
    let rows = sorted_top_ups.iter().skip(app.scroll_offset).map(|top_up| {
        let category_name = app.get_top_up_category_name(&top_up.category_id);
        // Show first 4 bytes as hex
        let id_display = top_up.id.iter().take(4)
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        let cells = vec![
            Cell::from(id_display),
            Cell::from(category_name),
            Cell::from(format!("{:.2}", top_up.amount)),
            Cell::from(top_up.date.format("%Y-%m-%d").to_string()),
            Cell::from(top_up.comment.clone().unwrap_or_default()),
        ];
        Row::new(cells).height(1)
    });

    let total: f64 = sorted_top_ups.iter().map(|t| t.amount).sum();

    let table = Table::new(
        rows,
        [
            ratatui::layout::Constraint::Length(6),
            ratatui::layout::Constraint::Length(20),
            ratatui::layout::Constraint::Length(12),
            ratatui::layout::Constraint::Length(12),
            ratatui::layout::Constraint::Min(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Top-Ups (Total: {} | Sum: {:.2})", sorted_top_ups.len(), total))
    )
    .style(Style::default().fg(Color::White))
    .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_widget(table, area);
}
