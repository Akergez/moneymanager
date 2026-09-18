//! Account form as a StatefulWidget

use crossterm::event::{KeyCode, MouseEvent, MouseEventKind, MouseButton};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, StatefulWidget, Widget, Wrap},
};
use crate::store::Store;
use super::category_form_widget::{FormInputResult, FormButtonAction};

/// Fields in the account form
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AccountFormField {
    #[default]
    Name,
    Currency,
    Balance,
}

impl AccountFormField {
    pub fn next(&self) -> Self {
        match self {
            Self::Name => Self::Currency,
            Self::Currency => Self::Balance,
            Self::Balance => Self::Name,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Name => Self::Balance,
            Self::Currency => Self::Name,
            Self::Balance => Self::Currency,
        }
    }
}

/// State for the account form
#[derive(Debug, Clone)]
pub struct AccountFormState {
    pub is_active: bool,
    pub current_field: AccountFormField,
    pub name: String,
    pub currency: String,
    pub opening_balance: String,
    /// `Some(id)` = editing an existing account, `None` = creating one.
    pub editing_id: Option<Vec<u8>>,
    pub error_message: Option<String>,
    pub pending_button_action: Option<FormButtonAction>,
    pub button_area: Option<Rect>,
}

impl Default for AccountFormState {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountFormState {
    pub fn new() -> Self {
        Self {
            is_active: false,
            current_field: AccountFormField::Name,
            name: String::new(),
            currency: String::new(),
            opening_balance: String::new(),
            editing_id: None,
            error_message: None,
            pending_button_action: None,
            button_area: None,
        }
    }

    /// Open the form to create a brand-new account.
    pub fn open_create(&mut self) {
        self.clear();
        self.opening_balance = "0.00".to_string();
        self.is_active = true;
        self.current_field = AccountFormField::Name;
    }

    /// Open the form to edit the account with the given fields.
    pub fn open_edit(
        &mut self,
        id: &[u8],
        name: &str,
        currency: &str,
        opening_balance: f64,
    ) {
        self.name = name.to_string();
        self.currency = currency.to_string().to_uppercase();
        self.opening_balance = format!("{}", opening_balance);
        self.editing_id = Some(id.to_vec());
        self.error_message = None;
        self.pending_button_action = None;
        self.is_active = true;
        self.current_field = AccountFormField::Name;
        self.button_area = None;
    }

    pub fn close(&mut self) {
        self.is_active = false;
        self.clear();
    }

    pub fn clear(&mut self) {
        self.name.clear();
        self.currency.clear();
        self.opening_balance.clear();
        self.editing_id = None;
        self.error_message = None;
        self.pending_button_action = None;
        self.button_area = None;
    }

    /// Validate the currency field: exactly 3 uppercase latin letters.
    fn normalize_currency(&mut self) {
        let cleaned: String = self
            .currency
            .chars()
            .filter(|c| c.is_ascii_alphabetic())
            .collect();
        let cleaned = cleaned.chars().take(3).collect::<String>().to_uppercase();
        self.currency = cleaned;
    }

    /// Submit the form - returns true if successful
    pub fn submit(&mut self, conn: &mut Store) -> Result<(), String> {
        self.normalize_currency();
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Account name cannot be empty".to_string());
        }
        if self.currency.len() != 3 {
            return Err("Currency must be 3 letters (e.g. RUB, USD)".to_string());
        }

        let balance_text = self.opening_balance.trim();
        let opening_balance = if balance_text.is_empty() {
            0.0
        } else {
            balance_text
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("Invalid opening balance: {balance_text:?}"))?
        };

        match self.editing_id.as_ref() {
            Some(id) => {
                crate::models::Account::update(conn, id, name, &self.currency, opening_balance)
                    .map_err(|e| format!("Database error: {e}"))?;
            }
            None => {
                crate::models::Account::create(conn, name, &self.currency, opening_balance)
                    .map_err(|e| format!("Database error: {e}"))?;
            }
        }

