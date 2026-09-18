//! Transfer form as a StatefulWidget (mirrors the expense form)

use chrono::NaiveDate;
use crossterm::event::{KeyCode, MouseEvent, MouseEventKind, MouseButton};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, StatefulWidget, Widget, Wrap},
};

use crate::ledger::format_rate_both;
use crate::models::{Account, Transfer};
use crate::store::Store;
use super::category_form_widget::{FormButtonAction, FormInputResult};

const DATE_FMT: &str = "%Y-%m-%d";

/// Fields in the transfer form
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TransferFormField {
    #[default]
    From,
    To,
    AmountFrom,
    AmountTo,
    Date,
    Comment,
}

impl TransferFormField {
    pub fn next(&self) -> Self {
        match self {
            Self::From => Self::To,
            Self::To => Self::AmountFrom,
            Self::AmountFrom => Self::AmountTo,
            Self::AmountTo => Self::Date,
            Self::Date => Self::Comment,
            Self::Comment => Self::From,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::From => Self::Comment,
            Self::To => Self::From,
            Self::AmountFrom => Self::To,
            Self::AmountTo => Self::AmountFrom,
            Self::Date => Self::AmountTo,
            Self::Comment => Self::Date,
        }
    }

    fn is_selector(&self) -> bool {
        matches!(self, Self::From | Self::To)
    }
}

/// State for the transfer form
#[derive(Debug, Clone, Default)]
pub struct TransferFormState {
    pub is_active: bool,
    pub current_field: TransferFormField,
    pub from_index: usize,
    pub to_index: usize,
    pub accounts: Vec<Account>,
    pub amount_from: String,
    pub amount_to: String,
    pub date: String,
    pub comment: String,
    pub error_message: Option<String>,
    pub pending_button_action: Option<FormButtonAction>,
    pub button_area: Option<Rect>,
}

