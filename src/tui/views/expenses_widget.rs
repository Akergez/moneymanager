//! Expenses view as a StatefulWidget

use crossterm::event::{KeyCode, MouseEvent, MouseEventKind, MouseButton};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};
use crate::models::{Category, Expense};
use super::state::{RowSelection, SortableState, SortColumn, SortOrder};
use crate::tui::utils::{format_amount, format_amount_short, format_money};

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
    /// Request to open the account form (create a new account)
    OpenAccountForm,
    /// Request to open the account form editing the currently selected account
    OpenAccountEdit,
    /// Mark the currently selected account (in the accounts view) as current
    MakeCurrent,
    /// Request to open the transfer form
    OpenTransferForm,
    /// Request to confirm deleting the currently selected record
    OpenConfirmDelete,
}

/// Trait for view states that can handle input
pub trait ViewState {
    fn handle_input(&mut self, key: KeyCode) -> ViewInputResult;

    /// Handle mouse input - default implementation does nothing
    fn handle_mouse(&mut self, _mouse: MouseEvent, _area: Rect) -> ViewInputResult {
        ViewInputResult::NotConsumed
    }
}

/// State for the expenses table view
#[derive(Debug, Clone)]
pub struct ExpensesViewState {
    pub sort: SortableState,
    pub rows: RowSelection,
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
            rows: RowSelection::default(),
            table_state: TableState::default(),
        }
    }

    /// Forget the current selection (e.g. after a sort change or reload).
    pub fn reset_selection(&mut self) {
        self.rows.reset();
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
                self.reset_selection();
                ViewInputResult::Consumed
            }
            KeyCode::Right => {
                self.sort.next_column();
                self.reset_selection();
                ViewInputResult::Consumed
            }
            KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
                if self.rows.selected.is_some() {
                    ViewInputResult::OpenConfirmDelete
                } else {
                    ViewInputResult::NotConsumed
                }
            }
            KeyCode::Up => {
                self.rows.move_by(-1);
                ViewInputResult::Consumed
            }
            KeyCode::Down => {
                self.rows.move_by(1);
                ViewInputResult::Consumed
            }
            KeyCode::PageUp => {
                self.rows.move_by(-10);
                ViewInputResult::Consumed
            }
            KeyCode::PageDown => {
                self.rows.move_by(10);
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
            // Scroll wheel support
            MouseEventKind::ScrollUp => {
                self.rows.scroll_by(-1);
                ViewInputResult::Consumed
            }
            MouseEventKind::ScrollDown => {
                self.rows.scroll_by(1);
                ViewInputResult::Consumed
            }
            // Click on header row to change sort column or button
            MouseEventKind::Down(MouseButton::Left) => {
                let x = mouse.column;
                let y = mouse.row;

                if is_in_button_bar {
                    // Check for create button (near right edge)
                    if mouse.column >= area.x + area.width - 12 && mouse.column < area.x + area.width - 1 {
                        return ViewInputResult::OpenExpenseForm;
                    }
                    return ViewInputResult::NotConsumed;
                }

                // Check if click is within the table area and on header row (row 1 inside border)
                if x >= area.x && x < area.x + area.width && y == area.y + 1 {
                    let relative_x = x - area.x - 1; // -1 for border

                    // Determine which column was clicked based on widths
                    // Widths: ID(10), Category(20), Amount(12), Date(12), Comment(rest)
                    let is_narrow = area.width < 60;

                    if is_narrow {
                        // Narrow: Cat(10), Amt(8), Date(6)
                        if relative_x < 10 {
                            self.sort.set_column(SortColumn::CategoryId);
                        } else if relative_x < 18 {
                            self.sort.set_column(SortColumn::Amount);
                        } else {
                            self.sort.set_column(SortColumn::Date);
                        }
                    } else {
                        if relative_x < 10 {
                            self.sort.set_column(SortColumn::Id);
                        } else if relative_x < 30 {
                            self.sort.set_column(SortColumn::CategoryId);
                        } else if relative_x < 42 {
                            self.sort.set_column(SortColumn::Amount);
                        } else if relative_x < 54 {
                            self.sort.set_column(SortColumn::Date);
                        } else {
                            self.sort.set_column(SortColumn::Comment);
                        }
                    }
                    self.reset_selection();
                    return ViewInputResult::Consumed;
                }

                // Click on a data row (below the border, header and its margin)
                // selects it; the table's bottom border sits right above the bar.
                if x >= area.x
                    && x < area.x + area.width
                    && self.rows.click_at(y, area.y, button_bar_start.saturating_sub(1))
                {
                    return ViewInputResult::Consumed;
                }

                ViewInputResult::NotConsumed
            }
            _ => ViewInputResult::NotConsumed,
        }
    }
}

/// Widget for rendering the expenses table
pub struct ExpensesView<'a> {
    expenses: &'a [Expense],
    categories: &'a [Category],
    /// Name of the account shown, for the title.
    account_name: &'a str,
    currency: &'a str,
}

