use crossterm::event::{KeyCode, KeyEvent};
use diesel::SqliteConnection;
use crate::tui::app::App;
use super::{InputResult, ViewHandler};

/// Handles input for the Top Ups view
pub struct TopUpsHandler;

impl ViewHandler for TopUpsHandler {
    fn handle_input(&self, app: &mut App, _conn: &mut SqliteConnection, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Left => {
                app.toggle_top_up_sort(app.top_up_sort_column.prev());
                InputResult::Consumed
            }
            KeyCode::Right => {
                app.toggle_top_up_sort(app.top_up_sort_column.next());
                InputResult::Consumed
            }
            _ => InputResult::NotConsumed,
        }
    }
}

