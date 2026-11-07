//! The settings dialog: how the window looks, and where the ledger syncs.
//!
//! What is under Appearance takes effect as it is chosen; Save is for the
//! sync storage alone.
//!
//! Sync has the two tabs of the first-run page, less its "Start without
//! sync": Server (a server's sign-in page, a config string, and exporting
//! the storage as one for the next device) and Custom storage (every field,
//! [`RemoteFields`]). Both edit the one storage that Save writes to
//! `.tresse/remotes.toml`; the Sync command reads that file again to contact
//! the configured origin. Under them, the Tresse storage on this device can be
//! rebuilt from the ledger, which also turns sync off.

use gpui_adaptive::prim::row;
use gpui_adaptive::{
    ActiveScale as _, Busy, Command, Confirm, Fields, Form, Note, Notice, Purpose, Segments,
    Setting, Swatches, Text, TextField, TextFieldState, Tone,
};
use gpui_adaptive_colors::Color;
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, ClipboardItem, Context, Entity, SharedString, Window, rems};
use money_core::remote::RemoteConfig;

use crate::appearance::{self, ColorChoice, InterfaceSize, SEEDS, ThemeChoice};
use crate::book::Book;
use crate::onboarding::{RemoteFields, Tab, describe_remote};
use crate::settings::Settings;

mod theme_rows;

use theme_rows::ThemeRows;

struct SettingsForm {
    themes: ThemeRows,
    /// The ledger whose storage the form can rebuild; none when the form is
    /// open without one.
    book: Option<Entity<Book>>,
    tab: Tab,
    /// The server the sign-in page opens on; the build's default, or any
    /// other one typed over it.
    server_url: Entity<TextFieldState>,
    config_string: Entity<TextFieldState>,
    remote: RemoteFields,
    /// The storage as it is saved, to tell an unsaved change from it.
    saved: Option<RemoteConfig>,
    /// A remote the server page just returned, shown in the fields at the
    /// next render, where there is a window and `set_value` wants one.
    received: Option<RemoteConfig>,
    error: Option<SharedString>,
    _connect: gpui_kit::Subscription,
}

