use gpui_adaptive::prim::{Text, column, row};
use gpui_adaptive::{
    ActiveScale as _, Busy, Command, Fields, Note, Screen, Segments, Stack, TextField, Tone,
};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, Window, img, rems};
use money_core::remote::StorageType;

use super::{Described, Onboarding, Tab};
use crate::ui;

impl Onboarding {
    /// A command of the page that works on the page itself.
    fn command(
        &self,
        id: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
        run: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Command {
        let this = cx.entity().downgrade();
        Command::new(id, label)
            .disabled(self.is_connecting())
            .on_trigger(move |window, cx| {
                this.update(cx, |this, cx| run(this, window, cx)).ok();
            })
    }

    /// A command of the page, as a button across the column.
    fn button(command: Command) -> AnyElement {
        command.as_button().full_width().into_any_element()
    }

    /// The short way: no sync, a server's sign-in page, or a config string.
    fn render_server(&self, cx: &mut Context<Self>) -> AnyElement {
        let gap = cx.scale().space_s;
        let waiting = crate::sync_server::SyncConnect::waiting(cx);
        let start = self.command(
            "start-without-sync",
            "Start without sync",
            cx,
            |this, _, cx| this.start_without_sync(cx),
        );
        let sign_in = if waiting {
            let cancel = self.command("cancel-server-connect", "Cancel", cx, |this, _, cx| {
                crate::sync_server::SyncConnect::cancel(cx);
                this.error = None;
                cx.notify();
            });
            row()
                .gap(gap)
                .items_center()
                .justify_between()
                .child(Busy::new("waiting-for-server").label("Waiting for the browser…"))
                .child(cancel.as_button())
                .into_any_element()
        } else {
            Self::button(
                self.command("open-server", "Sign in", cx, |this, _, cx| {
                    this.open_server_page(cx)
                })
                .primary(),
            )
        };
        let paste = self.command("connect-string", "Connect", cx, |this, _, cx| {
            this.connect(Described::ConfigString, cx)
        });
        column()
            .gap(cx.scale().space_xl)
            .child(
                column()
                    .gap(gap)
                    .child(Self::button(start))
                    .child(Note::new(
                        "A new ledger, kept only on this device. Sync can be set up later in Settings.",
                    )),
            )
            .child(
                Fields::new()
                    .child(Text::title("Sync through a server"))
                    .child(TextField::new(&self.server_url).label("Server"))
                    .child(sign_in)
                    .child(Note::new(
                        "Opens the server in the browser. Once you sign in there, it brings you back here.",
                    )),
            )
            .child(
                Fields::new()
                    .child(TextField::new(&self.config_string).label("Config string").help(
                        "From another device or the server's page, if the browser did not bring you back.",
                    ))
                    .child(Self::button(paste)),
            )
            .into_any_element()
    }

    /// Every field of a storage: S3 or a Tresse HTTP server.
    fn render_custom(&self, cx: &mut Context<Self>) -> AnyElement {
        let backend = cx.entity().downgrade();
        let generate = (!self.remote.has_key(cx)).then(|| {
            self.command(
                "generate-key",
                "Generate encryption key",
                cx,
                |this, window, cx| this.generate_key(window, cx),
            )
        });
        let connect = self.command("connect-fields", "Connect", cx, |this, _, cx| {
            this.connect(Described::Fields, cx)
        });
        self.remote
            .render(move |kind: StorageType, _, cx| {
                backend
                    .update(cx, |this, cx| {
                        this.remote.set_backend(kind);
                        cx.notify();
                    })
                    .ok();
            })
            .children(generate.map(|generate| generate.as_button()))
            .child(Self::button(connect.primary()))
            .into_any_element()
    }
}

impl Render for Onboarding {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A remote the server page returned lands in the fields here, where
        // there is a window to set them with.
        if let Some(remote) = self.pending_fill.take() {
            self.remote.fill(&remote, window, cx);
        }
        let tabs = {
            let this = cx.entity().downgrade();
            Segments::new("setup-tab", self.tab.key())
                .segment(Tab::Server.key(), "Server")
                .segment(Tab::Custom.key(), "Custom storage")
                .on_change(move |key, _, cx| {
                    let tab = Tab::from_key(key);
                    this.update(cx, |this, cx| this.show_tab(tab, cx)).ok();
                })
        };
        let body = match self.tab {
            Tab::Server => self.render_server(cx),
            Tab::Custom => self.render_custom(cx),
        };
        Screen::new("onboarding", "Set up your ledger")
            .root(true)
            .content(
                Stack::new()
                    .width(rems(26.))
                    .child(
                        row()
                            .gap(cx.scale().space_m)
                            .child(img(ui::app_icon()).flex_none().size(rems(3.)))
                            .child(Note::new(
                                "Keep it on this device, or sync it between your devices.",
                            )),
                    )
                    .child(tabs)
                    .child(body)
                    .children(
                        self.is_connecting()
                            .then(|| Busy::new("connecting").label("Downloading the ledger…")),
                    )
                    .children(
                        self.error
                            .clone()
                            .map(|error| Note::new(error).tone(Tone::Danger)),
                    ),
            )
    }
}
