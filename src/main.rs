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
use tui::{AppState, EventHandler, Event};

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
    let mut state = AppState::new(&mut conn)?;
    let event_handler = EventHandler::new();

    // Main loop
    let res = run_app(&mut terminal, &mut state, &mut conn, &event_handler);

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
    state: &mut AppState,
    conn: &mut SqliteConnection,
    event_handler: &EventHandler,
) -> io::Result<()> {
    while state.running {
        terminal.draw(|f| tui::ui::draw(f, state))?;

        match event_handler.next()? {
            Event::Key(key) => {
                state.handle_input(key.code, key.modifiers, conn);
            }
            Event::Tick => {}
        }
    }

    Ok(())
}


