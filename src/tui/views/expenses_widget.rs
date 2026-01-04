//! Expenses view as a StatefulWidget

use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, StatefulWidget, Table, TableState},
};
use crate::models::{Category, Expense};
use super::state::{SortableState, SortColumn, SortOrder};

/// Result of view input handling
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewInputResult {
    /// Input was consumed by this view
    Consumed,
    /// Input was not handled, pass to global handler
    NotConsumed,
    /// Request to open category form
    OpenCategoryForm,
    /// Request to open expense form
    OpenExpenseForm,
    /// Request to open top up category form
    OpenTopUpCategoryForm,
    /// Request to open top up form
    OpenTopUpForm,
}

/// Trait for view states that can handle input
pub trait ViewState {
    fn handle_input(&mut self, key: KeyCode) -> ViewInputResult;
}

/// State for the expenses table view
#[derive(Debug, Clone)]
pub struct ExpensesViewState {
    pub sort: SortableState,
    pub scroll_offset: usize,
    table_state: TableState,
}

impl Default for ExpensesViewState {
    fn default() -> Self {
        Self::new()
    }
}

impl ExpensesViewState {
    pub fn new() -> Self {
        Self {
            sort: SortableState::new(),
            scroll_offset: 0,
            table_state: TableState::default(),
        }
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


    /// Sort expenses according to current sort state
    pub fn sort_expenses(&self, expenses: &[Expense]) -> Vec<Expense> {
        let mut sorted = expenses.to_vec();
        let sort = &self.sort;

        sorted.sort_by(|a, b| {
            let cmp = match sort.column {
                SortColumn::Id => a.id.cmp(&b.id),
                SortColumn::CategoryId => a.category_id.cmp(&b.category_id),
                SortColumn::Amount => a.amount.partial_cmp(&b.amount).unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Date => a.date.cmp(&b.date),
                SortColumn::Comment => a.comment.cmp(&b.comment),
            };
            sort.apply(cmp)
        });

        sorted
    }
}

impl ViewState for ExpensesViewState {
    fn handle_input(&mut self, key: KeyCode) -> ViewInputResult {
        match key {
            KeyCode::Char('n') | KeyCode::Char('N') => ViewInputResult::OpenExpenseForm,
            KeyCode::Left => {
                self.sort.prev_column();
                ViewInputResult::Consumed
            }
            KeyCode::Right => {
                self.sort.next_column();
                ViewInputResult::Consumed
            }
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

/// Widget for rendering the expenses table
pub struct ExpensesView<'a> {
    expenses: &'a [Expense],
    categories: &'a [Category],
}

impl<'a> ExpensesView<'a> {
    pub fn new(expenses: &'a [Expense], categories: &'a [Category]) -> Self {
        Self { expenses, categories }
    }

    fn get_category_name(&self, category_id: &[u8]) -> String {
        self.categories
            .iter()
            .find(|c| c.id.as_slice() == category_id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    fn format_uuid_short(id: &[u8]) -> String {
        id.iter()
            .take(4)
            .map(|b| format!("{:02x}", b))
            .collect()
    }

    fn sort_indicator(current: SortColumn, target: SortColumn, order: SortOrder) -> &'static str {
        if current == target {
            match order {
                SortOrder::Ascending => " ▲",
                SortOrder::Descending => " ▼",
            }
        } else {
            ""
        }
    }
}

impl<'a> StatefulWidget for ExpensesView<'a> {
    type State = ExpensesViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let sorted_expenses = state.sort_expenses(self.expenses);
        let total: f64 = sorted_expenses.iter().map(|e| e.amount).sum();
        let sort = &state.sort;

        // Build headers with sort indicators
        let headers = vec![
            format!("ID{}", Self::sort_indicator(sort.column, SortColumn::Id, sort.order)),
            format!("Category{}", Self::sort_indicator(sort.column, SortColumn::CategoryId, sort.order)),
            format!("Amount{}", Self::sort_indicator(sort.column, SortColumn::Amount, sort.order)),
            format!("Date{}", Self::sort_indicator(sort.column, SortColumn::Date, sort.order)),
            format!("Comment{}", Self::sort_indicator(sort.column, SortColumn::Comment, sort.order)),
        ];

        let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
        let header_cells: Vec<Cell> = headers
            .iter()
            .map(|h| Cell::from(h.clone()).style(header_style))
            .collect();
        let header = Row::new(header_cells).height(1).bottom_margin(1);

        // Build rows
        let rows: Vec<Row> = sorted_expenses
            .iter()
            .skip(state.scroll_offset)
            .map(|exp| {
                let cells = vec![
                    Cell::from(Self::format_uuid_short(&exp.id)),
                    Cell::from(self.get_category_name(&exp.category_id)),
                    Cell::from(format!("{:.2}", exp.amount)),
                    Cell::from(exp.date.format("%Y-%m-%d").to_string()),
                    Cell::from(exp.comment.clone().unwrap_or_default()),
                ];
                Row::new(cells).height(1)
            })
            .collect();

        let widths = [
            Constraint::Length(10),
            Constraint::Length(20),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Min(20),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Expenses (Total: {} | Sum: {:.2})", sorted_expenses.len(), total))
            )
            .style(Style::default().fg(Color::White))
            .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

        // Render using StatefulWidget
        StatefulWidget::render(table, area, buf, &mut state.table_state);
    }
}

