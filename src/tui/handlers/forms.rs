use crossterm::event::{KeyCode, KeyEvent};
use diesel::SqliteConnection;
use crate::tui::app::{App, ActiveForm};
use crate::tui::forms::{Form, FormResult};
use super::{InputResult, ViewHandler};

/// Handles input when the app is in form input mode
pub struct FormHandler;

impl ViewHandler for FormHandler {
    fn handle_input(&self, app: &mut App, conn: &mut SqliteConnection, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Esc => {
                self.cancel(app);
                InputResult::Consumed
            }
            KeyCode::Enter => {
                self.submit(app, conn);
                InputResult::Consumed
            }
            KeyCode::Tab => {
                self.next_field(app);
                InputResult::Consumed
            }
            KeyCode::BackTab => {
                self.prev_field(app);
                InputResult::Consumed
            }
            KeyCode::Up => {
                self.handle_up(app);
                InputResult::Consumed
            }
            KeyCode::Down => {
                self.handle_down(app);
                InputResult::Consumed
            }
            KeyCode::Char(c) => {
                self.push_char(app, c);
                InputResult::Consumed
            }
            KeyCode::Backspace => {
                self.pop_char(app);
                InputResult::Consumed
            }
            _ => InputResult::Consumed, // Consume all input in form mode
        }
    }
}

impl FormHandler {
    fn cancel(&self, app: &mut App) {
        match app.active_form {
            ActiveForm::Category => app.category_form.clear(),
            ActiveForm::Expense => app.expense_form.clear(),
            ActiveForm::None => {}
        }
        app.active_form = ActiveForm::None;
    }

    fn submit(&self, app: &mut App, conn: &mut SqliteConnection) {
        let result = match app.active_form {
            ActiveForm::Category => app.category_form.submit(conn),
            ActiveForm::Expense => app.expense_form.submit(conn),
            ActiveForm::None => return,
        };

        match result {
            Ok(FormResult::Submitted) => {
                app.active_form = ActiveForm::None;
                // Reload data after successful submission
                let _ = app.reload_data(conn);
            }
            Ok(FormResult::FieldChanged) | Ok(FormResult::None) => {}
            Err(error) => {
                match app.active_form {
                    ActiveForm::Category => app.category_form.set_error(error),
                    ActiveForm::Expense => app.expense_form.set_error(error),
                    ActiveForm::None => {}
                }
            }
        }
    }

    fn next_field(&self, app: &mut App) {
        match app.active_form {
            ActiveForm::Category => app.category_form.next_field(),
            ActiveForm::Expense => app.expense_form.next_field(),
            ActiveForm::None => {}
        }
    }

    fn prev_field(&self, app: &mut App) {
        match app.active_form {
            ActiveForm::Category => app.category_form.prev_field(),
            ActiveForm::Expense => app.expense_form.prev_field(),
            ActiveForm::None => {}
        }
    }

    fn handle_up(&self, app: &mut App) {
        if app.active_form == ActiveForm::Expense && app.expense_form.is_on_category_field() {
            app.expense_form.category_prev();
        }
    }

    fn handle_down(&self, app: &mut App) {
        if app.active_form == ActiveForm::Expense && app.expense_form.is_on_category_field() {
            app.expense_form.category_next();
        }
    }

    fn push_char(&self, app: &mut App, c: char) {
        match app.active_form {
            ActiveForm::Category => app.category_form.push_char(c),
            ActiveForm::Expense => app.expense_form.push_char(c),
            ActiveForm::None => {}
        }
    }

    fn pop_char(&self, app: &mut App) {
        match app.active_form {
            ActiveForm::Category => app.category_form.pop_char(),
            ActiveForm::Expense => app.expense_form.pop_char(),
            ActiveForm::None => {}
        }
    }
}

