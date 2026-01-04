//! Expense form as a StatefulWidget

use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, StatefulWidget, Widget, Wrap},
};
use chrono::NaiveDate;
use diesel::SqliteConnection;
use crate::models::{Category, Expense};
use super::category_form_widget::FormInputResult;

/// Fields in the expense form
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ExpenseFormField {
    #[default]
    Category,
    Amount,
    Date,
    Comment,
}

impl ExpenseFormField {
    pub fn next(&self) -> Self {
        match self {
            Self::Category => Self::Amount,
            Self::Amount => Self::Date,
            Self::Date => Self::Comment,
            Self::Comment => Self::Category,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Category => Self::Comment,
            Self::Amount => Self::Category,
            Self::Date => Self::Amount,
            Self::Comment => Self::Date,
        }
    }
}

/// State for the expense form
#[derive(Debug, Clone)]
pub struct ExpenseFormState {
    pub is_active: bool,
    pub current_field: ExpenseFormField,
    pub category_index: usize,
    pub amount: String,
    pub date: String,
    pub comment: String,
    pub error_message: Option<String>,
    categories: Vec<(Vec<u8>, String)>,
}

impl Default for ExpenseFormState {
    fn default() -> Self {
        Self::new()
    }
}

impl ExpenseFormState {
    pub fn new() -> Self {
        Self {
            is_active: false,
            current_field: ExpenseFormField::Category,
            category_index: 0,
            amount: String::new(),
            date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            comment: String::new(),
            error_message: None,
            categories: Vec::new(),
        }
    }

    pub fn open(&mut self, categories: &[Category]) {
        self.clear();
        self.set_categories(categories);
        self.is_active = true;
    }

    pub fn close(&mut self) {
        self.is_active = false;
        self.clear();
    }

    pub fn clear(&mut self) {
        self.current_field = ExpenseFormField::Category;
        self.category_index = 0;
        self.amount.clear();
        self.date = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.comment.clear();
        self.error_message = None;
    }

    pub fn set_categories(&mut self, categories: &[Category]) {
        self.categories = categories
            .iter()
            .map(|c| (c.id.clone(), c.name.clone()))
            .collect();
    }

    pub fn next_field(&mut self) {
        self.current_field = self.current_field.next();
    }

    pub fn prev_field(&mut self) {
        self.current_field = self.current_field.prev();
    }

    pub fn category_next(&mut self) {
        if self.category_index + 1 < self.categories.len() {
            self.category_index += 1;
        }
    }

    pub fn category_prev(&mut self) {
        self.category_index = self.category_index.saturating_sub(1);
    }

    pub fn push_char(&mut self, c: char) {
        if self.current_field != ExpenseFormField::Category {
            let input = match self.current_field {
                ExpenseFormField::Amount => &mut self.amount,
                ExpenseFormField::Date => &mut self.date,
                ExpenseFormField::Comment => &mut self.comment,
                ExpenseFormField::Category => return,
            };
            input.push(c);
            self.error_message = None;
        }
    }

    pub fn pop_char(&mut self) {
        if self.current_field != ExpenseFormField::Category {
            let input = match self.current_field {
                ExpenseFormField::Amount => &mut self.amount,
                ExpenseFormField::Date => &mut self.date,
                ExpenseFormField::Comment => &mut self.comment,
                ExpenseFormField::Category => return,
            };
            input.pop();
            self.error_message = None;
        }
    }

    pub fn set_error(&mut self, msg: String) {
        self.error_message = Some(msg);
    }

    pub fn selected_category_name(&self) -> Option<&str> {
        self.categories
            .get(self.category_index)
            .map(|(_, name)| name.as_str())
    }

    /// Submit the form - returns true if should move to next field, false if submitted
    pub fn submit(&mut self, conn: &mut SqliteConnection) -> Result<bool, String> {
        // If on category field, just move to next
        if self.current_field == ExpenseFormField::Category {
            self.next_field();
            return Ok(true);
        }

        // Validate
        if self.categories.is_empty() {
            return Err("No categories available".to_string());
        }

        if self.category_index >= self.categories.len() {
            return Err("No category selected".to_string());
        }

        let amount: f64 = self.amount.trim()
            .parse()
            .map_err(|_| "Invalid amount".to_string())?;

        if amount <= 0.0 {
            return Err("Amount must be greater than 0".to_string());
        }

        let date = NaiveDate::parse_from_str(self.date.trim(), "%Y-%m-%d")
            .map_err(|_| "Invalid date format (use YYYY-MM-DD)".to_string())?;

        let comment = self.comment.trim();
        let comment = if comment.is_empty() { None } else { Some(comment) };

        let category_id = &self.categories[self.category_index].0;

        Expense::create(conn, category_id, amount, comment, date)
            .map_err(|e| format!("Database error: {}", e))?;

        self.close();
        Ok(false)
    }

