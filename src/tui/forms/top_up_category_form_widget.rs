//! Top Up Category form as a StatefulWidget

use crossterm::event::{KeyCode, MouseEvent, MouseEventKind, MouseButton};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, StatefulWidget, Widget, Wrap},
};
use diesel::SqliteConnection;
use crate::models::TopUpCategory;
use super::category_form_widget::FormInputResult;

/// Button action for this form
#[derive(Debug, Clone, Copy)]
pub enum FormButtonAction {
    Confirm,
    Cancel,
}

/// State for the top up category form
#[derive(Debug, Clone, Default)]
pub struct TopUpCategoryFormState {
    pub name: String,
    pub error_message: Option<String>,
    pub is_active: bool,
    pub pending_button_action: Option<FormButtonAction>,
    pub button_area: Option<Rect>,
}

impl TopUpCategoryFormState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(&mut self) {
        self.clear();
        self.is_active = true;
        self.button_area = None;
    }

    pub fn close(&mut self) {
        self.is_active = false;
        self.clear();
    }

    pub fn clear(&mut self) {
        self.name.clear();
        self.error_message = None;
        self.pending_button_action = None;
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
            return Err("Top Up Category name cannot be empty".to_string());
        }

        TopUpCategory::create(conn, name)
            .map_err(|e| format!("Database error: {}", e))?;

        self.close();
        Ok(())
    }

    /// Handle keyboard input, returns the result of the input handling
    pub fn handle_input(&mut self, key: KeyCode, conn: &mut SqliteConnection) -> FormInputResult {
        // Process any pending button actions first
        if let Some(action) = self.pending_button_action.take() {
            match action {
                FormButtonAction::Confirm => {
                    return match self.submit(conn) {
                        Ok(()) => FormInputResult::SubmittedNeedsReload,
                        Err(e) => {
                            self.set_error(e);
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

    /// Handle mouse input
    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> bool {
        if !self.is_active {
            return false;
        }

        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            if let Some(button_area) = self.button_area {
                let x = mouse.column;
                let y = mouse.row;

                // Check if click is in button row
                if y >= button_area.y && y < button_area.y + button_area.height {
                    let content_end = button_area.x + button_area.width;
                    let confirm_start = content_end.saturating_sub(9);
                    let cancel_end = confirm_start.saturating_sub(2);
                    let cancel_start = cancel_end.saturating_sub(8);

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
        }
        false
    }
}

/// Widget for rendering the top up category form
pub struct TopUpCategoryFormWidget;

impl TopUpCategoryFormWidget {
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

impl StatefulWidget for TopUpCategoryFormWidget {
    type State = TopUpCategoryFormState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if !state.is_active {
            return;
        }

        // Responsive sizing: use more screen space on narrow displays
        let (percent_x, percent_y) = if area.width < 60 {
            (95, 70)  // Almost full width for mobile-like resolution
        } else if area.width < 80 {
            (80, 55)  // Larger popup for medium screens
        } else {
            (60, 45)  // Original size for wide screens
        };
        
        let popup_area = Self::centered_rect(percent_x, percent_y, area);

        // Clear the area
        Widget::render(Clear, popup_area, buf);

        let block = Block::default()
            .title("Create New Top Up Category")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));

        let inner = block.inner(popup_area);
        Widget::render(block, popup_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),  // Name input
                Constraint::Length(1),  // Instructions
                Constraint::Min(1),     // Error message
                Constraint::Length(2),  // Buttons
            ])
            .split(inner);

        // Save button area for mouse handling
        state.button_area = Some(chunks[3]);

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

        // Buttons - right aligned
        let button_area = chunks[3];
        let available_width = button_area.width as usize;
        let buttons_width = 8 + 2 + 9; // "[Cancel]" + "  " + "[Confirm]"
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

