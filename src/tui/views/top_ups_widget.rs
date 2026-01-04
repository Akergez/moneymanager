//! Top-ups view as a StatefulWidget

use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, StatefulWidget, Table, TableState},
};
use crate::models::{TopUp, TopUpCategory};
use super::state::{SortableState, SortColumn, SortOrder};
use super::expenses_widget::{ViewInputResult, ViewState};

/// State for the top-ups table view
#[derive(Debug, Clone)]
pub struct TopUpsViewState {
    pub sort: SortableState,
    pub scroll_offset: usize,
    table_state: TableState,
}

impl Default for TopUpsViewState {
    fn default() -> Self {
        Self::new()
    }
}

impl TopUpsViewState {
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


    pub fn sort_top_ups(&self, top_ups: &[TopUp]) -> Vec<TopUp> {
        let mut sorted = top_ups.to_vec();
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

impl ViewState for TopUpsViewState {
    fn handle_input(&mut self, key: KeyCode) -> ViewInputResult {
        match key {
            KeyCode::Char('n') | KeyCode::Char('N') => ViewInputResult::OpenTopUpForm,
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

/// Widget for rendering the top-ups table
pub struct TopUpsView<'a> {
    top_ups: &'a [TopUp],
    categories: &'a [TopUpCategory],
}

impl<'a> TopUpsView<'a> {
    pub fn new(top_ups: &'a [TopUp], categories: &'a [TopUpCategory]) -> Self {
        Self { top_ups, categories }
    }

    fn get_category_name(&self, category_id: &[u8]) -> String {
        self.categories
            .iter()
            .find(|c| c.id.as_slice() == category_id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    fn format_uuid_short(id: &[u8]) -> String {
        id.iter().take(4).map(|b| format!("{:02x}", b)).collect()
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

impl<'a> StatefulWidget for TopUpsView<'a> {
    type State = TopUpsViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let sorted = state.sort_top_ups(self.top_ups);
        let total: f64 = sorted.iter().map(|t| t.amount).sum();
        let sort = &state.sort;

        // Responsive: determine if narrow screen
        let is_narrow = area.width < 60;
        let is_medium = area.width < 80;

        // Build headers with sort indicators (shorter for narrow screens)
        let headers = if is_narrow {
            vec![
                format!("Cat{}", Self::sort_indicator(sort.column, SortColumn::CategoryId, sort.order)),
                format!("Amt{}", Self::sort_indicator(sort.column, SortColumn::Amount, sort.order)),
                format!("Date{}", Self::sort_indicator(sort.column, SortColumn::Date, sort.order)),
            ]
        } else {
            vec![
                format!("ID{}", Self::sort_indicator(sort.column, SortColumn::Id, sort.order)),
                format!("Category{}", Self::sort_indicator(sort.column, SortColumn::CategoryId, sort.order)),
                format!("Amount{}", Self::sort_indicator(sort.column, SortColumn::Amount, sort.order)),
                format!("Date{}", Self::sort_indicator(sort.column, SortColumn::Date, sort.order)),
                format!("Comment{}", Self::sort_indicator(sort.column, SortColumn::Comment, sort.order)),
            ]
        };

        let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
        let header_cells: Vec<Cell> = headers.iter().map(|h| Cell::from(h.clone()).style(header_style)).collect();
        let header = Row::new(header_cells).height(1).bottom_margin(1);

        // Build rows - responsive columns
        let rows: Vec<Row> = sorted
            .iter()
            .skip(state.scroll_offset)
            .map(|t| {
                let cells = if is_narrow {
                    // Narrow: show only essential columns
                    let cat_name = self.get_category_name(&t.category_id);
                    let short_cat = if cat_name.len() > 10 {
                        format!("{:.9}", cat_name)
                    } else {
                        cat_name
                    };
                    vec![
                        Cell::from(short_cat),
                        Cell::from(format!("{:.0}", t.amount)),
                        Cell::from(t.date.format("%m-%d").to_string()),
                    ]
                } else {
                    vec![
                        Cell::from(Self::format_uuid_short(&t.id)),
                        Cell::from(self.get_category_name(&t.category_id)),
                        Cell::from(format!("{:.2}", t.amount)),
                        Cell::from(t.date.format("%Y-%m-%d").to_string()),
                        Cell::from(t.comment.clone().unwrap_or_default()),
                    ]
                };
                Row::new(cells).height(1)
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
            format!("{} | {:.0}", sorted.len(), total)
        } else {
            format!("Top-Ups (Total: {} | Sum: {:.2})", sorted.len(), total)
        };

        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default().borders(Borders::ALL).title(title))
            .style(Style::default().fg(Color::White))
            .row_highlight_style(Style::default().add_modifier(Modifier::BOLD));

        StatefulWidget::render(table, area, buf, &mut state.table_state);
    }
}
