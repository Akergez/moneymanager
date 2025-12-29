mod models;
mod schema;
mod services;
mod tui;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use std::env;
use std::io;
use crossterm::{
    event::{KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tui::{App, EventHandler, Event};

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
    while app.running {
        terminal.draw(|f| tui::ui::draw(f, app))?;

        match event_handler.next()? {
            Event::Key(key) => {
                // Handle input mode separately
                if app.input_mode != tui::app::InputMode::Normal {
                    handle_input_mode(app, conn, key);
                    continue;
                }

                // Normal mode key handling
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => app.quit(),
                    KeyCode::Char('c') | KeyCode::Char('C') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.quit()
                    }
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        match app.current_tab {
                            tui::app::Tab::ExpenseCategories => app.start_creating_category(),
                            tui::app::Tab::Expenses => app.start_creating_expense(),
                            _ => {}
                        }
                    }
                    KeyCode::Tab => app.next_tab(),
                    KeyCode::BackTab => app.previous_tab(),
                    KeyCode::Char('1') => app.current_tab = tui::app::Tab::ExpenseCategories,
                    KeyCode::Char('2') => app.current_tab = tui::app::Tab::Expenses,
                    KeyCode::Char('3') => app.current_tab = tui::app::Tab::TopUpCategories,
                    KeyCode::Char('4') => app.current_tab = tui::app::Tab::TopUps,
                    KeyCode::Char('5') => app.current_tab = tui::app::Tab::ExpensePieChart,
                    KeyCode::Char('6') => app.current_tab = tui::app::Tab::ExpenseBarChart,
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        app.reload_data(conn).ok();
                    }
                    KeyCode::Char('m') | KeyCode::Char('M') => {
                        if app.current_tab == tui::app::Tab::ExpensePieChart {
                            app.toggle_pie_chart_mode();
                        }
                    }
                    KeyCode::Up => app.scroll_up(),
                    KeyCode::Down => app.scroll_down(),
                    KeyCode::PageUp => app.page_up(),
                    KeyCode::PageDown => app.page_down(),
                    KeyCode::Left => {
                        if app.current_tab == tui::app::Tab::Expenses {
                            // Cycle through sort columns
                            let next_col = match app.expense_sort_column {
                                tui::app::SortColumn::Id => tui::app::SortColumn::Date,
                                tui::app::SortColumn::Date => tui::app::SortColumn::Amount,
                                tui::app::SortColumn::Amount => tui::app::SortColumn::CategoryId,
                                tui::app::SortColumn::CategoryId => tui::app::SortColumn::Comment,
                                tui::app::SortColumn::Comment => tui::app::SortColumn::Id,
                            };
                            app.toggle_expense_sort(next_col);
                        } else if app.current_tab == tui::app::Tab::TopUps {
                            let next_col = match app.top_up_sort_column {
                                tui::app::SortColumn::Id => tui::app::SortColumn::Date,
                                tui::app::SortColumn::Date => tui::app::SortColumn::Amount,
                                tui::app::SortColumn::Amount => tui::app::SortColumn::CategoryId,
                                tui::app::SortColumn::CategoryId => tui::app::SortColumn::Comment,
                                tui::app::SortColumn::Comment => tui::app::SortColumn::Id,
                            };
                            app.toggle_top_up_sort(next_col);
                        }
                    }
                    KeyCode::Right => {
                        if app.current_tab == tui::app::Tab::Expenses {
                            // Cycle through sort columns
                            let next_col = match app.expense_sort_column {
                                tui::app::SortColumn::Id => tui::app::SortColumn::Comment,
                                tui::app::SortColumn::Comment => tui::app::SortColumn::CategoryId,
                                tui::app::SortColumn::CategoryId => tui::app::SortColumn::Amount,
                                tui::app::SortColumn::Amount => tui::app::SortColumn::Date,
                                tui::app::SortColumn::Date => tui::app::SortColumn::Id,
                            };
                            app.toggle_expense_sort(next_col);
                        } else if app.current_tab == tui::app::Tab::TopUps {
                            let next_col = match app.top_up_sort_column {
                                tui::app::SortColumn::Id => tui::app::SortColumn::Comment,
                                tui::app::SortColumn::Comment => tui::app::SortColumn::CategoryId,
                                tui::app::SortColumn::CategoryId => tui::app::SortColumn::Amount,
                                tui::app::SortColumn::Amount => tui::app::SortColumn::Date,
                                tui::app::SortColumn::Date => tui::app::SortColumn::Id,
                            };
                            app.toggle_top_up_sort(next_col);
                        }
                    }
                    _ => {}
                }
            }
            Event::Tick => {}
        }
    }

    Ok(())
}

fn handle_input_mode(app: &mut App, conn: &mut SqliteConnection, key: crossterm::event::KeyEvent) {
    use crossterm::event::KeyCode;

    match key.code {
        KeyCode::Esc => {
            app.cancel_input();
        }
        KeyCode::Enter => {
            app.form_state.error_message = None;

            // If on category field in expense form, just move to next field
            if app.input_mode == tui::app::InputMode::CreatingExpense
                && app.form_state.current_field == tui::app::InputField::ExpenseCategory {
                app.form_state.next_field(app.input_mode);
            } else {
                // Otherwise, submit the form
                let result = match app.input_mode {
                    tui::app::InputMode::CreatingCategory => app.submit_category(conn),
                    tui::app::InputMode::CreatingExpense => app.submit_expense(conn),
                    tui::app::InputMode::Normal => Ok(()),
                };

                if let Err(error) = result {
                    app.form_state.error_message = Some(error);
                }
            }
        }
        KeyCode::Tab => {
            if app.input_mode == tui::app::InputMode::CreatingExpense {
                app.form_state.next_field(app.input_mode);
            }
        }
        KeyCode::BackTab => {
            if app.input_mode == tui::app::InputMode::CreatingExpense {
                app.form_state.prev_field(app.input_mode);
            }
        }
        KeyCode::Up => {
            // Navigate category list when on category field
            if app.input_mode == tui::app::InputMode::CreatingExpense
                && app.form_state.current_field == tui::app::InputField::ExpenseCategory {
                app.category_list_previous();
            }
        }
        KeyCode::Down => {
            // Navigate category list when on category field
            if app.input_mode == tui::app::InputMode::CreatingExpense
                && app.form_state.current_field == tui::app::InputField::ExpenseCategory {
                app.category_list_next();
            }
        }
        KeyCode::Char(c) => {
            // Only allow typing in non-category fields
            if app.form_state.current_field != tui::app::InputField::ExpenseCategory {
                app.form_state.current_input().push(c);
                app.form_state.error_message = None;
            }
        }
        KeyCode::Backspace => {
            // Only allow backspace in non-category fields
            if app.form_state.current_field != tui::app::InputField::ExpenseCategory {
                app.form_state.current_input().pop();
                app.form_state.error_message = None;
            }
        }
        _ => {}
    }
}
