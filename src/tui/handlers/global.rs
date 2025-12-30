use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use diesel::SqliteConnection;
use crate::tui::app::{App, Tab};
use super::{InputResult, ViewHandler};

/// Handles global input that applies across all views
pub struct GlobalHandler;

impl ViewHandler for GlobalHandler {
    fn handle_input(&self, app: &mut App, conn: &mut SqliteConnection, key: KeyEvent) -> InputResult {
        match key.code {
            // Quit
            KeyCode::Char('q') | KeyCode::Char('Q') => InputResult::Quit,
            KeyCode::Char('c') | KeyCode::Char('C') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                InputResult::Quit
            }

            // Tab navigation
            KeyCode::Tab => {
                app.next_tab();
                InputResult::Consumed
            }
            KeyCode::BackTab => {
                app.previous_tab();
                InputResult::Consumed
            }

            // Direct tab selection
            KeyCode::Char('1') => {
                app.current_tab = Tab::ExpenseCategories;
                app.scroll_offset = 0;
                InputResult::Consumed
            }
            KeyCode::Char('2') => {
                app.current_tab = Tab::Expenses;
                app.scroll_offset = 0;
                InputResult::Consumed
            }
            KeyCode::Char('3') => {
                app.current_tab = Tab::TopUpCategories;
                app.scroll_offset = 0;
                InputResult::Consumed
            }
            KeyCode::Char('4') => {
                app.current_tab = Tab::TopUps;
                app.scroll_offset = 0;
                InputResult::Consumed
            }
            KeyCode::Char('5') => {
                app.current_tab = Tab::ExpensePieChart;
                app.scroll_offset = 0;
                InputResult::Consumed
            }
            KeyCode::Char('6') => {
                app.current_tab = Tab::ExpenseBarChart;
                app.scroll_offset = 0;
                InputResult::Consumed
            }

            // Reload data
            KeyCode::Char('r') | KeyCode::Char('R') => {
                app.reload_data(conn).ok();
                InputResult::Consumed
            }

            // Scrolling
            KeyCode::Up => {
                app.scroll_up();
                InputResult::Consumed
            }
            KeyCode::Down => {
                app.scroll_down();
                InputResult::Consumed
            }
            KeyCode::PageUp => {
                app.page_up();
                InputResult::Consumed
            }
            KeyCode::PageDown => {
                app.page_down();
                InputResult::Consumed
            }

            _ => InputResult::NotConsumed,
        }
    }
}