impl SettingsForm {
    fn new(book: Option<Entity<Book>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let loaded = Settings::global(cx).remote();
        let error = loaded.as_ref().err().cloned().map(Into::into);
        let stored = loaded.unwrap_or_default();
        let field = |purpose: Purpose,
                     value: &str,
                     placeholder: &'static str,
                     window: &mut Window,
                     cx: &mut Context<Self>| {
            let value = value.to_string();
            cx.new(|cx| {
                let mut state = TextFieldState::new(purpose, window, cx);
                state.set_placeholder(placeholder, window, cx);
                state.set_value(value, window, cx);
                state
            })
        };
        // A storage on a server opens on the server it is on.
        let server = stored
            .as_ref()
            .filter(|remote| remote.storage_type == money_core::remote::StorageType::Http)
            .map(|remote| remote.url.clone())
            .or_else(|| crate::sync_server::default_server_url().map(str::to_string))
            .unwrap_or_default();
        let server_url = field(Purpose::Address, &server, "https://…", window, cx);
        let config_string = field(Purpose::Address, "", "tresse://…", window, cx);
        let connect = cx.observe_global::<crate::sync_server::SyncConnect>(|this, cx| {
            this.server_connect_changed(cx)
        });
        SettingsForm {
            themes: ThemeRows::new(window, cx),
            book,
            tab: Tab::Server,
            server_url,
            config_string,
            remote: RemoteFields::new(stored.as_ref(), window, cx),
            saved: stored,
            received: None,
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
                // At once, not when the window next loses focus: a paste
                // before that (into this app's own field, through the edit
                // menu) reads the phone's clipboard first, and would bring
                // back whatever was on it before the export.
                gpui_android_glue::clipboard::push(cx);
                self.error = None;
                Notice::new("Config string copied. It holds the keys.").push(window, cx);
            }
            Err(error) => self.error = Some(error.into()),
        }
        cx.notify();
    }

    fn generate_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.error = self.remote.generate_key(window, cx).err().map(Into::into);
        cx.notify();
    }

    /// Writes the ledger into fresh Tresse storage, after asking. Every
    /// record stays; the history and the sync settings on this device go.
    fn ask_to_rebuild(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(book) = self.book.clone() else {
            return;
        };
        if book.read(cx).is_syncing() {
            self.error = Some("Wait for the sync to finish first.".into());
            cx.notify();
            return;
        }
        let rebuild = Command::new("rebuild-storage-yes", "Rebuild storage")
            .destructive()
            .on_trigger(move |window, cx| {
                crate::sync_server::SyncConnect::cancel(cx);
                match book.update(cx, |book, cx| book.rebuild_storage(cx)) {
                    Ok(records) => {
                        // The form still holds the storage just removed;
                        // saving it would set it up again.
                        Form::close(window, cx);
                        Notice::new(format!(
                            "Storage rebuilt with {records} records. Sync is off until you set it up again."
                        ))
                        .tone(Tone::Success)
                        .push(window, cx);
                    }
                    Err(error) => Notice::error(format!("Couldn’t rebuild the storage. ({error})"))
                        .push(window, cx),
                }
            });
        Confirm::new("rebuild-storage", "Rebuild Tresse storage?")
            .body(
                "Every record stays. The version history and the sync settings on this device are \
                 removed, and the ledger stops syncing. Storage on a server or in a bucket is not \
                 changed: connect to a new one afterwards.",
            )
            .confirm(rebuild)
            .open(window, cx);
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
        let remote = if RemoteFields::is_blank(&remote) {
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
            .fold(
                Segments::new("theme", current.label()),
                |segments, choice| segments.segment(choice.label(), choice.label()),
            )
            .on_change(|key, window, cx| {
                if let Some(choice) = ThemeChoice::ALL.iter().find(|c| c.label() == key.as_ref()) {
                    appearance::set_theme_choice(*choice, window, cx);
                }
            });
        let sizes = InterfaceSize::ALL
            .iter()
            .fold(
                Segments::new("interface-size", size.label()),
                |segments, size| segments.segment(size.label(), size.label()),
            )
            .on_change(|key, _, cx| {
                if let Some(size) = InterfaceSize::ALL
                    .iter()
                    .find(|s| s.label() == key.as_ref())
                {
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

    /// Where the ledger syncs now, and whether the form holds a change of
    /// it that Save has yet to make.
    fn render_status(&self, cx: &mut Context<Self>) -> Note {
        let current = self
            .remote
            .read(cx)
            .ok()
            .filter(|r| !RemoteFields::is_blank(r));
        match (&self.saved, current) {
            (Some(saved), Some(current)) if *saved == current => {
                Note::new(format!("Syncing with {}.", describe_remote(saved)))
            }
            (_, Some(current)) => {
                Note::new(format!("Save to sync with {}.", describe_remote(&current)))
                    .tone(Tone::Info)
            }
            (Some(_), None) => Note::new(
                "Save to stop syncing. The ledger stays on this device and in the storage.",
            )
            .tone(Tone::Info),
            (None, None) => Note::new("Not syncing: the ledger is only on this device."),
        }
    }

    /// The short way: a server's sign-in page, a config string, and the
    /// storage exported as one.
    fn render_server_tab(&self, cx: &mut Context<Self>) -> Fields {
        let gap = cx.scale().space_s;
        let waiting = crate::sync_server::SyncConnect::waiting(cx);
        let sign_in = if waiting {
            let cancel = self.command("cancel-server-connect", "Cancel", cx, |this, _, cx| {
                crate::sync_server::SyncConnect::cancel(cx);
                this.error = None;
                cx.notify();
            });
            row()
                .gap(gap)
                .items_center()
                .child(Busy::new("waiting-for-server").label("Waiting for the browser…"))
                .child(cancel.as_button())
        } else {
            let open = self.command("connect-server", "Sign in", cx, |this, _, cx| {
                this.open_server_page(cx)
            });
            row()
                .gap(gap)
                .items_end()
                .child(
                    gpui_kit::div()
                        .flex_1()
                        .min_w_0()
                        .child(TextField::new(&self.server_url).label("Server")),
                )
                .child(open.as_button())
        };
        let fill = self.command("apply-string", "Use", cx, |this, window, cx| {
            this.apply_string(window, cx)
        });
        let export = self.command(
            "copy-string",
            "Export config string",
            cx,
            |this, window, cx| this.copy_string(window, cx),
        );
        let has_storage = self
            .remote
            .read(cx)
            .is_ok_and(|remote| !RemoteFields::is_blank(&remote));
        Fields::new()
            .child(sign_in)
            .child(Note::new(
                "Sign in opens the server in the browser. Once you sign in there, it brings you back here.",
            ))
            .child(
                row()
                    .gap(gap)
                    .items_end()
                    .child(
                        gpui_kit::div()
                            .flex_1()
                            .min_w_0()
                            .child(TextField::new(&self.config_string).label("Config string")),
                    )
                    .child(fill.as_button()),
            )
            .child(Note::new(
                "From another device or the server's page, if the browser did not bring you back.",
            ))
            .children(has_storage.then(|| {
                Setting::new("Another device")
                    .detail("Copies this storage as a config string. It holds the keys: keep it like a password.")
                    .control(export.as_button())
            }))
    }

    /// Every field of the storage.
    fn render_custom_tab(&self, cx: &mut Context<Self>) -> Fields {
        let gap = cx.scale().space_s;
        let clear = self.command("clear-sync", "Clear fields", cx, |this, window, cx| {
            this.remote.fill(&RemoteConfig::default(), window, cx);
            this.error = None;
            cx.notify();
        });
        let generate = (!self.remote.has_key(cx)).then(|| {
            self.command(
                "generate-key",
                "Generate encryption key",
                cx,
                |this, window, cx| this.generate_key(window, cx),
            )
        });
        let this = cx.entity().downgrade();
        self.remote
            .render(move |kind, _, cx| {
                this.update(cx, |this, cx| {
                    this.remote.set_backend(kind);
                    cx.notify();
                })
                .ok();
            })
            .child(
                row()
                    .flex_wrap()
                    .gap(gap)
                    .children(generate.map(Command::as_button))
                    .child(clear.as_button()),
            )
    }

    fn render_sync(&self, cx: &mut Context<Self>) -> Fields {
        let tabs = {
            let this = cx.entity().downgrade();
            Segments::new("sync-tab", self.tab.key())
                .segment(Tab::Server.key(), "Server")
                .segment(Tab::Custom.key(), "Custom storage")
                .on_change(move |key, _, cx| {
                    let tab = Tab::from_key(key);
                    this.update(cx, |this, cx| {
                        this.tab = tab;
                        cx.notify();
                    })
                    .ok();
                })
        };
        let body = match self.tab {
            Tab::Server => self.render_server_tab(cx),
            Tab::Custom => self.render_custom_tab(cx),
        };
        Fields::new()
            .child(Text::title("Sync"))
            .child(self.render_status(cx))
            .child(tabs)
            .child(body)
            .children(
                self.error
                    .clone()
                    .map(|error| Note::new(error).tone(Tone::Danger)),
            )
    }

    /// The Tresse storage on this device, apart from where it syncs.
    fn render_storage(&self, cx: &mut Context<Self>) -> Option<Fields> {
        self.book.as_ref()?;
        let rebuild = self.command("rebuild-storage", "Rebuild…", cx, |this, window, cx| {
            this.ask_to_rebuild(window, cx)
        });
        Some(
            Fields::new().child(Text::title("Storage")).child(
                Setting::new("Rebuild Tresse storage")
                    .detail(
                        "Writes the ledger into fresh storage, for data an earlier version left. \
                         Version history on this device is removed and sync is turned off.",
                    )
                    .control(rebuild.as_button()),
            ),
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
            .children(self.render_storage(cx))
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

/// Opens the settings. The storage they record is used by the Sync command,
/// not from here; `book` is the ledger whose storage they can rebuild.
pub fn open(book: Option<Entity<Book>>, window: &mut Window, cx: &mut App) {
    let form = cx.new(|cx| SettingsForm::new(book, window, cx));
    let saving = form.clone();
    Form::new("Settings", form)
        .commit("Save", move |_, cx| {
            saving.update(cx, |form, cx| form.submit(cx))
        })
        .width(rems(44.))
        .open(window, cx);
}
