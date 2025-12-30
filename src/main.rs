mod models;
mod schema;
mod services;
mod tui;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use std::env;
use std::io;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui::{App, EventHandler, Event};
use tui::app::{ActiveForm, Tab};
use tui::handlers::{
    InputResult, ViewHandler,
    GlobalHandler, FormHandler,
    ExpensesHandler, ExpenseCategoriesHandler,
    TopUpsHandler, TopUpCategoriesHandler,
    PieChartHandler, BarChartHandler,
};

// Establish database connection
pub fn establish_connection() -> SqliteConnection {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| "money_manager.db".to_string());
    SqliteConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut conn = establish_connection();
    let mut app = App::new(&mut conn)?;
    let event_handler = EventHandler::new();

    // Main loop
    let res = run_app(&mut terminal, &mut app, &mut conn, &event_handler);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    conn: &mut SqliteConnection,
    event_handler: &EventHandler,
) -> io::Result<()> {
    // Initialize handlers
    let global_handler = GlobalHandler;
    let form_handler = FormHandler;
    let expenses_handler = ExpensesHandler;
    let expense_categories_handler = ExpenseCategoriesHandler;
    let top_ups_handler = TopUpsHandler;
    let top_up_categories_handler = TopUpCategoriesHandler;
    let pie_chart_handler = PieChartHandler;
    let bar_chart_handler = BarChartHandler;

    while app.running {
        terminal.draw(|f| tui::ui::draw(f, app))?;

        match event_handler.next()? {
            Event::Key(key) => {
                // Handle form input mode first
                if app.active_form != ActiveForm::None {
                    form_handler.handle_input(app, conn, key);
                    continue;
                }

                // Try view-specific handler first
                let view_result = match app.current_tab {
                    Tab::Expenses => expenses_handler.handle_input(app, conn, key),
                    Tab::ExpenseCategories => expense_categories_handler.handle_input(app, conn, key),
                    Tab::TopUps => top_ups_handler.handle_input(app, conn, key),
                    Tab::TopUpCategories => top_up_categories_handler.handle_input(app, conn, key),
                    Tab::ExpensePieChart => pie_chart_handler.handle_input(app, conn, key),
                    Tab::ExpenseBarChart => bar_chart_handler.handle_input(app, conn, key),
                };

                // If view handler consumed the input, continue to next event
                if view_result == InputResult::Consumed {
                    continue;
                }

                // Fall back to global handler
                if global_handler.handle_input(app, conn, key) == InputResult::Quit {
                    app.quit();
                }
            }
            Event::Tick => {}
        }
    }

    Ok(())
}

