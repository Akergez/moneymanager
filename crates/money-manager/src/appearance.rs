//! Light or dark.
//!
//! Three choices, kept in `settings.json`. "System" is the default and means
//! what the desktop says right now — and again whenever it changes its mind,
//! which a desktop on a day/night schedule does twice a day.

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Window};

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
    if Theme::global(cx).mode != mode {
        Theme::change(mode, None, cx);
    }
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
    fn an_unknown_theme_is_the_system_one() {
        assert_eq!(ThemeChoice::parse("sepia"), ThemeChoice::System);
        assert_eq!(ThemeChoice::parse(""), ThemeChoice::System);
    }
}
