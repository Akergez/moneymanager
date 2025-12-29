use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};
use crate::tui::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let header_cells = ["#", "ID", "Name"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app.top_up_categories.iter().enumerate().skip(app.scroll_offset).map(|(idx, cat)| {
        // Show first 4 bytes as hex
        let id_display = cat.id.iter().take(4)
            .map(|b| format!("{:02x}", b))
            .collect::<String>();
        let cells = vec![
            Cell::from((idx + 1).to_string()), // 1-based index for user reference
            Cell::from(id_display),
            Cell::from(cat.name.clone()),
        ];
        Row::new(cells).height(1)
    });

    let table = Table::new(
        rows,
        [
            ratatui::layout::Constraint::Length(4),
            ratatui::layout::Constraint::Length(10),
            ratatui::layout::Constraint::Min(30),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Top-Up Categories (Total: {})", app.top_up_categories.len()))
    )
    .style(Style::default().fg(Color::White))
    .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_widget(table, area);
}
