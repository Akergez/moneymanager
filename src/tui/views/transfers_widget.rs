//! Transfers view as a StatefulWidget

use std::cmp::Ordering;

use crossterm::event::{KeyCode, MouseEvent, MouseEventKind, MouseButton};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};

use crate::ledger::{account_name, currency_of, format_rate};
use crate::models::{Account, Transfer};
use crate::tui::utils::{format_amount_short, format_money};
use super::expenses_widget::{ViewInputResult, ViewState};
use super::state::{RowSelection, SortOrder};

/// Sortable columns of the transfers table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferSortColumn {
    Date,
    From,
    To,
    Out,
    In,
    Comment,
}

impl TransferSortColumn {
    const ALL: [Self; 6] = [Self::Date, Self::From, Self::To, Self::Out, Self::In, Self::Comment];

    fn position(self) -> usize {
        Self::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }

    fn next(self) -> Self {
        Self::ALL[(self.position() + 1) % Self::ALL.len()]
    }

    fn prev(self) -> Self {
        Self::ALL[(self.position() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// State for the transfers table view
#[derive(Debug, Clone)]
pub struct TransfersViewState {
    pub sort_column: TransferSortColumn,
    pub sort_order: SortOrder,
    pub rows: RowSelection,
    /// Screen x-ranges `[start, end)` of the sortable header columns from the
    /// last render, for mouse clicks.
    header_columns: Vec<(TransferSortColumn, u16, u16)>,
    table_state: TableState,
}

impl Default for TransfersViewState {
    fn default() -> Self {
        Self::new()
    }
}

impl TransfersViewState {
    pub fn new() -> Self {
        Self {
            sort_column: TransferSortColumn::Date,
            sort_order: SortOrder::Descending,
            rows: RowSelection::default(),
            header_columns: Vec::new(),
            table_state: TableState::default(),
        }
    }

    pub fn reset_selection(&mut self) {
        self.rows.reset();
    }

    /// Sort by `column`; the same column again flips the order.
    fn sort_by(&mut self, column: TransferSortColumn) {
        if self.sort_column == column {
            self.sort_order = self.sort_order.toggle();
        } else {
            self.sort_column = column;
            self.sort_order = SortOrder::Ascending;
        }
        self.reset_selection();
    }

    /// Transfers in display order. The table and the delete action both use
    /// this, so a selected row index always maps to the same transfer.
    pub fn sorted<'t>(&self, transfers: &'t [Transfer], accounts: &[Account]) -> Vec<&'t Transfer> {
        let mut sorted: Vec<&Transfer> = transfers.iter().collect();
        sorted.sort_by(|a, b| {
            let cmp = match self.sort_column {
                TransferSortColumn::Date => a.date.cmp(&b.date),
                TransferSortColumn::From => account_name(accounts, &a.from_account_id)
                    .cmp(account_name(accounts, &b.from_account_id)),
                TransferSortColumn::To => account_name(accounts, &a.to_account_id)
                    .cmp(account_name(accounts, &b.to_account_id)),
                TransferSortColumn::Out => a.amount_from.partial_cmp(&b.amount_from).unwrap_or(Ordering::Equal),
                TransferSortColumn::In => a.amount_to.partial_cmp(&b.amount_to).unwrap_or(Ordering::Equal),
                TransferSortColumn::Comment => a.comment.cmp(&b.comment),
            };
            self.sort_order.apply(cmp)
        });
        sorted
    }
}

impl ViewState for TransfersViewState {
    fn handle_input(&mut self, key: KeyCode) -> ViewInputResult {
        match key {
            KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
                if self.rows.selected.is_some() {
                    ViewInputResult::OpenConfirmDelete
                } else {
                    ViewInputResult::NotConsumed
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => ViewInputResult::OpenTransferForm,
            KeyCode::Left => {
                self.sort_by(self.sort_column.prev());
                ViewInputResult::Consumed
            }
            KeyCode::Right => {
                self.sort_by(self.sort_column.next());
                ViewInputResult::Consumed
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
        let button_bar_height = 3u16;
        let button_bar_start = area.y + area.height.saturating_sub(button_bar_height);
        let is_in_button_bar = mouse.row >= button_bar_start;

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.rows.scroll_by(-1);
                ViewInputResult::Consumed
            }
            MouseEventKind::ScrollDown => {
                self.rows.scroll_by(1);
                ViewInputResult::Consumed
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let x = mouse.column;
                let y = mouse.row;

                if is_in_button_bar {
                    if x >= (area.x + area.width).saturating_sub(12) && x < (area.x + area.width).saturating_sub(1) {
                        return ViewInputResult::OpenTransferForm;
                    }
                    return ViewInputResult::NotConsumed;
                }

                // Header row: sort by the clicked column.
                if y == area.y + 1 {
                    if let Some(&(column, _, _)) =
                        self.header_columns.iter().find(|(_, start, end)| x >= *start && x < *end)
                    {
                        self.sort_by(column);
                    }
                    return ViewInputResult::Consumed;
                }

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

/// Widget for rendering the transfers table
pub struct TransfersView<'a> {
    transfers: &'a [Transfer],
    accounts: &'a [Account],
}

impl<'a> TransfersView<'a> {
    pub fn new(transfers: &'a [Transfer], accounts: &'a [Account]) -> Self {
        Self { transfers, accounts }
    }
}

impl<'a> StatefulWidget for TransfersView<'a> {
    type State = TransfersViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        render_table(chunks[0], buf, state, &self);
        render_button_bar(chunks[1], buf);
    }
}

fn sort_indicator(state: &TransfersViewState, column: TransferSortColumn) -> &'static str {
    if state.sort_column != column {
        ""
    } else {
        match state.sort_order {
            SortOrder::Ascending => " ▲",
            SortOrder::Descending => " ▼",
        }
    }
}

fn render_table(area: Rect, buf: &mut Buffer, state: &mut TransfersViewState, view: &TransfersView) {
    let is_narrow = area.width < 80;
    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);

    // (label, sort column, width); the Rate column is not sortable.
    let columns: Vec<(&str, Option<TransferSortColumn>, Constraint)> = if is_narrow {
        vec![
            ("Date", Some(TransferSortColumn::Date), Constraint::Length(8)),
            ("From", Some(TransferSortColumn::From), Constraint::Min(8)),
            ("To", Some(TransferSortColumn::To), Constraint::Min(8)),
            ("Out", Some(TransferSortColumn::Out), Constraint::Length(10)),
            ("In", Some(TransferSortColumn::In), Constraint::Length(10)),
        ]
    } else {
        vec![
            ("Date", Some(TransferSortColumn::Date), Constraint::Length(12)),
            ("From", Some(TransferSortColumn::From), Constraint::Min(12)),
            ("To", Some(TransferSortColumn::To), Constraint::Min(12)),
            ("Out", Some(TransferSortColumn::Out), Constraint::Length(15)),
            ("In", Some(TransferSortColumn::In), Constraint::Length(15)),
            ("Rate", None, Constraint::Length(22)),
            ("Comment", Some(TransferSortColumn::Comment), Constraint::Min(10)),
        ]
    };

    let header = Row::new(
        columns
            .iter()
            .map(|(label, column, _)| {
                let indicator = column.map(|c| sort_indicator(state, c)).unwrap_or("");
                Cell::from(format!("{label}{indicator}")).style(header_style)
            })
            .collect::<Vec<_>>(),
    )
    .height(1)
    .bottom_margin(1);
    let widths: Vec<Constraint> = columns.iter().map(|(_, _, w)| *w).collect();

    // Remember where each header column is for mouse sorting (the Table lays
    // out columns inside the border with 1 cell of spacing).
    let inner = Rect { x: area.x + 1, width: area.width.saturating_sub(2), ..area };
    let cells = Layout::horizontal(widths.clone()).spacing(1).split(inner);
    state.header_columns = columns
        .iter()
        .zip(cells.iter())
        .filter_map(|((_, column, _), rect)| column.map(|c| (c, rect.x, rect.x + rect.width)))
        .collect();

    let sorted = state.sorted(view.transfers, view.accounts);
    state.rows.update_layout(sorted.len(), area.height);
    let selected_index = state.rows.selected;
    let scroll_offset = state.rows.scroll_offset;
    let highlight_style = Style::default().fg(Color::Yellow).bg(Color::Magenta).add_modifier(Modifier::BOLD);

    let rows: Vec<Row> = sorted
        .iter()
        .skip(scroll_offset)
        .enumerate()
        .map(|(p, t)| {
            let from_currency = currency_of(view.accounts, &t.from_account_id);
            let to_currency = currency_of(view.accounts, &t.to_account_id);
            let mut cells = vec![
                Cell::from(if is_narrow { t.date.format("%m-%d").to_string() } else { t.date.to_string() }),
                Cell::from(account_name(view.accounts, &t.from_account_id).to_string()),
                Cell::from(account_name(view.accounts, &t.to_account_id).to_string()),
            ];
            if is_narrow {
                cells.push(Cell::from(format!("{} {from_currency}", format_amount_short(t.amount_from))));
                cells.push(Cell::from(format!("{} {to_currency}", format_amount_short(t.amount_to))));
            } else {
                cells.push(Cell::from(format_money(t.amount_from, from_currency)));
                cells.push(Cell::from(format_money(t.amount_to, to_currency)));
                cells.push(Cell::from(format_rate(t.rate(), from_currency, to_currency)));
                cells.push(Cell::from(t.comment.clone().unwrap_or_default()));
            }
            let is_selected = selected_index == Some(scroll_offset + p);
            Row::new(cells)
                .height(1)
                .style(if is_selected { highlight_style } else { Style::default().fg(Color::White) })
        })
        .collect();

    let title = format!("Transfers ({})", view.transfers.len());

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title))
        .style(Style::default().fg(Color::White));

    StatefulWidget::render(table, area, buf, &mut state.table_state);
}

fn render_button_bar(area: Rect, buf: &mut Buffer) {
    let available_width = area.width.saturating_sub(2) as usize;
    let button_width = 10;
    let left_padding = available_width.saturating_sub(button_width);

    let line = Line::from(vec![
        Span::raw(" ".repeat(left_padding)),
        Span::styled("[+ New]", Style::default().fg(Color::Green)),
    ]);

    let paragraph = Paragraph::new(vec![line])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

    Widget::render(paragraph, area, buf);
}
