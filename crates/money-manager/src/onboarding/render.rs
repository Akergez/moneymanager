use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::Input;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme, Disableable, Sizable, Size, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, Window, div, img};

use super::{ConnectBy, Onboarding, Step};
use crate::ui;

impl Onboarding {
    /// The two answers, each a button that says what it leads to.
    fn render_choice(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let option = |hint: &'static str, button: Button| {
            v_flex()
                .gap_2()
                .child(button.large().w_full())
                .child(div().text_sm().text_center().text_color(muted).child(hint))
        };
        v_flex()
            .gap_6()
            .child(option(
                "Starts empty on this device. You can add sync storage later.",
                Button::new("create-ledger")
                    .label("Create a new ledger…")
                    .on_click(cx.listener(|this, _, _, cx| this.go_to(Step::Create, cx))),
            ))
            .child(option(
                "Brings in a ledger that already syncs with S3 storage.",
                Button::new("connect-ledger")
                    .label("Connect to sync storage…")
                    .on_click(cx.listener(|this, _, _, cx| this.go_to(Step::Connect, cx))),
            ))
            .into_any_element()
    }

    fn render_create(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .gap_4()
            .child(
                ui::field("Currency", Input::new(&self.currency).large()).child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("A three-letter code. It is the currency of your first account."),
                ),
            )
            .child(self.render_actions("Create", cx))
            .into_any_element()
    }

    fn render_connect(&self, cx: &mut Context<Self>) -> AnyElement {
        let by = self.connect_by;
        v_flex()
            .gap_4()
            .child(
                TabBar::new("connect-by")
                    .segmented()
                    .w_full()
                    .selected_index(match by {
                        ConnectBy::ConfigString => 0,
                        ConnectBy::Details => 1,
                    })
                    .child(Tab::new().label("Config string"))
                    .child(Tab::new().label("Details"))
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        this.connect_by = if *index == 0 {
                            ConnectBy::ConfigString
                        } else {
                            ConnectBy::Details
                        };
                        this.error = None;
                        cx.notify();
                    })),
            )
            .child(match by {
                ConnectBy::ConfigString => {
                    ui::field("Config string", Input::new(&self.config_string).large()).child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                "Exported by another device in Settings. It holds the keys, so \
                             treat it as a password.",
                            ),
                    )
                }
                ConnectBy::Details => self.remote.render(Size::Large),
            })
            .child(self.render_actions("Connect", cx))
            .into_any_element()
    }

    /// Back, and the step's own commitment — the one Enter makes.
    fn render_actions(&self, commit: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.is_connecting();
        h_flex()
            .gap_2()
            .child(
                Button::new("back")
                    .label("Back")
                    .large()
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| this.go_to(Step::Choose, cx))),
            )
            .child(div().flex_1())
            .when(busy, |row| row.child(Spinner::new()))
            .child(
                Button::new("commit")
                    .label(commit)
                    .primary()
                    .large()
                    .disabled(busy)
                    .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
            )
    }
}

impl Render for Onboarding {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, lead) = match self.step {
            Step::Choose => (
                "Set up your ledger",
                "Everything is kept inside the application, on this device.",
            ),
            Step::Create => ("New ledger", "You can add more accounts afterwards."),
            Step::Connect => (
                "Connect to sync storage",
                "The ledger is downloaded once and kept in step from then on.",
            ),
        };
        let body = match self.step {
            Step::Choose => self.render_choice(cx),
            Step::Create => self.render_create(cx),
            Step::Connect => self.render_connect(cx),
        };

        let form = v_flex()
            .w_96()
            .max_w_full()
            .gap_6()
            .child(
                v_flex()
                    .items_center()
                    .gap_2()
                    .child(img(ui::app_icon()).size_16())
                    .child(div().text_xl().font_semibold().child(title))
                    .child(
                        div()
                            .text_sm()
                            .text_center()
                            .text_color(cx.theme().muted_foreground)
                            .child(lead),
                    ),
            )
            .child(body)
            .children(
                self.error
                    .clone()
                    .map(|error| ui::form_error(error, cx).text_center()),
            );

        // The page scrolls as a whole: on a phone the details form is taller
        // than the screen, and the keyboard takes half of that.
        div()
            .id("onboarding")
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .min_h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .p_6()
                    .child(form),
            )
    }
}
