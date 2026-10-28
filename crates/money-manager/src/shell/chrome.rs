//! The window's own frame: its title bar, and the room the system's bars take.

use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App};

/// The bar across the top of the window, around `content`: the toolkit's
/// title bar on a desktop, a plain strip tall enough for a finger on a phone.
pub fn title_frame(content: impl IntoElement, cx: &App) -> AnyElement {
    gpui_android_glue::chrome::title_frame(content, Default::default(), cx)
}

/// How much of the top and of the bottom of the screen the system draws its
/// own bars over, which the window lies under edge to edge. The keyboard is
/// not in it: see [`keyboard_inset`]. These are the platform's insets in its
/// own units, not layout of ours.
pub(super) fn system_bars() -> (f32, f32) {
    let (top, bottom) = gpui_android_glue::insets::system_bars();
    (f32::from(top), f32::from(bottom))
}

/// How tall the on-screen keyboard is over `window` as it is drawn now, or
/// nothing while it is down: partway while it slides in or out, so what makes
/// room for it moves with it. The interface as a whole stays where it is
/// under the keyboard, and only what is being typed into — a dialog, a sheet
/// — makes room for it.
///
/// A desktop has no keyboard of the system's. `MONEY_MANAGER_KEYBOARD=<height>`
/// pretends one of that height is up, which is the only way to look at what
/// the interface does about one without a phone.
pub fn keyboard_inset(window: &gpui_kit::Window) -> gpui_kit::Pixels {
    #[cfg(not(target_os = "android"))]
    if let Some(height) = std::env::var("MONEY_MANAGER_KEYBOARD")
        .ok()
        .and_then(|height| height.parse::<f32>().ok())
    {
        return gpui_kit::Pixels::from(height);
    }
    gpui_android_glue::keyboard::keyboard_inset(window)
}

/// How tall the status bar at the top of `window` is.
pub fn status_bar_inset() -> gpui_kit::Pixels {
    gpui_kit::Pixels::from(system_bars().0)
}
