mod config;
mod import;
mod models;
mod services;
mod store;
mod tui;

use std::io;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use crossterm::{
    event::DisableMouseCapture,
    event::EnableMouseCapture,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use config::Config;
use store::Store;
use tui::{AppState, Event, EventHandler};

/// A terminal-based money management application.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the RDX chunk directory (one file per content-addressed chunk).
    #[arg(short, long, default_value = "money_manager.chunks")]
    data: PathBuf,

    /// Path to the configuration file.
    #[arg(short, long, default_value = "money_manager.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Synchronize the local data with the configured S3 remote.
    Sync,
    /// Print a fresh base64 encryption key for the config's `encryption_key`.
    Keygen,
    /// Import or export the S3 remote config as a single shareable string.
    Config {
        /// Print a commented config template (redirect into money_manager.toml).
        #[arg(long)]
        template: bool,
        /// Print the current [remote] config as one shareable string.
        #[arg(long)]
        export: bool,
        /// Parse a shareable config string and write it into the config file.
        #[arg(long, value_name = "STRING")]
        import: Option<String>,
    },
    /// Import records from a CSV file mirroring the original SQL table.
    ImportCsv {
        /// Which table the CSV maps to.
        #[arg(long, value_enum)]
        table: import::Table,
        /// Path to the CSV file (must have a header row).
        file: PathBuf,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if let Some(Command::Keygen) = args.command {
        use base64::Engine;
        let key = rdx_sync::crypto::generate_key()?;
        println!("{}", base64::engine::general_purpose::STANDARD.encode(key));
        return Ok(());
    }

    let mut cfg = Config::load(&args.config)?;

    if let Some(Command::Config { template, export, import }) = &args.command {
        if *template {
            print!("{}", config::CONFIG_TEMPLATE);
        } else if let Some(string) = import {
            cfg.remote = Some(config::RemoteConfig::from_share_string(string)?);
            cfg.save(&args.config)?;
            println!("imported S3 config into {}", args.config.display());
        } else if *export {
            let remote = cfg
                .remote
                .as_ref()
                .ok_or("[remote] section missing in config")?;
            println!("{}", remote.to_share_string());
        } else {
            return Err("config: pass --template, --export, or --import <STRING>".into());
        }
        return Ok(());
    }

    let source = cfg.ensure_source(&args.config)?;
    let mut store = Store::open(&args.data, source)?;

    // One-time migration from the old single-blob format.
    let legacy = PathBuf::from("money_manager.rdx");
    if store.is_empty()? && legacy.exists() {
        let n = store.import_legacy_blob(&legacy)?;
        eprintln!(
            "migrated {n} records from legacy {} into {}",
            legacy.display(),
            args.data.display()
        );
    }

    if let Some(Command::Sync) = args.command {
        let remote = cfg
            .remote
            .as_ref()
            .ok_or("[remote] section missing in config")?;
        let report = store.sync(remote)?;
        println!(
            "sync completed (pulled {}, pushed {})",
            report.pulled.len(),
            report.pushed.len()
        );
        return Ok(());
    }

    if let Some(Command::ImportCsv { table, file }) = &args.command {
        let n = import::import_csv(&mut store, *table, file)?;
        println!("imported {n} rows from {}", file.display());
        return Ok(());
    }

    run_tui(&mut store, cfg.remote.clone())
}

fn run_tui(
    store: &mut Store,
    remote: Option<config::RemoteConfig>,
) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(store, remote)?;
    let event_handler = EventHandler::new();

    let res = run_app(&mut terminal, &mut state, store, &event_handler);

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
    store: &mut Store,
    event_handler: &EventHandler,
) -> io::Result<()> {
    while state.running {
        let frame_area = terminal.get_frame().area();
        terminal.draw(|f| tui::ui::draw(f, state))?;

        match event_handler.next()? {
            Event::Key(key) => {
                state.handle_input(key.code, key.modifiers, store);
            }
            Event::Mouse(mouse) => {
                state.handle_mouse(mouse, frame_area, store);
            }
            Event::Tick => {}
        }
    }

    Ok(())
}
