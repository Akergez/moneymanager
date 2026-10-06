//! The part of the settings dialog that chooses a theme over the colour
//! scheme: one list for light and one for dark, so that "System" has a pair
//! to switch between, and the Zed registry to get more from.
//!
//! A choice takes effect as it is made, like the rest of Appearance: the
//! only way to judge a theme is to see it. The registry is asked for its
//! list when its section is first opened and searched locally after that.

use gpui_kit::component::button::Button;
use gpui_kit::component::select::{SearchableVec, Select, SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme, IndexPath, Sizable, ThemeMode, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{AppContext, Context, Entity, SharedString, Subscription, Window, div};
use gpui_zed_themes::{Browser, BrowserEvent};

use super::SettingsForm;
use crate::themes;

/// A searchable list of theme names.
type Names = SelectState<SearchableVec<SharedString>>;

pub(super) struct ThemeRows {
    light: Entity<Names>,
    dark: Entity<Names>,
    browser: Entity<Browser>,
    /// Whether the registry section is open.
    browsing: bool,
    _subscriptions: Vec<Subscription>,
}

impl ThemeRows {
    pub(super) fn new(window: &mut Window, cx: &mut Context<SettingsForm>) -> Self {
        // An install that outlived the dialog it was started from told
        // nobody; what is on disk now is what there is to choose from.
        themes::load(cx);

        let mut subscriptions = Vec::new();
        let mut picker = |mode: ThemeMode| {
            let names = themes::names(mode, cx);
            let worn = themes::worn(mode, cx);
            let selected = names.iter().position(|name| *name == worn).unwrap_or(0);
            let state = cx.new(|cx| {
                SelectState::new(
                    SearchableVec::new(names),
                    Some(IndexPath::default().row(selected)),
                    window,
                    cx,
                )
                .searchable(true)
            });
            subscriptions.push(cx.subscribe(
                &state,
                move |_, _, event: &SelectEvent<SearchableVec<SharedString>>, cx| {
                    if let SelectEvent::Confirm(Some(name)) = event {
                        themes::choose(mode, name, cx);
                    }
                },
            ));
            state
        };
        let light = picker(ThemeMode::Light);
        let dark = picker(ThemeMode::Dark);

        let browser =
            cx.new(|cx| Browser::new(themes::registry(), themes::store(), window, cx));
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
            let names = themes::names(mode, cx);
            let worn = themes::worn(mode, cx);
            state.update(cx, |state, cx| {
                state.set_items(SearchableVec::new(names), window, cx);
                state.set_selected_value(&worn, window, cx);
            });
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

    pub(super) fn render(&self, cx: &mut Context<SettingsForm>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let row = |title: &'static str, subtitle: &'static str| {
            v_flex()
                .flex_1()
                .min_w_0()
                .child(title)
                .child(div().text_xs().text_color(muted).child(subtitle))
        };
        let select = |state: &Entity<Names>| {
            div().flex_none().w_48().child(
                Select::new(state)
                    .small()
                    .search_placeholder("Search themes"),
            )
        };
        v_flex()
            .gap_4()
            .child(
                h_flex()
                    .gap_3()
                    .child(row("Light theme", "Worn in light"))
                    .child(select(&self.light)),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(row("Dark theme", "Worn in dark"))
                    .child(select(&self.dark)),
            )
            .child(
                h_flex()
                    .gap_3()
                    .child(row("More themes", "From the Zed editor's registry"))
                    .child(
                        Button::new("browse-themes")
                            .small()
                            .outline()
                            .label(if self.browsing { "Done" } else { "Browse…" })
                            .on_click(cx.listener(|form, _, window, cx| {
                                form.themes.browse(window, cx);
                            })),
                    ),
            )
            .when(self.browsing, |column| column.child(self.browser.clone()))
    }
}
