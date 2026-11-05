//! The settings dialog: how the window looks, and where the ledger syncs.
//!
//! What is under Appearance takes effect as it is chosen; Save is for the
//! sync storage alone.
//!
//! The sync storage uses the same fields as the first-run form
//! ([`RemoteFields`]), and takes the same one-string form: a string can be
//! applied to fill the fields, and the fields can be copied out as a string
//! for the next device. Saving writes `.tresse/remotes.toml`; the Sync command
//! reads that file again to contact the configured origin.

use gpui_adaptive::prim::row;
use gpui_adaptive::{
    ActiveScale as _, Busy, Command, Fields, Form, Note, Notice, Purpose, Segments, Setting,
    Swatches, Text, TextField, TextFieldState, Tone,
};
use gpui_adaptive_colors::Color;
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, ClipboardItem, Context, Entity, SharedString, Window, rems};
use money_core::remote::{self, RemoteConfig};

use crate::appearance::{self, ColorChoice, InterfaceSize, SEEDS, ThemeChoice};
use crate::onboarding::RemoteFields;
use crate::settings::Settings;

mod theme_rows;

use theme_rows::ThemeRows;

struct SettingsForm {
    themes: ThemeRows,
    /// The server the sign-in page opens on; the build's default, or any
    /// other one typed over it.
    server_url: Entity<TextFieldState>,
    config_string: Entity<TextFieldState>,
    remote: RemoteFields,
    /// A remote the server page just returned, shown in the fields at the
    /// next render, where there is a window and `set_value` wants one.
    received: Option<RemoteConfig>,
    /// Whether a storage was set up when the dialog opened, which is what
    /// decides whether there is one to remove.
    had_remote: bool,
    error: Option<SharedString>,
    _connect: gpui_kit::Subscription,
}

impl SettingsForm {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let loaded = Settings::global(cx).remote();
        let error = loaded.as_ref().err().cloned().map(Into::into);
        let stored = loaded.unwrap_or_default();
        let server_url = cx.new(|cx| {
            let mut state = TextFieldState::new(Purpose::Address, window, cx);
            state.set_placeholder("https://sync.example.com", window, cx);
            state.set_value(
                crate::sync_server::default_server_url().unwrap_or_default(),
                window,
                cx,
            );
            state
        });
        let connect = cx.observe_global::<crate::sync_server::SyncConnect>(
            |this, cx| this.server_connect_changed(cx),
        );
        SettingsForm {
            themes: ThemeRows::new(window, cx),
            server_url,
            config_string: cx.new(|cx| {
                let mut state = TextFieldState::new(Purpose::Address, window, cx);
                state.set_placeholder("tresse://…", window, cx);
                state
            }),
            remote: RemoteFields::new(stored.as_ref(), window, cx),
            received: None,
            had_remote: stored.is_some(),
            error,
            _connect: connect,
        }
    }

    /// The server page's return (or its refusal): show the remote it
    /// describes in the fields, or the refusal.
    fn server_connect_changed(&mut self, cx: &mut Context<Self>) {
        if !crate::sync_server::SyncConnect::waiting(cx) {
            match crate::sync_server::SyncConnect::take_outcome(cx) {
                Some(Ok(remote)) => {
                    self.received = Some(remote);
                    self.error = None;
                }
                Some(Err(error)) => self.error = Some(error.into()),
                None => {}
            }
        }
        cx.notify();
    }

    /// Starts the browser round trip to the server named in the field.
    fn open_server_page(&mut self, cx: &mut Context<Self>) {
        if crate::sync_server::SyncConnect::waiting(cx) {
            return;
        }
        let server = self.server_url.read(cx).value(cx).to_string();
        match crate::sync_server::SyncConnect::begin(cx, &server) {
            Ok(page) => gpui_android_glue::open_url(&page, cx),
            Err(error) => self.error = Some(error.into()),
        }
        cx.notify();
    }

    /// Whether every field is empty, which is how "no storage" is said.
    fn is_blank(remote: &RemoteConfig) -> bool {
        remote.url.is_empty()
            && remote.token.is_empty()
            && remote.repo_id.is_empty()
            && remote.encryption_key.is_empty()
            && remote.s3_endpoint.is_empty()
            && remote.s3_region.is_empty()
            && remote.s3_bucket.is_empty()
            && remote.s3_access_key_id.is_empty()
            && remote.s3_secret_access_key.is_empty()
    }

    /// Fills the fields from the pasted string. A string from the server's
    /// Copy button may carry no key: a new repository gets one generated
    /// here, an existing one keeps the key of the storage on disk.
    fn apply_string(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.config_string.read(cx).value(cx);
        let previous = Settings::global(cx).remote().ok().flatten();
        match crate::sync_server::import_share_string(&text, previous.as_ref()) {
            Ok(remote) => {
                self.remote.fill(&remote, window, cx);
                self.config_string
                    .update(cx, |field, cx| field.set_value("", window, cx));
                self.error = None;
            }
            Err(error) => self.error = Some(error.into()),
        }
        cx.notify();
    }

    /// Puts the fields on the clipboard as one string, for the next device.
    fn copy_string(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let remote = match self.remote.read(cx) {
            Ok(remote) => remote,
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
                return;
            }
        };
        match remote.validate() {
            Ok(()) => {
                cx.write_to_clipboard(ClipboardItem::new_string(remote.to_share_string()));
                self.error = None;
                Notice::new("Config string copied. It holds the keys.").push(window, cx);
            }
            Err(error) => self.error = Some(error.into()),
        }
        cx.notify();
    }

    /// A new key, for a storage that has none yet. Replacing the key of a
    /// storage that already holds an encrypted ledger would lock it out, so
    /// this is only offered while the field is empty.
    fn generate_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut remote = match self.remote.read(cx) {
            Ok(remote) => remote,
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
                return;
            }
        };
        if !remote.encryption_key.is_empty() {
            return;
        }
        match remote::generate_encryption_key() {
            Ok(key) => {
                remote.encryption_key = key;
                self.remote.fill(&remote, window, cx);
                self.error = None;
            }
            Err(error) => self.error = Some(error.into()),
        }
        cx.notify();
    }

    /// Records the storage. Answers whether it did.
    fn submit(&mut self, cx: &mut Context<Self>) -> bool {
        let remote = match self.remote.read(cx) {
            Ok(remote) => remote,
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
                return false;
            }
        };
        let remote = if Self::is_blank(&remote) {
            None
        } else if let Err(error) = remote.validate() {
            self.error = Some(error.into());
            cx.notify();
            return false;
        } else {
            Some(remote)
        };
        match money_core::tresse::config::write_remote(&crate::paths::ledger_dir(), remote.as_ref())
        {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
                false
            }
        }
    }
}

