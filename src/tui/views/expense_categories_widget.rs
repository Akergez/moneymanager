//! Expense categories view as a StatefulWidget

use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, StatefulWidget, Table, TableState},
};
use crate::models::Category;
use super::expenses_widget::{ViewInputResult, ViewState};

/// State for the expense categories table view
#[derive(Debug, Clone, Default)]
pub struct ExpenseCategoriesViewState {
    pub scroll_offset: usize,
    table_state: TableState,
}

impl ExpenseCategoriesViewState {
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

impl ViewState for ExpenseCategoriesViewState {
    fn handle_input(&mut self, key: KeyCode) -> ViewInputResult {
        match key {
            KeyCode::Char('n') | KeyCode::Char('N') => ViewInputResult::OpenCategoryForm,
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
}

/// Widget for rendering the expense categories table
pub struct ExpenseCategoriesView<'a> {
    categories: &'a [Category],
}

impl<'a> ExpenseCategoriesView<'a> {
    pub fn new(categories: &'a [Category]) -> Self {
        Self { categories }
    }

    fn format_uuid_short(id: &[u8]) -> String {
        id.iter().take(4).map(|b| format!("{:02x}", b)).collect()
    }
}

impl<'a> StatefulWidget for ExpenseCategoriesView<'a> {
    type State = ExpenseCategoriesViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
        let header = Row::new(vec![
            Cell::from("ID").style(header_style),
            Cell::from("Name").style(header_style),
        ]).height(1).bottom_margin(1);

        let rows: Vec<Row> = self.categories
            .iter()
            .skip(state.scroll_offset)
            .map(|cat| {
                Row::new(vec![
                    Cell::from(Self::format_uuid_short(&cat.id)),
                    Cell::from(cat.name.clone()),
                ]).height(1)
            })
            .collect();

        let widths = [Constraint::Length(10), Constraint::Min(20)];

        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default().borders(Borders::ALL).title(format!("Expense Categories ({})", self.categories.len())))
            .style(Style::default().fg(Color::White))
            .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

        StatefulWidget::render(table, area, buf, &mut state.table_state);
    }
}
