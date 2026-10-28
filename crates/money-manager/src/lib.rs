//! Money Manager: expenses, income and transfers across accounts.
//!
//! The ledger itself — the records, the RDX store they are kept in and the
//! sync — is the `money-core` crate, which knows nothing of an interface.
//! This crate is the interface, drawn with GPUI through `gpui-kit`, with one
//! module per thing a person does:
//!
//! - [`onboarding`] — the first-run question: a new ledger, or a synced one;
//! - [`workspace`] — navigation, the account, and the mode all screens share;
//! - [`transactions`], [`categories`], [`charts`] — the three screens;
//! - [`accounts`] — an account's details, and transfers;
//! - [`settings_dialog`] — appearance and the sync storage.
//!
//! Under them: [`book`] is the open ledger as the interface sees it,
//! [`settings`] what this installation remembers, and [`shell`] the window.
//!
//! This is a library so that Android can load it: there the system starts a
//! Java activity, which loads this as a shared object and calls
//! [`android_main`]. Everywhere else `main.rs` calls [`run`].

mod accounts;
mod appearance;
mod assets;
mod book;
mod bottom_sheet;
mod categories;
mod charts;
mod date_field;
mod fonts;
mod onboarding;
mod paths;
mod script;
mod settings;
mod settings_dialog;
mod shell;
mod themes;
mod transactions;
mod ui;
mod workspace;

/// Names the application to the system. The directories the ledger and the
/// settings live in are named after it, so it must never change.
pub const APP_ID: &str = "app.akergez.MoneyManager";

/// Today, as every form and chart means it.
///
/// `MONEY_MANAGER_TODAY=2026-10-05` pins it, which is what lets a UI scenario
/// state how many of the sample records there are without that depending on
/// the day it is run.
pub(crate) fn today() -> chrono::NaiveDate {
    std::env::var("MONEY_MANAGER_TODAY")
        .ok()
        .and_then(|day| chrono::NaiveDate::parse_from_str(&day, "%Y-%m-%d").ok())
        .unwrap_or_else(|| chrono::Local::now().date_naive())
}

/// Whether this launch should skip the first-run question and show a sample
/// ledger (`MONEY_MANAGER_DEMO=1`). Only an installation with no ledger of
/// its own is ever filled: see `Shell::start`.
pub(crate) fn demo_requested() -> bool {
    std::env::var_os("MONEY_MANAGER_DEMO").is_some()
}

/// The desktop entry point: what `main` is.
#[cfg(not(target_os = "android"))]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn,money_manager=info".into()),
        )
        .init();

    gpui_kit::application()
        .with_assets(assets::AppAssets)
        .run(start);
}

/// What every platform does once the toolkit is running.
fn start(cx: &mut gpui_kit::App) {
    gpui_kit::init(cx);
    // On Android `launch::run` has done this already.
    #[cfg(not(target_os = "android"))]
    gpui_android_glue::init(cx);
    settings::Settings::install(cx);
    fonts::install(cx);
    appearance::init(cx);
    themes::load(cx);
    appearance::apply(None, cx);
    shell::init(cx);
    shell::open_window(cx);
}

/// The Android entry point, called by `android-activity` on a thread of its
/// own once the activity has loaded this library. The ledger and the settings
/// are kept in the application's private directory, which `launch::run`
/// points the XDG variables at: no shared storage, so no permission to ask
/// for.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: gpui_android_glue::launch::AndroidApp) {
    use gpui_android_glue::launch::{Config, run};
    run(
        app,
        Config::new("money-manager").filter("warn,money_manager=info"),
        assets::AppAssets,
        start,
    );
}
