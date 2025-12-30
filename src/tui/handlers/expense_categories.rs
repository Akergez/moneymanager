use crossterm::event::{KeyCode, KeyEvent};
use diesel::SqliteConnection;
use crate::tui::app::{App, ActiveForm};
use crate::tui::forms::Form;
use super::{InputResult, ViewHandler};

/// Handles input for the Expense Categories view
pub struct ExpenseCategoriesHandler;

impl ViewHandler for ExpenseCategoriesHandler {
    fn handle_input(&self, app: &mut App, _conn: &mut SqliteConnection, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Char('n') | KeyCode::Char('N') => {
                app.category_form.clear();
                app.active_form = ActiveForm::Category;
                InputResult::Consumed
            }
            _ => InputResult::NotConsumed,
        }
    }
}

