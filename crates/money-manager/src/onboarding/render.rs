use gpui_adaptive::prim::{column, row};
use gpui_adaptive::{
    ActiveScale as _, Busy, Command, Fields, Note, Screen, Segments, Stack, TextField, Tone,
};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, Window, img, rems};
use money_core::remote::StorageType;

use super::{ConnectBy, Onboarding, Step};
use crate::ui;

impl Onboarding {
    /// A command of the page, as a button across the column.
    fn button(command: Command) -> AnyElement {
        command.as_button().full_width().into_any_element()
    }

    /// The two answers, each a button that says what it leads to.
    fn render_choice(&self, cx: &mut Context<Self>) -> AnyElement {
        let (create, connect) = (cx.entity().downgrade(), cx.entity().downgrade());
        let option = |hint: &'static str, command: Command| {
            column()
                .gap(cx.scale().space_s)
                .child(Self::button(command))
                .child(Note::new(hint))
        };
        column()
            .gap(cx.scale().space_xl)
            .child(option(
                "Starts empty on this device. You can add sync storage later.",
                Command::new("create-ledger", "Create a new ledger…").on_trigger(move |_, cx| {
                    create.update(cx, |this, cx| this.go_to(Step::Create, cx)).ok();
                }),
            ))
            .child(option(
                "Brings in a ledger from a Tresse repository.",
                Command::new("connect-ledger", "Connect to sync storage…").on_trigger(move |_, cx| {
                    connect.update(cx, |this, cx| this.go_to(Step::Connect, cx)).ok();
                }),
            ))
            .into_any_element()
    }

    fn render_create(&self, cx: &mut Context<Self>) -> AnyElement {
        Fields::new()
            .child(TextField::new(&self.account_name).label("Account name"))
            .child(
                TextField::new(&self.currency)
                    .label("Currency")
                    .help("A three-letter code. It is the currency of your first account."),
            )
            .child(self.render_commit("Create", cx))
            .into_any_element()
    }

    fn render_connect(&self, cx: &mut Context<Self>) -> AnyElement {
        let by = self.connect_by;
        let (choose, backend) = (cx.entity().downgrade(), cx.entity().downgrade());
        let way = Segments::new(
            "connect-by",
            match by {
                ConnectBy::ConfigString => "string",
                ConnectBy::Details => "details",
            },
        )
        .segment("string", "Config string")
        .segment("details", "Details")
        .on_change(move |key, _, cx| {
            let by = if key.as_ref() == "string" {
                ConnectBy::ConfigString
            } else {
                ConnectBy::Details
            };
            choose
                .update(cx, |this, cx| {
                    this.connect_by = by;
                    this.error = None;
                    cx.notify();
                })
                .ok();
        });
        let fields = match by {
            ConnectBy::ConfigString => Fields::new().child(
                TextField::new(&self.config_string)
                    .label("Config string")
                    .help(
                        "Exported by another device in Settings. It holds the keys, so \
                         treat it as a password.",
                    ),
            ),
            ConnectBy::Details => self.remote.render(move |kind: StorageType, _, cx| {
                backend
                    .update(cx, |this, cx| {
                        this.remote.set_backend(kind);
                        cx.notify();
                    })
                    .ok();
            }),
        };
        Fields::new()
            .child(way)
            .child(fields)
            .child(self.render_commit("Connect", cx))
            .into_any_element()
    }

    /// The step's own commitment — the one Enter makes — and, while the
    /// first sync runs, that it does.
    fn render_commit(&self, commit: &'static str, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.is_connecting();
        let this = cx.entity().downgrade();
        column()
            .gap(cx.scale().space_m)
            .child(Self::button(
                Command::new("commit", commit)
                    .primary()
                    .disabled(busy)
                    .on_trigger(move |_, cx| {
                        this.update(cx, |this, cx| this.submit(cx)).ok();
                    }),
            ))
            .children(busy.then(|| Busy::new("connecting").label("Downloading the ledger…")))
            .into_any_element()
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
        // Back, past the first step: to the first step, but not while the
        // first sync runs.
        let up = (self.step != Step::Choose && !self.is_connecting()).then(|| {
            let this = cx.entity().downgrade();
            let up: gpui_adaptive::Handler = std::rc::Rc::new(move |_, cx| {
                this.update(cx, |this, cx| this.go_to(Step::Choose, cx)).ok();
            });
            up
        });
        Screen::new("onboarding", title)
            .root(true)
            .up(up)
            .content(
                Stack::new()
                    .width(rems(26.))
                    .child(
                        row()
                            .gap(cx.scale().space_m)
                            .child(img(ui::app_icon()).flex_none().size(rems(3.)))
                            .child(Note::new(lead)),
                    )
                    .child(body)
                    .children(
                        self.error
                            .clone()
                            .map(|error| Note::new(error).tone(Tone::Danger)),
                    ),
            )
    }
}
