//! Themes to wear instead of the colour scheme.
//!
//! What the window wears by default is a scheme made from one colour
//! ([`crate::appearance`]). Someone who would rather have a theme a person
//! designed can install one from the Zed editor's registry — the settings
//! dialog browses it — and choose it for light, for dark, or for both. The
//! choice is two names in `settings.json`; no name means the scheme.
//!
//! Reading Zed's format, the registry and the directory the themes are kept
//! in are the `gpui-zed-themes` crate's. This module is what is ours: which
//! themes there are to choose from right now, and which is chosen. No theme
//! is compiled in or kept in the repository.
//!
//! The list is read once at startup and again after an install or a removal
//! — a handful of small files, read before there is a frame to delay.

use std::collections::HashSet;
use std::rc::Rc;

use gpui_kit::component::{ThemeConfig, ThemeMode};
use gpui_kit::{App, Global, SharedString};
use gpui_zed_themes::{Registry, Store};

use crate::settings::Settings;

/// What the colour scheme is called in a list of themes.
pub const SCHEME: &str = "Color scheme";

/// Every installed theme, one to a name.
struct Catalogue(Vec<Rc<ThemeConfig>>);

impl Global for Catalogue {}

pub fn store() -> Store {
    Store::new(crate::paths::themes_dir())
}

/// Zed's registry, or what `MONEY_MANAGER_THEMES_API` names in its place:
/// the UI tests have a stand-in, since a test must not depend on a service
/// of somebody else's.
pub fn registry() -> Registry {
    let registry = Registry::new(concat!("money-manager/", env!("CARGO_PKG_VERSION")));
    match std::env::var("MONEY_MANAGER_THEMES_API") {
        Ok(api) if !api.is_empty() => registry.at(api),
        _ => registry,
    }
}

/// A theme with nothing but its colours. The interface's fonts, sizes and
/// corners are chosen in the settings and are not a theme's to change; a file
/// written by hand in the toolkit's own format could otherwise set them.
fn colors_only(mut theme: ThemeConfig) -> ThemeConfig {
    theme.font_size = None;
    theme.font_family = None;
    theme.mono_font_family = None;
    theme.mono_font_size = None;
    theme.radius = None;
    theme.radius_lg = None;
    theme.shadow = None;
    theme
}

/// One list, the first theme of a name winning: names are what the settings
/// store, so there can only be one theme to a name. Nothing may take the
/// scheme's.
fn catalogued(themes: impl IntoIterator<Item = ThemeConfig>) -> Vec<Rc<ThemeConfig>> {
    let mut seen: HashSet<SharedString> = HashSet::from([SCHEME.into()]);
    themes
        .into_iter()
        .filter(|theme| seen.insert(theme.name.clone()))
        .map(|theme| Rc::new(colors_only(theme)))
        .collect()
}

/// Reads the themes directory again and wears what the settings name. Call
/// once at startup, after [`crate::appearance::init`], and after the
/// directory changed.
pub fn load(cx: &mut App) {
    cx.set_global(Catalogue(catalogued(store().themes())));
    apply(cx);
}

fn catalogue(cx: &App) -> &[Rc<ThemeConfig>] {
    cx.try_global::<Catalogue>()
        .map(|catalogue| catalogue.0.as_slice())
        .unwrap_or_default()
}

/// What there is to choose from in one mode: the scheme first, then the
/// installed themes of that mode in alphabetical order.
pub fn names(mode: ThemeMode, cx: &App) -> Vec<SharedString> {
    let mut names: Vec<SharedString> = catalogue(cx)
        .iter()
        .filter(|theme| theme.mode == mode)
        .map(|theme| theme.name.clone())
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.insert(0, SCHEME.into());
    names
}

/// How many themes are installed, of either mode.
pub fn count(cx: &App) -> usize {
    catalogue(cx).len()
}

/// The installed theme chosen for `mode`, if one is chosen and still there:
/// a theme removed since it was chosen leaves the scheme, not nothing.
fn chosen(mode: ThemeMode, cx: &App) -> Option<Rc<ThemeConfig>> {
    let wanted = Settings::global(cx).theme_name(mode.is_dark())?;
    catalogue(cx)
        .iter()
        .find(|theme| theme.mode == mode && theme.name == wanted)
        .cloned()
}

/// The name of what is worn in `mode`, as [`names`] lists it.
pub fn worn(mode: ThemeMode, cx: &App) -> SharedString {
    chosen(mode, cx).map_or(SCHEME.into(), |theme| theme.name.clone())
}

/// Chooses by name, as [`names`] lists them.
pub fn choose(mode: ThemeMode, name: &str, cx: &mut App) {
    let name = (name != SCHEME).then_some(name);
    Settings::update(cx, |settings| settings.set_theme_name(mode.is_dark(), name));
    apply(cx);
}

/// Makes the window wear what the settings name, in both modes.
fn apply(cx: &mut App) {
    for mode in [ThemeMode::Light, ThemeMode::Dark] {
        match chosen(mode, cx) {
            Some(theme) => gpui_adaptive_colors::wear(theme, cx),
            None => gpui_adaptive_colors::wear_scheme(mode, cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(name: &str, mode: ThemeMode) -> ThemeConfig {
        ThemeConfig {
            name: name.to_string().into(),
            mode,
            ..Default::default()
        }
    }

    #[test]
    fn the_first_theme_of_a_name_is_the_one_kept_and_none_is_called_the_scheme() {
        let themes = catalogued([
            named("A", ThemeMode::Dark),
            named(SCHEME, ThemeMode::Light),
            named("A", ThemeMode::Light),
            named("B", ThemeMode::Light),
        ]);
        let listed: Vec<_> = themes
            .iter()
            .map(|theme| (theme.name.as_ref(), theme.mode))
            .collect();
        assert_eq!(listed, [("A", ThemeMode::Dark), ("B", ThemeMode::Light)]);
    }

    #[test]
    fn a_theme_brings_its_colours_and_nothing_else() {
        let mut theme = named("Loud", ThemeMode::Light);
        theme.font_size = Some(30.0);
        theme.font_family = Some("Papyrus".into());
        theme.radius = Some(40);
        theme.colors.background = Some("#101010".into());

        let kept = &catalogued([theme])[0];
        assert_eq!(kept.font_size, None);
        assert_eq!(kept.font_family, None);
        assert_eq!(kept.radius, None);
        assert_eq!(kept.colors.background.as_deref(), Some("#101010"));
    }
}