        self.close();
        Ok(())
    }

    /// Handle keyboard input, returns the result of the input handling
    pub fn handle_input(&mut self, key: KeyCode, conn: &mut Store) -> FormInputResult {
        // Process any pending button actions first
        if let Some(action) = self.pending_button_action.take() {
            match action {
                FormButtonAction::Confirm => {
                    return match self.submit(conn) {
                        Ok(()) => FormInputResult::SubmittedNeedsReload,
                        Err(e) => {
                            self.error_message = Some(e);
                            FormInputResult::Consumed
                        }
                    };
                }
                FormButtonAction::Cancel => {
                    self.close();
                    return FormInputResult::Closed;
                }
            }
        }

        match key {
            KeyCode::Esc => {
                self.close();
                FormInputResult::Closed
            }
            KeyCode::Enter => match self.submit(conn) {
                Ok(()) => FormInputResult::SubmittedNeedsReload,
                Err(e) => {
                    self.error_message = Some(e);
                    FormInputResult::Consumed
                }
            },
            KeyCode::Tab => {
                self.current_field = self.current_field.next();
                FormInputResult::Consumed
            }
            KeyCode::BackTab => {
                self.current_field = self.current_field.prev();
                FormInputResult::Consumed
            }
            KeyCode::Char(c) => {
                self.append_char(c);
                FormInputResult::Consumed
            }
            KeyCode::Backspace => {
                self.backspace();
                FormInputResult::Consumed
            }
            _ => FormInputResult::Consumed,
        }
    }

    fn append_char(&mut self, c: char) {
        self.error_message = None;
        match self.current_field {
            // Names may contain spaces ("Card USD"); surrounding ones are trimmed on save.
            AccountFormField::Name => self.name.push(c),
            AccountFormField::Currency => {
                if c.is_ascii_alphabetic() && self.currency.len() < 3 {
                    self.currency.push(c.to_ascii_uppercase());
                }
            }
            AccountFormField::Balance => match c {
                '0'..='9' | '.' | '-' => self.opening_balance.push(c),
                ',' => self.opening_balance.push('.'),
                _ => {}
            },
        }
    }

    fn backspace(&mut self) {
        self.error_message = None;
        match self.current_field {
            AccountFormField::Name => {
                self.name.pop();
            }
            AccountFormField::Currency => {
                self.currency.pop();
            }
            AccountFormField::Balance => {
                self.opening_balance.pop();
            }
        }
    }

    /// Handle mouse input
    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> bool {
        if !self.is_active {
            return false;
        }

        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind
            && let Some(button_area) = self.button_area
        {
            let x = mouse.column;
            let y = mouse.row;

            // Check if click is in button row
            if y >= button_area.y && y < button_area.y + button_area.height {
                let content_end = button_area.x + button_area.width;
                let confirm_start = content_end.saturating_sub(9); // "[Confirm]" = 9 chars
                let cancel_end = confirm_start.saturating_sub(2);
                let cancel_start = cancel_end.saturating_sub(8); // "[Cancel]" = 8 chars

                if x >= confirm_start && x < content_end {
                    self.pending_button_action = Some(FormButtonAction::Confirm);
                    return true;
                }
                if x >= cancel_start && x < cancel_end {
                    self.pending_button_action = Some(FormButtonAction::Cancel);
                    return true;
                }
            }
        }
        false
    }
}

/// Widget for rendering the account form
pub struct AccountFormWidget;

impl AccountFormWidget {
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
}

impl StatefulWidget for AccountFormWidget {
    type State = AccountFormState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if !state.is_active {
            return;
        }

        let (percent_x, percent_y) = if area.width < 60 {
            (95, 75)
        } else if area.width < 80 {
            (80, 60)
        } else {
            (65, 55)
        };

        let popup_area = Self::centered_rect(percent_x, percent_y, area);
        Widget::render(Clear, popup_area, buf);

        let title = match state.editing_id {
            Some(_) => "Edit Account",
            None => "Create New Account",
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        Widget::render(block, popup_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),  // Name
                Constraint::Length(3),  // Currency
                Constraint::Length(3),  // Balance
                Constraint::Length(1),  // Instructions
                Constraint::Min(1),     // Error
                Constraint::Length(2),  // Buttons
            ])
            .split(inner);

        state.button_area = Some(chunks[5]);

        let name = if state.current_field == AccountFormField::Name {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };
        let currency = if state.current_field == AccountFormField::Currency {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };
        let balance = if state.current_field == AccountFormField::Balance {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };

        Widget::render(
            Paragraph::new(state.name.as_str()).style(name).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Name"),
            ),
            chunks[0],
            buf,
        );
        Widget::render(
            Paragraph::new(state.currency.as_str()).style(currency).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Currency (RUB/USD/EUR)"),
            ),
            chunks[1],
            buf,
        );
        Widget::render(
            Paragraph::new(state.opening_balance.as_str()).style(balance).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Opening Balance"),
            ),
            chunks[2],
            buf,
        );

        let instructions = Paragraph::new("Tab:next field | Esc:Back | Enter:OK")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        Widget::render(instructions, chunks[3], buf);

        if let Some(error) = &state.error_message {
            let error_msg = Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            Widget::render(error_msg, chunks[4], buf);
        }

        let button_area = chunks[5];
        let available_width = button_area.width as usize;
        let buttons_width = 8 + 2 + 9;
        let left_padding = available_width.saturating_sub(buttons_width);

        let buttons_line = Line::from(vec![
            Span::raw(" ".repeat(left_padding)),
            Span::styled("[Cancel]", Style::default().fg(Color::Red)),
            Span::raw("  "),
            Span::styled("[Confirm]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]);

        let buttons = Paragraph::new(vec![buttons_line]);
        Widget::render(buttons, button_area, buf);
    }
}
