use crossterm::event::KeyEvent;
use diesel::SqliteConnection;
use crate::tui::app::App;
use super::{InputResult, ViewHandler};

/// Handles input for the Top Up Categories view
pub struct TopUpCategoriesHandler;

impl ViewHandler for TopUpCategoriesHandler {
    fn handle_input(&self, _app: &mut App, _conn: &mut SqliteConnection, _key: KeyEvent) -> InputResult {
        // No view-specific handlers for this view currently
        InputResult::NotConsumed
    }
}

