mod models;
mod schema;
mod services;
mod tui;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use std::path::PathBuf;
use std::io;
use clap::Parser;
use crossterm::{
    execute,
    event::EnableMouseCapture,
    event::DisableMouseCapture,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui::{AppState, EventHandler, Event};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// A terminal-based money management application
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the SQLite database file
    #[arg(short, long, default_value = "money_manager.db")]
    database: PathBuf,
}

// Establish database connection
pub fn establish_connection(database_path: &PathBuf) -> SqliteConnection {
    let database_url = database_path.to_string_lossy();
    SqliteConnection::establish(&database_url)
        .expect(&format!("Error connecting to {}", database_url))
}

fn run_migrations(conn: &mut SqliteConnection) {
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run database migrations");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut conn = establish_connection(&args.database);

    // Run migrations (creates tables if db is new)
    run_migrations(&mut conn);

    let mut state = AppState::new(&mut conn)?;
    let event_handler = EventHandler::new();

    // Main loop
    let res = run_app(&mut terminal, &mut state, &mut conn, &event_handler);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
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
        let frame_area = terminal.get_frame().area();
        terminal.draw(|f| tui::ui::draw(f, state))?;

        match event_handler.next()? {
            Event::Key(key) => {
                state.handle_input(key.code, key.modifiers, conn);
            }
            Event::Mouse(mouse) => {
                state.handle_mouse(mouse, frame_area);
            }
            Event::Tick => {}
        }
    }

    Ok(())
}


