//! The window's own frame: its title bar, and the room the system's bars take.

use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App};

/// The bar across the top of the window, around `content`.
///
/// On a desktop it is the toolkit's title bar: what drags the window and
/// carries its buttons. A phone has no window to drag, minimise or close, and
/// that bar draws the buttons regardless — so there it is a plain strip, tall
/// enough for a finger.
pub fn title_frame(content: impl IntoElement, cx: &App) -> AnyElement {
    let theme = cx.theme();
    #[cfg(not(target_os = "android"))]
    {
        gpui_kit::component::TitleBar::new()
            .bg(theme.title_bar)
            .border_color(theme.title_bar_border)
            .child(content)
            .into_any_element()
    }
    #[cfg(target_os = "android")]
    {
        gpui_kit::component::h_flex()
            .flex_none()
            .h_12()
            .px_3()
            .border_b_1()
            .bg(theme.title_bar)
            .border_color(theme.title_bar_border)
            .child(content)
            .into_any_element()
    }
}

/// How much of the top and of the bottom of the screen the system draws its
/// own bars over, which the window lies under edge to edge. These are the
/// platform's insets in its own units, not layout of ours.
#[cfg(target_os = "android")]
pub(super) fn system_bars() -> (f32, f32) {
    gpui_mobile::android::jni::platform()
        .and_then(|platform| platform.primary_window())
        .map(|window| {
            let insets = window.safe_area_insets_logical();
            (insets.top, insets.bottom)
        })
        .unwrap_or_default()
}

/// A desktop has no bars of the system's over its windows.
/// `MONEY_MANAGER_KEYBOARD=<height>` pretends an on-screen keyboard of that
/// height is up, which is the only way to look at what the interface does
/// about one without a phone.
#[cfg(not(target_os = "android"))]
pub(super) fn system_bars() -> (f32, f32) {
    let keyboard = std::env::var("MONEY_MANAGER_KEYBOARD")
        .ok()
        .and_then(|height| height.parse().ok())
        .unwrap_or(0.0);
    (0.0, keyboard)
}

/// How tall the on-screen keyboard is over `window`, or nothing while it is
/// down. See [`bottom_bar_room`]: the interface as a whole stays where it is
/// under the keyboard, and only what is being typed into — a dialog — makes
/// room for it.
pub fn keyboard_inset(window: &gpui_kit::Window) -> gpui_kit::Pixels {
    let (_, inset) = system_bars();
    let height = f32::from(window.viewport_size().height);
    gpui_kit::Pixels::from(keyboard_height(inset, height))
}

/// How tall the status bar at the top of `window` is.
pub fn status_bar_inset() -> gpui_kit::Pixels {
    gpui_kit::Pixels::from(system_bars().0)
}

/// The part of the bottom inset that is a keyboard: all of it, once it is
/// too tall to be the system's bar.
fn keyboard_height(inset: f32, window_height: f32) -> f32 {
    if inset > window_height * BAR_AT_MOST {
        inset
    } else {
        0.0
    }
}

/// How much of the window's height an inset at the bottom may take and still
/// be the system's navigation bar. More than this is the on-screen keyboard.
const BAR_AT_MOST: f32 = 0.2;

/// The room to leave at the bottom for the system's own bar.
///
/// The platform reports one number for everything that covers the bottom of
/// the window, and while the keyboard is up that number is the keyboard's
/// height. Making room for it would lift the whole interface — the
/// navigation with it — every time a field is typed in, and drop it again a
/// moment after the keyboard has gone. The keyboard lies over the interface
/// instead: an inset too tall to be a bar is passed over, and the room stays
/// what it was (`resting`) before the keyboard came up.
pub(super) fn bottom_bar_room(inset: f32, window_height: f32, resting: f32) -> f32 {
    if inset > window_height * BAR_AT_MOST {
        resting
    } else {
        inset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keyboard_does_not_move_what_the_navigation_bar_does() {
        // The gesture bar, then a keyboard over it, then the bar again.
        let mut room = bottom_bar_room(24.0, 800.0, 0.0);
        assert_eq!(room, 24.0);
        room = bottom_bar_room(310.0, 800.0, room);
        assert_eq!(room, 24.0);
        room = bottom_bar_room(24.0, 800.0, room);
        assert_eq!(room, 24.0);
        // Turning the phone changes the bar's own height, and that is taken.
        assert_eq!(bottom_bar_room(48.0, 800.0, room), 48.0);
        assert_eq!(bottom_bar_room(0.0, 800.0, room), 0.0);
    }

    #[test]
    fn an_inset_is_the_keyboard_or_the_bar_never_both() {
        // What the interface as a whole ignores is exactly what a dialog
        // makes room for.
        assert_eq!(keyboard_height(310.0, 800.0), 310.0);
        assert_eq!(keyboard_height(24.0, 800.0), 0.0);
        assert_eq!(keyboard_height(0.0, 800.0), 0.0);
    }
}
