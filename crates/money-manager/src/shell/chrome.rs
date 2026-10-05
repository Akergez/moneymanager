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

#[cfg(not(target_os = "android"))]
pub(super) fn system_bars() -> (f32, f32) {
    (0.0, 0.0)
}
