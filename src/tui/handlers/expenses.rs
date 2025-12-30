use crossterm::event::{KeyCode, KeyEvent};
use diesel::SqliteConnection;
use crate::tui::app::{App, ActiveForm};
use crate::tui::forms::Form;
use super::{InputResult, ViewHandler};

/// Handles input for the Expenses view
pub struct ExpensesHandler;

impl ViewHandler for ExpensesHandler {
    fn handle_input(&self, app: &mut App, _conn: &mut SqliteConnection, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Char('n') | KeyCode::Char('N') => {
                app.expense_form.clear();
                app.expense_form.set_categories(&app.categories);
                app.active_form = ActiveForm::Expense;
                InputResult::Consumed
            }
            KeyCode::Left => {
                app.toggle_expense_sort(app.expense_sort_column.prev());
                InputResult::Consumed
            }
            KeyCode::Right => {
                app.toggle_expense_sort(app.expense_sort_column.next());
                InputResult::Consumed
            }
            _ => InputResult::NotConsumed,
        }
    }
}