impl SettingsForm {
    /// A command of the form that works on the form itself.
    fn command(
        &self,
        id: &'static str,
        label: &'static str,
        cx: &mut Context<Self>,
        run: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Command {
        let this = cx.entity().downgrade();
        Command::new(id, label).on_trigger(move |window, cx| {
            this.update(cx, |this, cx| run(this, window, cx)).ok();
        })
    }

    fn render_appearance(&self, cx: &mut Context<Self>) -> Fields {
        let current = appearance::theme_choice(cx);
        let size = appearance::interface_size(cx);
        let themes = ThemeChoice::ALL
            .iter()
            .fold(Segments::new("theme", current.label()), |segments, choice| {
                segments.segment(choice.label(), choice.label())
            })
            .on_change(|key, window, cx| {
                if let Some(choice) = ThemeChoice::ALL.iter().find(|c| c.label() == key.as_ref()) {
                    appearance::set_theme_choice(*choice, window, cx);
                }
            });
        let sizes = InterfaceSize::ALL
            .iter()
            .fold(Segments::new("interface-size", size.label()), |segments, size| {
                segments.segment(size.label(), size.label())
            })
            .on_change(|key, _, cx| {
                if let Some(size) = InterfaceSize::ALL.iter().find(|s| s.label() == key.as_ref()) {
                    appearance::set_interface_size(*size, cx);
                }
            });
        Fields::new()
            .child(Text::title("Appearance"))
            .child(
                Setting::new("Theme")
                    .detail("System follows the desktop as it changes")
                    .control(themes),
            )
            .child(
                Setting::new("Color")
                    .detail("System follows the desktop's accent color")
                    .below(color_choices(appearance::color_choice(cx))),
            )
            .children(self.themes.render(cx))
            .child(
                Setting::new("Size")
                    .detail("Text, controls and charts together")
                    .control(sizes),
            )
    }

    fn render_sync(&self, cx: &mut Context<Self>) -> Fields {
        let has_key = self
            .remote
            .read(cx)
            .is_ok_and(|remote| !remote.encryption_key.is_empty());
        let gap = cx.scale().space_s;
        let waiting = crate::sync_server::SyncConnect::waiting(cx);
        let connect_server =
            self.command("connect-server", "Connect", cx, |this, _, cx| {
                this.open_server_page(cx)
            });
        let cancel_server = self.command("cancel-server-connect", "Cancel", cx, |this, _, cx| {
            crate::sync_server::SyncConnect::cancel(cx);
            this.error = None;
            cx.notify();
        });
        let fill = self.command("apply-string", "Fill in", cx, |this, window, cx| {
            this.apply_string(window, cx)
        });
        let copy = self.command("copy-string", "Copy config string", cx, |this, window, cx| {
            this.copy_string(window, cx)
        });
        let clear = self.command("clear-sync", "Clear sync fields", cx, |this, window, cx| {
            this.remote.fill(&RemoteConfig::default(), window, cx);
            this.error = None;
            cx.notify();
        });
        let generate = (!has_key).then(|| {
            self.command("generate-key", "Generate encryption key", cx, |this, window, cx| {
                this.generate_key(window, cx)
            })
        });
        let server = if waiting {
            row()
                .gap(gap)
                .items_center()
                .child(Busy::new("waiting-for-server").label("Waiting for the server…"))
                .child(cancel_server.as_button())
        } else {
            row()
                .gap(gap)
                .items_end()
                .child(
                    gpui_kit::div().flex_1().min_w_0().child(
                        TextField::new(&self.server_url)
                            .label("Server URL")
                            .help(
                                "Any Tresse server. Yours is already filled in when this build \
                                 names one.",
                            ),
                    ),
                )
                .child(connect_server.as_button())
        };
        let this = cx.entity().downgrade();
        Fields::new()
            .child(Text::title("Sync storage"))
            .child(Note::new(
                "Sign in on a Tresse server's page, or set up the storage by hand: \
                 a Tresse repository on S3 or an HTTP server. Leave every field empty to \
                 keep the ledger on this device only.",
            ))
            .child(server)
            .child(
                row()
                    .gap(gap)
                    .items_end()
                    .child(
                        gpui_kit::div().flex_1().min_w_0().child(
                            TextField::new(&self.config_string)
                                .label("Config string")
                                .help(
                                    "Paste the string another device copied, or the server's \
                                     Copy button, to fill in the fields below.",
                                ),
                        ),
                    )
                    .child(fill.as_button()),
            )
            .child(self.remote.render(move |kind, _, cx| {
                this.update(cx, |this, cx| {
                    this.remote.set_backend(kind);
                    cx.notify();
                })
                .ok();
            }))
            .child(
                row()
                    .flex_wrap()
                    .gap(gap)
                    .child(copy.as_button())
                    .child(clear.as_button())
                    .children(generate.map(Command::as_button)),
            )
            .children(self.had_remote.then(|| {
                Note::new(
                    "Emptying the fields stops syncing. \
                     The ledger on this device and in the bucket both stay.",
                )
            }))
            .children(
                self.error
                    .clone()
                    .map(|error| Note::new(error).tone(Tone::Danger)),
            )
    }
}

impl Render for SettingsForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // A remote the server page returned lands in the fields here, where
        // there is a window to set them with.
        if let Some(remote) = self.received.take() {
            self.remote.fill(&remote, window, cx);
        }
        let space = cx.scale().space_xl;
        gpui_adaptive::prim::column()
            .gap(space)
            .child(self.render_appearance(cx))
            .child(self.render_sync(cx))
    }
}

