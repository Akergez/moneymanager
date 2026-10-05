//! The window, and what is in it at each stage of a launch.
//!
//! A launch is in one of three stages. With no ledger set up it asks which
//! there should be ([`crate::onboarding`]); with one, it shows the workspace
//! ([`crate::workspace`]); and when the ledger on disk cannot be opened it
//! says so rather than offering to make a new one over it.
//!
//! [`Shell`] is the one view the window holds. It owns the stage and the
//! parts of the frame every stage shares: the strip under the system's status
//! bar on a phone, and the window's own title bar everywhere else.

mod actions;
mod chrome;
mod layout;
mod open_window;
mod shell_view;
mod window_size;

pub use actions::*;
pub use chrome::title_frame;
pub use layout::Layout;
pub use open_window::open_window;
pub use shell_view::{Shell, Stage};

use gpui_kit::{AnyWindowHandle, App, Entity, Global, KeyBinding};

/// What outlives any one view: the window, once there is one.
#[derive(Default)]
pub struct Session {
    pub window: Option<(AnyWindowHandle, Entity<Shell>)>,
}

impl Global for Session {}

/// Sets up the shortcuts and what happens when the window goes away.
pub fn init(cx: &mut App) {
    cx.set_global(Session::default());

    // Anything reachable only by pointer is reachable only slowly.
    cx.bind_keys([
        KeyBinding::new("ctrl-q", Quit, None),
        KeyBinding::new("ctrl-1", ShowTransactions, Some(CONTEXT)),
        KeyBinding::new("ctrl-2", ShowCategories, Some(CONTEXT)),
        KeyBinding::new("ctrl-3", ShowCharts, Some(CONTEXT)),
        KeyBinding::new("ctrl-e", ToggleMode, Some(CONTEXT)),
        KeyBinding::new("ctrl-n", NewRecord, Some(CONTEXT)),
        KeyBinding::new("ctrl-t", NewTransfer, Some(CONTEXT)),
        KeyBinding::new("ctrl-r", SyncNow, Some(CONTEXT)),
        KeyBinding::new("ctrl-,", OpenSettings, Some(CONTEXT)),
    ]);
    cx.on_action(|_: &Quit, cx| cx.quit());

    // One window is the whole application: nothing runs behind it.
    cx.on_window_closed(|cx, _| {
        let ours = cx
            .try_global::<Session>()
            .and_then(|session| session.window.as_ref().map(|(handle, _)| *handle));
        if ours.is_none_or(|handle| !cx.windows().contains(&handle)) {
            cx.quit();
        }
    })
    .detach();
}

/// The shell of the window, when there is one.
pub fn current(cx: &App) -> Option<Entity<Shell>> {
    cx.try_global::<Session>()
        .and_then(|session| session.window.as_ref().map(|(_, shell)| shell.clone()))
}
