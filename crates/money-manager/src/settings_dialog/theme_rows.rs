//! The part of the settings that chooses a theme over the colour scheme: one
//! list for light and one for dark, so that "System" has a pair to switch
//! between, and the Zed registry to get more from.
//!
//! A choice takes effect as it is made, like the rest of Appearance: the
//! only way to judge a theme is to see it. The registry is asked for its
//! list when its section is first opened and searched locally after that.

use gpui_adaptive::theme::ThemeMode;
use gpui_adaptive::{Choice, ChoiceEvent, ChoiceOption, ChoiceState, Command, Setting};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Entity, Subscription, Window, div, rems};
use gpui_zed_themes::{Browser, BrowserEvent};

use super::SettingsForm;
use crate::themes;

pub(super) struct ThemeRows {
    light: Entity<ChoiceState>,
    dark: Entity<ChoiceState>,
    browser: Entity<Browser>,
    /// Whether the registry section is open.
    browsing: bool,
    _subscriptions: Vec<Subscription>,
}

/// The themes installed for `mode`, as options.
fn options(mode: ThemeMode, cx: &App) -> Vec<ChoiceOption> {
    themes::names(mode, cx)
        .into_iter()
        .map(|name| ChoiceOption::new(name.clone(), name))
        .collect()
}

impl ThemeRows {
    pub(super) fn new(window: &mut Window, cx: &mut Context<SettingsForm>) -> Self {
        // An install that outlived the form it was started from told
        // nobody; what is on disk now is what there is to choose from.
        themes::load(cx);

        let mut subscriptions = Vec::new();
        let mut picker = |mode: ThemeMode, cx: &mut Context<SettingsForm>| {
            let (options, worn) = (options(mode, cx), themes::worn(mode, cx));
            let state = cx.new(|cx| ChoiceState::searchable(options, Some(worn), window, cx));
            subscriptions.push(cx.subscribe(&state, move |_, _, event: &ChoiceEvent, cx| {
                let ChoiceEvent::Change(name) = event;
                themes::choose(mode, name, cx);
            }));
            state
        };
        let light = picker(ThemeMode::Light, cx);
        let dark = picker(ThemeMode::Dark, cx);

        let browser = cx.new(|cx| Browser::new(themes::registry(), themes::store(), window, cx));
        subscriptions.push(cx.subscribe_in(
            &browser,
            window,
            |form, _, _: &BrowserEvent, window, cx| {
                themes::load(cx);
                form.themes.listed_again(window, cx);
            },
        ));

        ThemeRows {
            light,
            dark,
            browser,
            browsing: false,
            _subscriptions: subscriptions,
        }
    }

    /// Makes the two lists say what there is to choose from now.
    fn listed_again(&self, window: &mut Window, cx: &mut Context<SettingsForm>) {
        for (state, mode) in [
            (&self.light, ThemeMode::Light),
            (&self.dark, ThemeMode::Dark),
        ] {
            let (options, worn) = (options(mode, cx), themes::worn(mode, cx));
            state.update(cx, |state, cx| state.set_options(options, Some(worn), window, cx));
        }
        cx.notify();
    }

    /// Opens or closes the registry section.
    fn browse(&mut self, window: &mut Window, cx: &mut Context<SettingsForm>) {
        self.browsing = !self.browsing;
        if self.browsing {
            self.browser.update(cx, |browser, cx| {
                browser.load(cx);
                browser.focus(window, cx);
            });
        }
        cx.notify();
    }

    pub(super) fn render(&self, cx: &mut Context<SettingsForm>) -> [Setting; 3] {
        let theme = |state: &Entity<ChoiceState>, id: &'static str| {
            div().w(rems(14.)).child(Choice::new(state).id(id))
        };
        let form = cx.entity().downgrade();
        let browse = Command::new(
            "browse-themes",
            if self.browsing { "Done" } else { "Browse…" },
        )
        .on_trigger(move |window, cx| {
            form.update(cx, |form, cx| form.themes.browse(window, cx)).ok();
        });
        let more = Setting::new("More themes")
            .detail("From the Zed editor's registry")
            .control(browse.as_button());
        [
            Setting::new("Light theme")
                .detail("Worn in light")
                .control(theme(&self.light, "light-theme")),
            Setting::new("Dark theme")
                .detail("Worn in dark")
                .control(theme(&self.dark, "dark-theme")),
            if self.browsing {
                more.below(self.browser.clone())
            } else {
                more
            },
        ]
    }
}