impl TransferFormState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the form to create a transfer from `current_account`.
    pub fn open(&mut self, accounts: &[Account], current_account: &[u8], default_date: Option<NaiveDate>) {
        *self = Self::new();
        self.accounts = accounts.to_vec();
        self.from_index = self
            .accounts
            .iter()
            .position(|a| a.id == current_account)
            .unwrap_or(0);
        // To: prefer an account in another currency (the usual exchange case),
        // otherwise any other account.
        let from_currency = self.source_currency().to_string();
        self.to_index = (0..self.accounts.len())
            .filter(|&i| i != self.from_index)
            .find(|&i| self.accounts[i].currency != from_currency)
            .or_else(|| (0..self.accounts.len()).find(|&i| i != self.from_index))
            .unwrap_or(self.from_index);
        self.date = default_date
            .unwrap_or_else(|| chrono::Local::now().date_naive())
            .format(DATE_FMT)
            .to_string();
        self.is_active = true;
    }

    pub fn close(&mut self) {
        self.is_active = false;
        self.error_message = None;
        self.pending_button_action = None;
    }

    pub fn source_currency(&self) -> &str {
        self.accounts.get(self.from_index).map(|a| a.currency.as_str()).unwrap_or("")
    }

    pub fn target_currency(&self) -> &str {
        self.accounts.get(self.to_index).map(|a| a.currency.as_str()).unwrap_or("")
    }

    fn same_currency(&self) -> bool {
        self.source_currency() == self.target_currency()
    }

    /// Cycle the From account; To is moved off it if they collide.
    fn step_from(&mut self, forward: bool) {
        let n = self.accounts.len();
        if n < 2 {
            return;
        }
        self.from_index = if forward { (self.from_index + 1) % n } else { (self.from_index + n - 1) % n };
        if self.to_index == self.from_index {
            self.step_to(true);
        }
    }

    /// Cycle the To account, always skipping the From account.
    fn step_to(&mut self, forward: bool) {
        let n = self.accounts.len();
        if n < 2 {
            return;
        }
        let mut i = self.to_index;
        loop {
            i = if forward { (i + 1) % n } else { (i + n - 1) % n };
            if i != self.from_index {
                self.to_index = i;
                return;
            }
        }
    }

    fn parse_amount(s: &str) -> Option<f64> {
        s.trim().parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0)
    }

    /// Credited amount as it will be saved: an empty field means "same as
    /// debited" when both accounts share a currency.
    fn effective_amount_to(&self) -> &str {
        if self.amount_to.trim().is_empty() && self.same_currency() {
            &self.amount_from
        } else {
            &self.amount_to
        }
    }

    /// Live rate line, e.g. "1 USD = 92.35 RUB · 1 RUB = 0.0108 USD".
    pub fn rate_text(&self) -> Option<String> {
        let from = Self::parse_amount(&self.amount_from)?;
        let to = Self::parse_amount(self.effective_amount_to())?;
        Some(format_rate_both(to / from, self.source_currency(), self.target_currency()))
    }

    fn input_mut(&mut self) -> Option<&mut String> {
        match self.current_field {
            TransferFormField::AmountFrom => Some(&mut self.amount_from),
            TransferFormField::AmountTo => Some(&mut self.amount_to),
            TransferFormField::Date => Some(&mut self.date),
            TransferFormField::Comment => Some(&mut self.comment),
            TransferFormField::From | TransferFormField::To => None,
        }
    }

    fn push_char(&mut self, c: char) {
        let c = match self.current_field {
            TransferFormField::AmountFrom | TransferFormField::AmountTo => match c {
                '0'..='9' | '.' => c,
                ',' => '.',
                _ => return,
            },
            TransferFormField::Date if !(c.is_ascii_digit() || c == '-') => return,
            _ => c,
        };
        if let Some(input) = self.input_mut() {
            input.push(c);
            self.error_message = None;
        }
    }

    fn pop_char(&mut self) {
        if let Some(input) = self.input_mut() {
            input.pop();
            self.error_message = None;
        }
    }

    /// Validate and save. `Ok(true)` = just moved to the next field,
    /// `Ok(false)` = saved (needs reload).
    pub fn submit(&mut self, conn: &mut Store) -> Result<bool, String> {
        if self.current_field.is_selector() {
            self.current_field = self.current_field.next();
            return Ok(true);
        }
        self.save(conn).map(|()| false)
    }

    fn save(&mut self, conn: &mut Store) -> Result<(), String> {
        if self.accounts.len() < 2 || self.from_index == self.to_index {
            return Err("Choose two different accounts".to_string());
        }
        let amount_from = Self::parse_amount(&self.amount_from)
            .ok_or_else(|| "Debited amount must be a number greater than 0".to_string())?;
        let amount_to = Self::parse_amount(self.effective_amount_to())
            .ok_or_else(|| "Credited amount must be a number greater than 0".to_string())?;
        let date = NaiveDate::parse_from_str(self.date.trim(), DATE_FMT)
            .map_err(|_| "Invalid date format (use YYYY-MM-DD)".to_string())?;
        let comment = self.comment.trim();
        let comment = (!comment.is_empty()).then_some(comment);
        Transfer::create(
            conn,
            &self.accounts[self.from_index].id,
            &self.accounts[self.to_index].id,
            amount_from,
            amount_to,
            comment,
            date,
        )
        .map_err(|e| format!("Cannot save transfer: {e}"))?;
        self.close();
        Ok(())
    }

    fn result_of(&mut self, submitted: Result<bool, String>) -> FormInputResult {
        match submitted {
            Ok(false) => FormInputResult::SubmittedNeedsReload,
            Ok(true) => FormInputResult::Consumed,
            Err(e) => {
                self.error_message = Some(e);
                FormInputResult::Consumed
            }
        }
    }

    pub fn handle_input(&mut self, key: KeyCode, conn: &mut Store) -> FormInputResult {
        if let Some(action) = self.pending_button_action.take() {
            return match action {
                FormButtonAction::Confirm => {
                    let saved = self.save(conn).map(|()| false);
                    self.result_of(saved)
                }
                FormButtonAction::Cancel => {
                    self.close();
                    FormInputResult::Closed
                }
            };
        }
        match key {
            KeyCode::Esc => {
                self.close();
                return FormInputResult::Closed;
            }
            KeyCode::Enter => {
                let submitted = self.submit(conn);
                return self.result_of(submitted);
            }
            KeyCode::Tab => self.current_field = self.current_field.next(),
            KeyCode::BackTab => self.current_field = self.current_field.prev(),
            KeyCode::Up | KeyCode::Left if self.current_field == TransferFormField::From => self.step_from(false),
            KeyCode::Down | KeyCode::Right if self.current_field == TransferFormField::From => self.step_from(true),
            KeyCode::Up | KeyCode::Left if self.current_field == TransferFormField::To => self.step_to(false),
            KeyCode::Down | KeyCode::Right if self.current_field == TransferFormField::To => self.step_to(true),
            KeyCode::Up => self.current_field = self.current_field.prev(),
            KeyCode::Down => self.current_field = self.current_field.next(),
            KeyCode::Char(c) => self.push_char(c),
            KeyCode::Backspace => self.pop_char(),
            _ => {}
        }
        FormInputResult::Consumed
    }

    /// Returns true if a button was clicked (the caller then calls
    /// `handle_input` to act on it).
    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> bool {
        if !self.is_active {
            return false;
        }
        let (MouseEventKind::Down(MouseButton::Left), Some(area)) = (mouse.kind, self.button_area) else {
            return false;
        };
        if mouse.row < area.y || mouse.row >= area.y + area.height {
            return false;
        }
        // Right-aligned "[Cancel]  [Confirm]".
        let end = area.x + area.width;
        let confirm_start = end.saturating_sub(9);
        let cancel_end = confirm_start.saturating_sub(2);
        let cancel_start = cancel_end.saturating_sub(8);
        let x = mouse.column;
        if x >= confirm_start && x < end {
            self.pending_button_action = Some(FormButtonAction::Confirm);
            true
        } else if x >= cancel_start && x < cancel_end {
            self.pending_button_action = Some(FormButtonAction::Cancel);
            true
        } else {
            false
        }
    }
}

