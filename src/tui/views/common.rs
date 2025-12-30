use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};
use crate::tui::app::{SortColumn, SortOrder};

/// Color palette for charts and visual elements
pub const CHART_COLORS: [Color; 10] = [
    Color::Cyan,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Red,
    Color::LightCyan,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
];

/// Common styles used across views
pub mod styles {
    use super::*;

    pub fn header() -> Style {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    }

    pub fn highlight() -> Style {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    }

    pub fn normal() -> Style {
        Style::default().fg(Color::White)
    }

    pub fn error() -> Style {
        Style::default().fg(Color::Red)
    }

    pub fn instruction() -> Style {
        Style::default().fg(Color::Gray)
    }

    pub fn row_highlight() -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }
}

/// Format UUID bytes as short hex string (first 4 bytes)
pub fn format_uuid_short(id: &[u8]) -> String {
    id.iter()
        .take(4)
        .map(|b| format!("{:02x}", b))
        .collect()
}

/// Get sort indicator arrow for column headers
pub fn sort_indicator(current: SortColumn, target: SortColumn, order: SortOrder) -> &'static str {
    if current == target {
        match order {
            SortOrder::Ascending => " ▲",
            SortOrder::Descending => " ▼",
        }
    } else {
        ""
    }
}

/// Create a centered popup rect
pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Configuration for rendering a data table
pub struct TableConfig {
    pub title: String,
    pub widths: Vec<Constraint>,
}

/// Render a simple table with static headers (for categories)
pub fn render_simple_table<'a, I, F>(
    frame: &mut Frame,
    area: Rect,
    config: TableConfig,
    headers: &[&str],
    items: I,
    row_mapper: F,
    scroll_offset: usize,
) where
    I: Iterator,
    F: Fn(I::Item) -> Vec<Cell<'a>>,
{
    let header_cells = headers
        .iter()
        .map(|h| Cell::from(*h).style(styles::header()));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = items.skip(scroll_offset).map(|item| {
        Row::new(row_mapper(item)).height(1)
    });

    let table = Table::new(rows, config.widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(config.title))
        .style(styles::normal())
        .row_highlight_style(styles::row_highlight());

    frame.render_widget(table, area);
}

/// Render a sortable table with dynamic headers (for expenses/top-ups)
pub fn render_sortable_table<'a, I, F>(
    frame: &mut Frame,
    area: Rect,
    config: TableConfig,
    headers: Vec<String>,
    items: I,
    row_mapper: F,
    scroll_offset: usize,
) where
    I: Iterator,
    F: Fn(I::Item) -> Vec<Cell<'a>>,
{
    let header_cells: Vec<Cell> = headers
        .iter()
        .map(|h| Cell::from(h.clone()).style(styles::header()))
        .collect();
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = items.skip(scroll_offset).map(|item| {
        Row::new(row_mapper(item)).height(1)
    });

    let table = Table::new(rows, config.widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(config.title))
        .style(styles::normal())
        .row_highlight_style(styles::row_highlight());

    frame.render_widget(table, area);
}

