//! Top-up categories view as a StatefulWidget

use crossterm::event::{KeyCode, MouseEvent, MouseEventKind, MouseButton};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};
use crate::models::TopUpCategory;
use super::expenses_widget::{ViewInputResult, ViewState};

/// State for the top-up categories table view
#[derive(Debug, Clone, Default)]
pub struct TopUpCategoriesViewState {
    pub scroll_offset: usize,
    table_state: TableState,
}

impl TopUpCategoriesViewState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    pub fn page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(10);
    }

    pub fn page_down(&mut self) {
        self.scroll_offset += 10;
    }
}

impl ViewState for TopUpCategoriesViewState {
    fn handle_input(&mut self, key: KeyCode) -> ViewInputResult {
        match key {
            KeyCode::Char('n') | KeyCode::Char('N') => ViewInputResult::OpenTopUpCategoryForm,
            KeyCode::Up => {
                self.scroll_up();
                ViewInputResult::Consumed
            }
            KeyCode::Down => {
                self.scroll_down();
                ViewInputResult::Consumed
            }
            KeyCode::PageUp => {
                self.page_up();
                ViewInputResult::Consumed
            }
            KeyCode::PageDown => {
                self.page_down();
                ViewInputResult::Consumed
            }
            _ => ViewInputResult::NotConsumed,
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) -> ViewInputResult {
        // Button bar is 3 rows at the bottom
        let button_bar_height = 3u16;
        let button_bar_start = area.y + area.height.saturating_sub(button_bar_height);
        let is_in_button_bar = mouse.row >= button_bar_start;

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_up();
                ViewInputResult::Consumed
            }
            MouseEventKind::ScrollDown => {
                self.scroll_down();
                ViewInputResult::Consumed
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if is_in_button_bar {
                    // Check for create button (near right edge)
                    if mouse.column >= area.x + area.width - 12 && mouse.column < area.x + area.width - 1 {
                        return ViewInputResult::OpenTopUpCategoryForm;
                    }
                }
                ViewInputResult::NotConsumed
            }
            _ => ViewInputResult::NotConsumed,
        }
    }
}

/// Widget for rendering the top-up categories table
pub struct TopUpCategoriesView<'a> {
    categories: &'a [TopUpCategory],
}

impl<'a> TopUpCategoriesView<'a> {
    pub fn new(categories: &'a [TopUpCategory]) -> Self {
        Self { categories }
    }
}

impl<'a> StatefulWidget for TopUpCategoriesView<'a> {
    type State = TopUpCategoriesViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Split into table on top and button bar at bottom
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),       // Table
                Constraint::Length(3),    // Button bar
            ])
            .split(area);


        // Render table
        render_table(chunks[0], buf, state, self.categories);

        // Render button bar
        render_button_bar(chunks[1], buf);
    }
}

fn render_button_bar(area: Rect, buf: &mut Buffer) {
    let available_width = area.width.saturating_sub(2) as usize;
    let button_width = 10; // "[+ Create]"
    let left_padding = available_width.saturating_sub(button_width);

    let line = Line::from(vec![
        Span::raw(" ".repeat(left_padding)),
        Span::styled("[+ Create]", Style::default().fg(Color::Green)),
    ]);

    let paragraph = Paragraph::new(vec![line])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
        );

    Widget::render(paragraph, area, buf);
}

fn render_table(area: Rect, buf: &mut Buffer, state: &mut TopUpCategoriesViewState, categories: &[TopUpCategory]) {
        // Responsive: determine if narrow screen
        let is_narrow = area.width < 60;

        let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);

        // Responsive headers
        let header = if is_narrow {
            Row::new(vec![
                Cell::from("Name").style(header_style),
            ]).height(1).bottom_margin(1)
        } else {
            Row::new(vec![
                Cell::from("ID").style(header_style),
                Cell::from("Name").style(header_style),
            ]).height(1).bottom_margin(1)
        };

        // Responsive rows
        let rows: Vec<Row> = categories
            .iter()
            .skip(state.scroll_offset)
            .map(|cat| {
                if is_narrow {
                    Row::new(vec![
                        Cell::from(cat.name.clone()),
                    ]).height(1)
                } else {
                    Row::new(vec![
                        Cell::from(format_uuid_short(&cat.id)),
                        Cell::from(cat.name.clone()),
                    ]).height(1)
                }
            })
            .collect();

        // Responsive widths
        let widths: Vec<Constraint> = if is_narrow {
            vec![Constraint::Min(10)]
        } else {
            vec![Constraint::Length(10), Constraint::Min(20)]
        };

        // Responsive title
        let title = if is_narrow {
            format!("Categories ({})", categories.len())
        } else {
            format!("Top-Up Categories ({})", categories.len())
        };

        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default().borders(Borders::ALL).title(title))
            .style(Style::default().fg(Color::White))
            .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

        StatefulWidget::render(table, area, buf, &mut state.table_state);
}

fn format_uuid_short(id: &[u8]) -> String {
    id.iter().take(4).map(|b| format!("{:02x}", b)).collect()
}
