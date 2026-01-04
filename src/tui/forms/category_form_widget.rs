//! Category form as a StatefulWidget

use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph, StatefulWidget, Widget, Wrap},
};
use diesel::SqliteConnection;
use crate::models::Category;

/// Result of handling input in a form
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormInputResult {
    /// Input was consumed, no action needed
    Consumed,
    /// Form was submitted successfully, data needs reload
    SubmittedNeedsReload,
    /// Form was closed/cancelled
    Closed,
}

/// State for the category form
#[derive(Debug, Clone, Default)]
pub struct CategoryFormState {
    pub name: String,
    pub error_message: Option<String>,
    pub is_active: bool,
}

impl CategoryFormState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self) {
        self.clear();
        self.is_active = true;
    }

    pub fn close(&mut self) {
        self.is_active = false;
        self.clear();
    }

    pub fn clear(&mut self) {
        self.name.clear();
        self.error_message = None;
    }

    pub fn push_char(&mut self, c: char) {
        self.name.push(c);
        self.error_message = None;
    }

    pub fn pop_char(&mut self) {
        self.name.pop();
        self.error_message = None;
    }

    pub fn set_error(&mut self, msg: String) {
        self.error_message = Some(msg);
    }

    /// Submit the form - returns true if successful
    pub fn submit(&mut self, conn: &mut SqliteConnection) -> Result<(), String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Category name cannot be empty".to_string());
        }

        Category::create(conn, name)
            .map_err(|e| format!("Database error: {}", e))?;

        self.close();
        Ok(())
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
                    Ok(()) => FormInputResult::SubmittedNeedsReload,
                    Err(e) => {
                        self.set_error(e);
                        FormInputResult::Consumed
                    }
                }
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

/// Widget for rendering the category form
pub struct CategoryFormWidget;

impl CategoryFormWidget {
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

impl StatefulWidget for CategoryFormWidget {
    type State = CategoryFormState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if !state.is_active {
            return;
        }

        // Responsive sizing: use more screen space on narrow displays
        let (percent_x, percent_y) = if area.width < 60 {
            (95, 60)  // Almost full width for mobile-like resolution
        } else if area.width < 80 {
            (80, 50)  // Larger popup for medium screens
        } else {
            (60, 40)  // Original size for wide screens
        };
        
        let popup_area = Self::centered_rect(percent_x, percent_y, area);

        // Clear the area
        Widget::render(Clear, popup_area, buf);

        let block = Block::default()
            .title("Create New Category")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(popup_area);
        Widget::render(block, popup_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(inner);

        // Name input
        let input = Paragraph::new(state.name.as_str())
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title("Category Name"));
        Widget::render(input, chunks[0], buf);

        // Instructions - responsive text
        let instructions_text = if area.width < 60 {
            "Enter:OK | Esc:Back"
        } else {
            "Press Enter to submit, Esc to cancel"
        };
        let instructions = Paragraph::new(instructions_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        Widget::render(instructions, chunks[1], buf);

        // Error message
        if let Some(error) = &state.error_message {
            let error_msg = Paragraph::new(error.as_str())
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            Widget::render(error_msg, chunks[2], buf);
        }
    }
}