impl<'a> ExpensesView<'a> {
    pub fn new(
        expenses: &'a [Expense],
        categories: &'a [Category],
        account_name: &'a str,
        currency: &'a str,
    ) -> Self {
        Self { expenses, categories, account_name, currency }
    }
}

impl<'a> StatefulWidget for ExpensesView<'a> {
    type State = ExpensesViewState;

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
        render_expenses_table(chunks[0], buf, state, &self);

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

fn render_expenses_table(area: Rect, buf: &mut Buffer, state: &mut ExpensesViewState, view: &ExpensesView) {
        let categories = view.categories;
        let sorted_expenses = state.sort_expenses(view.expenses);
        let sum: f64 = sorted_expenses.iter().map(|e| e.amount).sum();
        state.rows.update_layout(sorted_expenses.len(), area.height);
        let sort = &state.sort;

        // Responsive: determine if narrow screen
        let is_narrow = area.width < 60;
        let is_medium = area.width < 80;

        // Build headers with sort indicators (shorter for narrow screens)
        let headers = if is_narrow {
            vec![
                format!("Cat{}", sort_indicator(sort.column, SortColumn::CategoryId, sort.order)),
                format!("Amt{}", sort_indicator(sort.column, SortColumn::Amount, sort.order)),
                format!("Date{}", sort_indicator(sort.column, SortColumn::Date, sort.order)),
            ]
        } else {
            vec![
                format!("ID{}", sort_indicator(sort.column, SortColumn::Id, sort.order)),
                format!("Category{}", sort_indicator(sort.column, SortColumn::CategoryId, sort.order)),
                format!("Amount{}", sort_indicator(sort.column, SortColumn::Amount, sort.order)),
                format!("Date{}", sort_indicator(sort.column, SortColumn::Date, sort.order)),
                format!("Comment{}", sort_indicator(sort.column, SortColumn::Comment, sort.order)),
            ]
        };

        let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
        let header_cells: Vec<Cell> = headers
            .iter()
            .map(|h| Cell::from(h.clone()).style(header_style))
            .collect();
        let header = Row::new(header_cells).height(1).bottom_margin(1);

        // Build rows - responsive columns, highlighting the selected row
        let selected_index = state.rows.selected;
        let scroll_offset = state.rows.scroll_offset;
        let highlight_style = Style::default().fg(Color::Yellow).bg(Color::Magenta).add_modifier(Modifier::BOLD);

        let rows: Vec<Row> = sorted_expenses
            .iter()
            .skip(scroll_offset)
            .enumerate()
            .map(|(p, exp)| {
                let is_selected = selected_index == Some(scroll_offset + p);
                let cells = if is_narrow {
                    // Narrow: show only essential columns
                    let cat_name = get_category_name(categories, &exp.category_id);
                    let short_cat = if cat_name.len() > 10 {
                        format!("{:.9}", cat_name)
                    } else {
                        cat_name
                    };
                    let cells: Vec<Cell> = vec![
                        Cell::from(short_cat),
                        Cell::from(format_amount_short(exp.amount)),
                        Cell::from(exp.date.format("%m-%d").to_string()),
                    ];
                    cells
                } else {
                    let cells: Vec<Cell> = vec![
                        Cell::from(format_uuid_short(&exp.id)),
                        Cell::from(get_category_name(categories, &exp.category_id)),
                        Cell::from(format_amount(exp.amount)),
                        Cell::from(exp.date.format("%Y-%m-%d").to_string()),
                        Cell::from(exp.comment.clone().unwrap_or_default()),
                    ];
                    cells
                };
                let styled: Vec<Cell> = cells
                    .into_iter()
                    .map(|c| if is_selected { c.style(highlight_style) } else { c })
                    .collect();
                Row::new(styled).height(1)
            })
            .collect();

        // Responsive column widths
        let widths: Vec<Constraint> = if is_narrow {
            vec![
                Constraint::Length(10),  // Category
                Constraint::Length(8),   // Amount
                Constraint::Length(6),   // Date (MM-DD)
            ]
        } else if is_medium {
            vec![
                Constraint::Length(8),
                Constraint::Length(15),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Min(10),
            ]
        } else {
            vec![
                Constraint::Length(10),
                Constraint::Length(20),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Min(20),
            ]
        };

        // Responsive title
        let title = if is_narrow {
            format!("{} | {} {}", sorted_expenses.len(), format_amount_short(sum), view.currency)
        } else {
            format!(
                "Expenses · {} ({}) · Total: {} | Sum: {}",
                view.account_name,
                view.currency,
                sorted_expenses.len(),
                format_money(sum, view.currency)
            )
        };

        let table = Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
            )
            .style(Style::default().fg(Color::White))
            .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

        // Render using StatefulWidget
        StatefulWidget::render(table, area, buf, &mut state.table_state);
}

fn get_category_name(categories: &[Category], category_id: &[u8]) -> String {
    categories
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

