use crossterm::event::{KeyCode, MouseEvent, MouseEventKind, MouseButton};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, StatefulWidget, Widget, Wrap},
};

use crate::tui::forms::category_form_widget::{FormButtonAction, FormInputResult};

/// Type of record being deleted (used by the caller to perform the actual delete).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordType {
    Expense,
    TopUp,
    Transfer,
}

const CANCEL_LABEL: &str = "[Cancel]";
const DELETE_LABEL: &str = "[Delete]";
const BUTTON_GAP: u16 = 2;

/// State for the delete-confirmation dialog.
#[derive(Debug, Clone)]
pub struct ConfirmDialogState {
    pub is_active: bool,
    pub record_type: RecordType,
    pub record_id: Vec<u8>,
    /// Question on the first line, e.g. "Delete expense?".
    pub title: String,
    /// Details of the record (and its consequences), wrapped below the title.
    pub description: String,
    /// Which button Enter activates; ←/→/Tab move it.
    pub focus: FormButtonAction,
    /// Set by a mouse click on a button; consumed by the next `handle_input`.
    pub pending_button_action: Option<FormButtonAction>,
    pub button_area: Option<Rect>,
}

impl Default for ConfirmDialogState {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfirmDialogState {
    pub fn new() -> Self {
        Self {
            is_active: false,
            record_type: RecordType::Expense,
            record_id: Vec::new(),
            title: String::new(),
            description: String::new(),
            focus: FormButtonAction::Confirm,
            pending_button_action: None,
            button_area: None,
        }
    }

    /// Open the dialog to confirm deletion of the given record.
    pub fn open(&mut self, record_type: RecordType, record_id: Vec<u8>, title: String, description: String) {
        self.is_active = true;
        self.record_type = record_type;
        self.record_id = record_id;
        self.title = title;
        self.description = description;
        self.focus = FormButtonAction::Confirm;
        self.pending_button_action = None;
        self.button_area = None;
    }

    pub fn close(&mut self) {
        *self = Self::new();
    }

    fn finish(&mut self, action: FormButtonAction) -> FormInputResult {
        self.close();
        match action {
            FormButtonAction::Confirm => FormInputResult::SubmittedNeedsReload,
            FormButtonAction::Cancel => FormInputResult::Closed,
        }
    }

    /// Handle keyboard input. `SubmittedNeedsReload` means "delete confirmed".
    ///
    /// y/Enter-on-[Delete] confirm; n/Esc/Enter-on-[Cancel] cancel; ←/→/Tab
    /// only move the focus and never act on their own.
    pub fn handle_input(&mut self, key: KeyCode) -> FormInputResult {
        if let Some(action) = self.pending_button_action.take() {
            return self.finish(action);
        }

        match key {
            KeyCode::Char('y') | KeyCode::Char('Y') => self.finish(FormButtonAction::Confirm),
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => self.finish(FormButtonAction::Cancel),
            KeyCode::Enter => self.finish(self.focus),
            KeyCode::Left | KeyCode::Right | KeyCode::Tab | KeyCode::BackTab => {
                self.focus = match self.focus {
                    FormButtonAction::Confirm => FormButtonAction::Cancel,
                    FormButtonAction::Cancel => FormButtonAction::Confirm,
                };
                FormInputResult::Consumed
            }
            _ => FormInputResult::Consumed,
        }
    }

    /// Handle mouse input. Returns true if a button was clicked (the caller then
    /// calls `handle_input` to act on it).
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
        let cancel_start = area.x;
        let cancel_end = cancel_start + CANCEL_LABEL.len() as u16;
        let delete_start = cancel_end + BUTTON_GAP;
        let delete_end = delete_start + DELETE_LABEL.len() as u16;
        let action = if (cancel_start..cancel_end).contains(&mouse.column) {
            FormButtonAction::Cancel
        } else if (delete_start..delete_end).contains(&mouse.column) {
            FormButtonAction::Confirm
        } else {
            return false;
        };
        self.pending_button_action = Some(action);
        true
    }
}

/// Widget for rendering the delete-confirmation dialog.
pub struct ConfirmDialog;

impl ConfirmDialog {
    pub fn new() -> Self {
        Self
    }
}

impl StatefulWidget for ConfirmDialog {
    type State = ConfirmDialogState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if !state.is_active {
            return;
        }

        // A fixed-size box, centered and clamped to the screen.
        let width = area.width.min(72);
        let height = area.height.min(10);
        let dialog_area = Rect {
            x: area.x + (area.width - width) / 2,
            y: area.y + (area.height - height) / 2,
            width,
            height,
        };
        Widget::render(Clear, dialog_area, buf);

        let block = Block::default()
            .title("Confirm delete")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta));
        let inner = block.inner(dialog_area);
        Widget::render(block, dialog_area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .horizontal_margin(1)
            .constraints([
                Constraint::Length(1), // title
                Constraint::Min(1),    // description
                Constraint::Length(1), // buttons
            ])
            .split(inner);

        let title = Paragraph::new(state.title.as_str())
            .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));
        Widget::render(title, chunks[0], buf);

        let desc = Paragraph::new(state.description.as_str())
            .style(Style::default().fg(Color::Yellow))
            .wrap(Wrap { trim: true });
        Widget::render(desc, chunks[1], buf);

        state.button_area = Some(chunks[2]);
        let button_style = |action: FormButtonAction, color: Color| {
            let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
            if state.focus == action { style.add_modifier(Modifier::REVERSED) } else { style }
        };
        let buttons = Line::from(vec![
            Span::styled(CANCEL_LABEL, button_style(FormButtonAction::Cancel, Color::Green)),
            Span::raw(" ".repeat(BUTTON_GAP as usize)),
            Span::styled(DELETE_LABEL, button_style(FormButtonAction::Confirm, Color::Red)),
        ]);
        Widget::render(Paragraph::new(buttons), chunks[2], buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open() -> ConfirmDialogState {
        let mut d = ConfirmDialogState::new();
        d.open(RecordType::Expense, vec![1], "Delete?".into(), String::new());
        d
    }

    #[test]
    fn arrows_only_move_focus_and_esc_always_cancels() {
        let mut d = open();
        assert_eq!(d.handle_input(KeyCode::Right), FormInputResult::Consumed);
        assert!(d.is_active);
        assert_eq!(d.handle_input(KeyCode::Esc), FormInputResult::Closed);
        assert!(!d.is_active);
    }

    #[test]
    fn enter_activates_the_focused_button() {
        let mut d = open();
        assert_eq!(d.handle_input(KeyCode::Enter), FormInputResult::SubmittedNeedsReload);

        let mut d = open();
        d.handle_input(KeyCode::Left);
        assert_eq!(d.focus, FormButtonAction::Cancel);
        assert_eq!(d.handle_input(KeyCode::Enter), FormInputResult::Closed);

        let mut d = open();
        d.handle_input(KeyCode::Left);
        assert_eq!(d.handle_input(KeyCode::Char('y')), FormInputResult::SubmittedNeedsReload);
    }
}