pub struct TransferFormWidget;

impl TransferFormWidget {
    pub fn new() -> Self {
        Self
    }

    fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

    fn field_style(state: &TransferFormState, field: TransferFormField) -> Style {
        if state.current_field == field {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        }
    }

    fn render_field(state: &TransferFormState, field: TransferFormField, title: String, value: String, area: Rect, buf: &mut Buffer) {
        let style = Self::field_style(state, field);
        let block = Block::default().borders(Borders::ALL).title(title).border_style(style);
        Widget::render(Paragraph::new(value).style(style).block(block), area, buf);
    }
}

impl StatefulWidget for TransferFormWidget {
    type State = TransferFormState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if !state.is_active {
            return;
        }

        let (percent_x, percent_y) = if area.width < 60 {
            (95, 95)
        } else if area.width < 80 {
            (85, 80)
        } else {
            (70, 70)
        };
        let popup_area = Self::centered_rect(percent_x, percent_y, area);
        Widget::render(Clear, popup_area, buf);

        let block = Block::default()
            .title("New Transfer")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(popup_area);
        Widget::render(block, popup_area, buf);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // From | To
                Constraint::Length(3), // Debited | Credited
                Constraint::Length(3), // Date | Comment
                Constraint::Length(1), // Rate
                Constraint::Length(1), // Instructions
                Constraint::Min(1),    // Error
                Constraint::Length(1), // Buttons
            ])
            .split(inner);
        let halves = |r: Rect| {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(r)
        };
        let accounts_row = halves(rows[0]);
        let amounts_row = halves(rows[1]);
        let date_row = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(20), Constraint::Min(10)])
            .split(rows[2]);

        let account_label = |i: usize| {
            state
                .accounts
                .get(i)
                .map(|a| format!("‹ {} ({}) ›", a.name, a.currency))
                .unwrap_or_default()
        };
        Self::render_field(state, TransferFormField::From, "From".into(), account_label(state.from_index), accounts_row[0], buf);
        Self::render_field(state, TransferFormField::To, "To".into(), account_label(state.to_index), accounts_row[1], buf);
        Self::render_field(
            state,
            TransferFormField::AmountFrom,
            format!("Debited ({})", state.source_currency()),
            state.amount_from.clone(),
            amounts_row[0],
            buf,
        );
        let credited_title = if state.same_currency() {
            format!("Credited ({}, empty = same)", state.target_currency())
        } else {
            format!("Credited ({})", state.target_currency())
        };
        Self::render_field(state, TransferFormField::AmountTo, credited_title, state.amount_to.clone(), amounts_row[1], buf);
        Self::render_field(state, TransferFormField::Date, "Date (YYYY-MM-DD)".into(), state.date.clone(), date_row[0], buf);
        Self::render_field(state, TransferFormField::Comment, "Comment (optional)".into(), state.comment.clone(), date_row[1], buf);

        let rate_line = Line::from(vec![
            Span::styled("Rate: ", Style::default().fg(Color::Gray)),
            Span::styled(
                state.rate_text().unwrap_or_else(|| "enter both amounts".to_string()),
                Style::default().fg(Color::Green),
            ),
        ]);
        Widget::render(Paragraph::new(rate_line), rows[3], buf);

        let instructions = if state.current_field.is_selector() {
            "←/→: Pick account | Tab: Next field | Esc: Cancel"
        } else {
            "Tab: Next field | Enter: Save | Esc: Cancel"
        };
        Widget::render(
            Paragraph::new(instructions).style(Style::default().fg(Color::Gray)).alignment(Alignment::Center),
            rows[4],
            buf,
        );

        if let Some(error) = &state.error_message {
            let error_msg = Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            Widget::render(error_msg, rows[5], buf);
        }

        // Buttons - right aligned, like the other forms.
        let button_area = rows[6];
        state.button_area = Some(button_area);
        let left_padding = (button_area.width as usize).saturating_sub(8 + 2 + 9);
        let buttons_line = Line::from(vec![
            Span::raw(" ".repeat(left_padding)),
            Span::styled("[Cancel]", Style::default().fg(Color::Red)),
            Span::raw("  "),
            Span::styled("[Confirm]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]);
        Widget::render(Paragraph::new(buttons_line), button_area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(n: u8, currency: &str) -> Account {
        Account { id: vec![n], name: format!("A{n}"), currency: currency.into(), opening_balance: 0.0 }
    }

    #[test]
    fn open_defaults_and_selectors_never_collide() {
        let accounts = vec![account(1, "RUB"), account(2, "RUB"), account(3, "USD")];
        let mut f = TransferFormState::new();
        f.open(&accounts, &[1], None);
        assert_eq!(f.from_index, 0);
        assert_eq!(f.to_index, 2, "prefers an account in another currency");

        for _ in 0..7 {
            f.step_from(true);
            assert_ne!(f.from_index, f.to_index);
        }
        for _ in 0..7 {
            f.step_to(false);
            assert_ne!(f.from_index, f.to_index);
        }
    }

    #[test]
    fn same_currency_credited_defaults_to_debited() {
        let accounts = vec![account(1, "RUB"), account(2, "RUB")];
        let mut f = TransferFormState::new();
        f.open(&accounts, &[1], None);
        f.amount_from = "150".into();
        assert_eq!(f.effective_amount_to(), "150");
        assert_eq!(f.rate_text().as_deref(), Some("1 RUB = 1.00 RUB · 1 RUB = 1.00 RUB"));

        let accounts = vec![account(1, "USD"), account(2, "RUB")];
        f.open(&accounts, &[1], None);
        f.amount_from = "100".into();
        assert_eq!(f.rate_text(), None, "different currencies need both amounts");
        f.amount_to = "9235".into();
        assert_eq!(f.rate_text().as_deref(), Some("1 USD = 92.35 RUB · 1 RUB = 0.0108 USD"));
    }
}