    /// Handle keyboard input, returns the result of the input handling
    pub fn handle_input(&mut self, key: KeyCode, conn: &mut SqliteConnection) -> FormInputResult {
        match key {
            KeyCode::Esc => {
                self.close();
                FormInputResult::Closed
            }
            KeyCode::Enter => {
                match self.submit(conn) {
                    Ok(false) => FormInputResult::SubmittedNeedsReload, // Submitted
                    Ok(true) => FormInputResult::Consumed, // Just moved to next field
                    Err(e) => {
                        self.set_error(e);
                        FormInputResult::Consumed
                    }
                }
            }
            KeyCode::Tab => {
                self.next_field();
                FormInputResult::Consumed
            }
            KeyCode::BackTab => {
                self.prev_field();
                FormInputResult::Consumed
            }
            KeyCode::Up => {
                if self.current_field == ExpenseFormField::Category {
                    self.category_prev();
                }
                FormInputResult::Consumed
            }
            KeyCode::Down => {
                if self.current_field == ExpenseFormField::Category {
                    self.category_next();
                }
                FormInputResult::Consumed
            }
            KeyCode::Char(c) => {
                self.push_char(c);
                FormInputResult::Consumed
            }
            KeyCode::Backspace => {
                self.pop_char();
                FormInputResult::Consumed
            }
            _ => FormInputResult::Consumed,
        }
    }
}

/// Widget for rendering the expense form
pub struct ExpenseFormWidget;

impl ExpenseFormWidget {
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

    fn highlight_style() -> Style {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    }

    fn normal_style() -> Style {
        Style::default().fg(Color::White)
    }
}

impl StatefulWidget for ExpenseFormWidget {
    type State = ExpenseFormState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if !state.is_active {
            return;
        }

        // Responsive sizing: use more screen space on narrow displays
        let (percent_x, percent_y) = if area.width < 60 {
            (95, 90)  // Almost full screen for mobile-like resolution
        } else if area.width < 80 {
            (85, 75)  // Larger popup for medium screens
        } else {
            (70, 60)  // Original size for wide screens
        };
        
        let popup_area = Self::centered_rect(percent_x, percent_y, area);

        Widget::render(Clear, popup_area, buf);

        let block = Block::default()
            .title("Create New Expense")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        Widget::render(block, popup_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),  // Category
                Constraint::Length(3),  // Amount
                Constraint::Length(3),  // Date
                Constraint::Length(3),  // Comment
                Constraint::Length(3),  // Instructions
                Constraint::Min(1),     // Error/Categories
            ])
            .split(inner);

        // Category selector
        let cat_style = if state.current_field == ExpenseFormField::Category {
            Self::highlight_style()
        } else {
            Self::normal_style()
        };
        let cat_text = state.selected_category_name().unwrap_or("No categories available");
        let cat_input = Paragraph::new(cat_text)
            .style(cat_style)
            .block(Block::default().borders(Borders::ALL).title("Category (↑/↓ to select)"));
        Widget::render(cat_input, chunks[0], buf);

        // Amount input
        let amt_style = if state.current_field == ExpenseFormField::Amount {
            Self::highlight_style()
        } else {
            Self::normal_style()
        };
        let amt_input = Paragraph::new(state.amount.as_str())
            .style(amt_style)
            .block(Block::default().borders(Borders::ALL).title("Amount"));
        Widget::render(amt_input, chunks[1], buf);

        // Date input
        let date_style = if state.current_field == ExpenseFormField::Date {
            Self::highlight_style()
        } else {
            Self::normal_style()
        };
        let date_input = Paragraph::new(state.date.as_str())
            .style(date_style)
            .block(Block::default().borders(Borders::ALL).title("Date (YYYY-MM-DD)"));
        Widget::render(date_input, chunks[2], buf);

        // Comment input
        let comment_style = if state.current_field == ExpenseFormField::Comment {
            Self::highlight_style()
        } else {
            Self::normal_style()
        };
        let comment_input = Paragraph::new(state.comment.as_str())
            .style(comment_style)
            .block(Block::default().borders(Borders::ALL).title("Comment (optional)"));
        Widget::render(comment_input, chunks[3], buf);

        // Instructions - responsive text based on width
        let instructions_text = if area.width < 60 {
            if state.current_field == ExpenseFormField::Category {
                "↑/↓:Select | Enter:Next | Esc:Back"
            } else {
                "Tab:Next | Enter:OK | Esc:Back"
            }
        } else if state.current_field == ExpenseFormField::Category {
            "↑/↓: Select category | Enter/Tab: Next field | Esc: Cancel"
        } else {
            "Tab: Next field | Enter: Submit | Esc: Cancel"
        };
        let instructions = Paragraph::new(instructions_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        Widget::render(instructions, chunks[4], buf);

        // Error message or categories list
        if let Some(error) = &state.error_message {
            let error_msg = Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            Widget::render(error_msg, chunks[5], buf);
        } else if state.current_field == ExpenseFormField::Category && !state.categories.is_empty() {
            let items: Vec<ListItem> = state.categories
                .iter()
                .enumerate()
                .map(|(idx, (_, name))| {
                    let style = if idx == state.category_index {
                        Self::highlight_style()
                    } else {
                        Self::normal_style()
                    };
                    ListItem::new(name.as_str()).style(style)
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Available Categories")
                        .border_style(Style::default().fg(Color::Cyan))
                );
            Widget::render(list, chunks[5], buf);
        }
    }
}