/// The system's colour, then ours as swatches. A swatch shows the seed
/// itself rather than what a scheme makes of it, since that is what tells
/// them apart.
fn color_choices(current: ColorChoice) -> Swatches {
    let picked = match current {
        ColorChoice::System => None,
        ColorChoice::Seed(color) => SEEDS
            .into_iter()
            .find(|(stored, _)| Color::parse_hex(stored) == Some(color))
            .map(|(stored, _)| stored),
    };
    SEEDS
        .into_iter()
        .filter_map(|(stored, name)| Some((stored, name, Color::parse_hex(stored)?)))
        .fold(
            Swatches::new("colors", picked).unset("System"),
            |swatches, (stored, name, color)| {
                swatches.swatch(stored, name, gpui_adaptive_colors::hsla(color))
            },
        )
        .on_pick(|key, _, cx| {
            let choice = key
                .and_then(|key| Color::parse_hex(key))
                .map_or(ColorChoice::System, ColorChoice::Seed);
            appearance::set_color_choice(choice, cx);
        })
}

/// Opens the settings. Nothing in them reads or writes the ledger: the
/// storage they record is used by the Sync command, not from here.
pub fn open(window: &mut Window, cx: &mut App) {
    let form = cx.new(|cx| SettingsForm::new(window, cx));
    let saving = form.clone();
    Form::new("Settings", form)
        .commit("Save", move |_, cx| saving.update(cx, |form, cx| form.submit(cx)))
        .width(rems(44.))
        .open(window, cx);
}
