//! Light or dark.
//!
//! Three choices, kept in `settings.json`. "System" is the default and means
//! what the desktop says right now — and again whenever it changes its mind,
//! which a desktop on a day/night schedule does twice a day.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Pixels, Window};

use crate::settings::Settings;

/// Light, dark, or whatever the system is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 3] = [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark];

    pub fn label(self) -> &'static str {
        match self {
            ThemeChoice::System => "System",
            ThemeChoice::Light => "Light",
            ThemeChoice::Dark => "Dark",
        }
    }

    fn stored(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
        }
    }

    /// Anything unrecognised is the default: a settings file from a later
    /// version must not make this one refuse to draw.
    fn parse(stored: &str) -> Self {
        match stored {
            "light" => ThemeChoice::Light,
            "dark" => ThemeChoice::Dark,
            _ => ThemeChoice::System,
        }
    }
}

/// How large everything is drawn. Every size in the interface is in rems, so
/// this one factor on the rem scales text, controls, spacing and charts
/// together; nothing is sized apart from it.
///
/// It stops at Large. The component library keeps a few sizes of its own in
/// pixels — the title bar's height, a table row's, the collapsed sidebar's
/// width — and past this factor text no longer fits inside them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InterfaceSize {
    Compact,
    #[default]
    Regular,
    Large,
}

impl InterfaceSize {
    pub const ALL: [InterfaceSize; 3] = [
        InterfaceSize::Compact,
        InterfaceSize::Regular,
        InterfaceSize::Large,
    ];

    pub fn label(self) -> &'static str {
        match self {
            InterfaceSize::Compact => "Compact",
            InterfaceSize::Regular => "Regular",
            InterfaceSize::Large => "Large",
        }
    }

    fn stored(self) -> &'static str {
        match self {
            InterfaceSize::Compact => "compact",
            InterfaceSize::Regular => "regular",
            InterfaceSize::Large => "large",
        }
    }

    fn parse(stored: &str) -> Self {
        Self::ALL
            .into_iter()
            .find(|size| size.stored() == stored)
            .unwrap_or_else(Self::for_platform)
    }

    /// The size nobody has chosen yet. A phone is held closer than a monitor
    /// stands, and its system has already scaled everything to be read at
    /// that distance: the library's own size is the right one there, and a
    /// step up from it is too much.
    pub fn for_platform() -> Self {
        if cfg!(target_os = "android") {
            InterfaceSize::Compact
        } else {
            InterfaceSize::default()
        }
    }

    /// The factor on the component library's own rem. Compact is the
    /// library's size, which is small for a list of amounts read at arm's
    /// length; Regular is a step up from it.
    pub fn factor(self) -> f32 {
        match self {
            InterfaceSize::Compact => 1.0,
            InterfaceSize::Regular => 1.125,
            InterfaceSize::Large => 1.25,
        }
    }
}

/// The component library's own rem and monospace size, noted before anything
/// scales them: a factor is applied to these, never to its own result.
static LIBRARY_SIZES: std::sync::OnceLock<(Pixels, Pixels)> = std::sync::OnceLock::new();

pub fn interface_size(cx: &App) -> InterfaceSize {
    // A scenario's click coordinates are for one size; `MONEY_MANAGER_UI_SIZE`
    // pins it whatever the settings say.
    std::env::var("MONEY_MANAGER_UI_SIZE")
        .ok()
        .or_else(|| Settings::global(cx).interface_size())
        .map(|stored| InterfaceSize::parse(&stored))
        .unwrap_or_else(InterfaceSize::for_platform)
}

pub fn set_interface_size(size: InterfaceSize, cx: &mut App) {
    Settings::update(cx, |settings| settings.set_interface_size(size.stored()));
    apply_size(cx);
    cx.refresh_windows();
}

/// The rem the interface is drawn at: the library's, times the chosen size.
pub fn rem(cx: &App) -> Pixels {
    library_sizes(cx).0 * interface_size(cx).factor()
}

fn library_sizes(cx: &App) -> (Pixels, Pixels) {
    *LIBRARY_SIZES.get_or_init(|| {
        let theme = Theme::global(cx);
        (theme.font_size, theme.mono_font_size)
    })
}

/// Makes the theme's sizes say what the settings say. The window takes its
/// rem from the theme on every frame, so this is all it takes.
fn apply_size(cx: &mut App) {
    let (font, mono) = library_sizes(cx);
    let factor = interface_size(cx).factor();
    let theme = Theme::global_mut(cx);
    theme.font_size = font * factor;
    theme.mono_font_size = mono * factor;
}

pub fn theme_choice(cx: &App) -> ThemeChoice {
    Settings::global(cx)
        .theme()
        .map(|stored| ThemeChoice::parse(&stored))
        .unwrap_or_default()
}

pub fn set_theme_choice(choice: ThemeChoice, window: &mut Window, cx: &mut App) {
    Settings::update(cx, |settings| settings.set_theme(choice.stored()));
    apply(Some(window), cx);
}

/// Makes the theme say what the settings say. Call at startup, again once
/// there is a window — on Linux only a window knows what the desktop's
/// appearance is — and whenever the choice changes.
pub fn apply(window: Option<&mut Window>, cx: &mut App) {
    let mode = match theme_choice(cx) {
        ThemeChoice::Light => ThemeMode::Light,
        ThemeChoice::Dark => ThemeMode::Dark,
        // Android's window starts out calling itself light whatever the phone
        // says, so the phone is asked.
        #[cfg(target_os = "android")]
        ThemeChoice::System => {
            let _ = &window;
            if gpui_mobile::android::jni::query_night_mode_via_jni() {
                ThemeMode::Dark
            } else {
                ThemeMode::Light
            }
        }
        #[cfg(not(target_os = "android"))]
        ThemeChoice::System => window
            .as_ref()
            .map(|window| window.appearance())
            .unwrap_or_else(|| cx.window_appearance())
            .into(),
    };
    // The clock and the icons beside it are drawn over our own top bar, and
    // have to be told which of its colours to stand out against.
    #[cfg(target_os = "android")]
    gpui_mobile::set_system_chrome(&gpui_mobile::SystemChromeStyle {
        status_bar_style: if mode.is_dark() {
            gpui_mobile::StatusBarContentStyle::Light
        } else {
            gpui_mobile::StatusBarContentStyle::Dark
        },
        ..Default::default()
    });
    // Before the first change of theme, so the sizes noted are the library's.
    library_sizes(cx);
    if Theme::global(cx).mode != mode {
        Theme::change(mode, None, cx);
    }
    // A change of theme may put the library's sizes back.
    apply_size(cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_theme_choice_survives_being_stored() {
        for choice in ThemeChoice::ALL {
            assert_eq!(ThemeChoice::parse(choice.stored()), choice);
        }
    }

    #[test]
    fn an_interface_size_survives_being_stored_and_only_ever_enlarges() {
        for size in InterfaceSize::ALL {
            assert_eq!(InterfaceSize::parse(size.stored()), size);
            assert!(size.factor() >= 1.0);
        }
        assert_eq!(InterfaceSize::parse("enormous"), InterfaceSize::Regular);
    }

    #[test]
    fn an_unknown_theme_is_the_system_one() {
        assert_eq!(ThemeChoice::parse("sepia"), ThemeChoice::System);
        assert_eq!(ThemeChoice::parse(""), ThemeChoice::System);
    }
}
