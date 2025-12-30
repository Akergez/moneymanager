use crossterm::event::KeyEvent;
use diesel::SqliteConnection;
use crate::tui::app::App;
use super::{InputResult, ViewHandler};

/// Handles input for the Bar Chart view
pub struct BarChartHandler;

impl ViewHandler for BarChartHandler {
    fn handle_input(&self, _app: &mut App, _conn: &mut SqliteConnection, _key: KeyEvent) -> InputResult {
        // No view-specific handlers for this view currently
        InputResult::NotConsumed
    }
}

