//! Accounts view as a StatefulWidget

use std::collections::HashMap;

use crossterm::event::{KeyCode, MouseEvent, MouseEventKind, MouseButton};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, StatefulWidget, Table, TableState, Widget},
};

use crate::ledger::AccountSummary;
use crate::models::Account;
use crate::tui::utils::format_money;
use super::expenses_widget::{ViewInputResult, ViewState};
use super::state::RowSelection;

/// State for the accounts table view
#[derive(Debug, Clone, Default)]
pub struct AccountsViewState {
    pub rows: RowSelection,
    pub table_state: TableState,
}

impl AccountsViewState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Index (in `AppState::accounts` order) of the selected account.
    pub fn selected(&self) -> Option<usize> {
        self.rows.selected
    }
}

impl ViewState for AccountsViewState {
    fn handle_input(&mut self, key: KeyCode) -> ViewInputResult {
        match key {
            KeyCode::Char('n') | KeyCode::Char('N') => ViewInputResult::OpenAccountForm,
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if self.rows.selected.is_some() {
                    ViewInputResult::OpenAccountEdit
                } else {
                    ViewInputResult::NotConsumed
                }
            }
            KeyCode::Enter => {
                if self.rows.selected.is_some() {
                    ViewInputResult::MakeCurrent
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
                if is_in_button_bar {
                    if x >= (area.x + area.width).saturating_sub(12) && x < (area.x + area.width).saturating_sub(1) {
                        return ViewInputResult::OpenAccountForm;
                    }
                    return ViewInputResult::NotConsumed;
                }
                if x >= area.x
                    && x < area.x + area.width
                    && self.rows.click_at(mouse.row, area.y, button_bar_start.saturating_sub(1))
                {
                    return ViewInputResult::Consumed;
                }
                ViewInputResult::NotConsumed
            }
            _ => ViewInputResult::NotConsumed,
        }
    }
}

/// Widget for rendering the accounts table
pub struct AccountsView<'a> {
    accounts: &'a [Account],
    current_account: &'a [u8],
    summaries: &'a HashMap<Vec<u8>, AccountSummary>,
}

impl<'a> AccountsView<'a> {
    pub fn new(
        accounts: &'a [Account],
        current_account: &'a [u8],
        summaries: &'a HashMap<Vec<u8>, AccountSummary>,
    ) -> Self {
        Self { accounts, current_account, summaries }
    }
}

impl<'a> StatefulWidget for AccountsView<'a> {
    type State = AccountsViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        state.rows.update_layout(self.accounts.len(), chunks[0].height);
        // Start with the current account selected so Enter/e work right away.
        if state.rows.selected.is_none()
            && let Some(i) = self.accounts.iter().position(|a| a.id == self.current_account)
        {
            state.rows.select(i);
            state.rows.update_layout(self.accounts.len(), chunks[0].height);
        }

        render_table(chunks[0], buf, state, &self);
        render_button_bar(chunks[1], buf);
    }
}

fn render_table(area: Rect, buf: &mut Buffer, state: &mut AccountsViewState, view: &AccountsView) {
    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let highlight_style = Style::default()
        .add_modifier(Modifier::BOLD)
        .bg(Color::DarkGray);

    let header = Row::new(vec![
        Cell::from("Name").style(header_style),
        Cell::from("Currency").style(header_style),
        Cell::from("In").style(header_style),
        Cell::from("Out").style(header_style),
        Cell::from("Balance").style(header_style),
    ])
    .height(1)
    .bottom_margin(1);

    let scroll_offset = state.rows.scroll_offset;
    let rows: Vec<Row> = view.accounts
        .iter()
        .skip(scroll_offset)
        .enumerate()
        .map(|(visible_idx, acct)| {
            let data_idx = scroll_offset + visible_idx;
            let is_current = acct.id == view.current_account;
            let is_selected = state.rows.selected == Some(data_idx);
            let mark = if is_current { "● " } else { "" };
            let name_cell = Cell::from(format!("{}{}", mark, acct.name))
                .style(Style::default().fg(if is_current { Color::Green } else { Color::White }));
            let summary = view.summaries.get(&acct.id).copied().unwrap_or_default();
            let bal_color = if summary.balance < 0.0 { Color::Red } else { Color::White };
            Row::new(vec![
                name_cell,
                Cell::from(acct.currency.clone()),
                Cell::from(format_money(summary.total_in, &acct.currency)),
                Cell::from(format_money(summary.total_out, &acct.currency)),
                Cell::from(format_money(summary.balance, &acct.currency)).style(Style::default().fg(bal_color)),
            ])
            .height(1)
            .style(if is_selected { highlight_style } else { Style::default() })
        })
        .collect();

    let widths = vec![
        Constraint::Min(20),
        Constraint::Min(8),
        Constraint::Length(16),
        Constraint::Length(16),
        Constraint::Min(16),
    ];

    let title = format!("Accounts ({})", view.accounts.len());

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
        Span::styled("[+ Create]", Style::default().fg(Color::Green)),
    ]);

    let paragraph = Paragraph::new(vec![line])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

    Widget::render(paragraph, area, buf);
}
