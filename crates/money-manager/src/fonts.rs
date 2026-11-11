//! The fonts the application brings with it.
//!
//! The text renderer loads a **variable** UI font as one regular face, so
//! nothing drawn bold actually is — and the default UI fonts on current
//! desktops are variable. A phone has no family the renderer's fallbacks name
//! at all. So the application carries static faces of Inter and points the
//! theme at them (see `assets/fonts/README.md`).
//!
//! Not on Android: there gpui-mobile draws the phone's own (variable) font at
//! every weight, and the interface is in it, as big and as bold as the phone
//! is set to draw text.

use std::borrow::Cow;

use gpui_kit::App;
#[cfg(not(target_os = "android"))]
use gpui_adaptive::theme::Theme;

/// The family the bundled UI faces belong to.
#[cfg(not(target_os = "android"))]
const UI_FAMILY: &str = "Inter";

/// Registers the bundled fonts and points the theme at them. Call once, after
/// the component library is initialised and before any window is opened.
pub fn install(cx: &mut App) {
    let fonts: Vec<Cow<'static, [u8]>> = vec![
        Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Regular.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Medium.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Inter-SemiBold.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/Inter-Bold.ttf")),
    ];
    match cx.text_system().add_fonts(fonts) {
        // Text still draws, in whatever the desktop has; it is bold that will
        // be missing, which is worth saying once.
        Err(error) => tracing::warn!(%error, "could not load the bundled fonts"),
        #[cfg(not(target_os = "android"))]
        Ok(()) => Theme::update(cx, |theme| theme.font_family = UI_FAMILY.into()),
        #[cfg(target_os = "android")]
        Ok(()) => {}
    }
}
