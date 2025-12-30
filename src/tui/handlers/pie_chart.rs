use crossterm::event::{KeyCode, KeyEvent};
use diesel::SqliteConnection;
use crate::tui::app::App;
use super::{InputResult, ViewHandler};

/// Handles input for the Pie Chart view
pub struct PieChartHandler;

impl ViewHandler for PieChartHandler {
    fn handle_input(&self, app: &mut App, _conn: &mut SqliteConnection, key: KeyEvent) -> InputResult {
        match key.code {
            KeyCode::Char('m') | KeyCode::Char('M') => {
                app.toggle_pie_chart_mode();
                InputResult::Consumed
            }
            _ => InputResult::NotConsumed,
        }
    }
}

