pub mod global;
pub mod expenses;
pub mod expense_categories;
pub mod top_ups;
pub mod top_up_categories;
pub mod pie_chart;
pub mod bar_chart;
pub mod forms;

use crossterm::event::KeyEvent;
use diesel::SqliteConnection;
use crate::tui::app::App;

/// Result of handling an input event
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputResult {
    /// Input was consumed and handled
    Consumed,
    /// Input was not handled, pass to next handler
    NotConsumed,
    /// Request to quit the application
    Quit,
}

/// Trait for view-specific input handlers
pub trait ViewHandler {
    fn handle_input(&self, app: &mut App, conn: &mut SqliteConnection, key: KeyEvent) -> InputResult;
}

pub use global::GlobalHandler;
pub use expenses::ExpensesHandler;
pub use expense_categories::ExpenseCategoriesHandler;
pub use top_ups::TopUpsHandler;
pub use top_up_categories::TopUpCategoriesHandler;
pub use pie_chart::PieChartHandler;
pub use bar_chart::BarChartHandler;
pub use forms::FormHandler;

