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

use gpui_adaptive_colors::Color;
use gpui_kit::component::button::{Button, ButtonGroup};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{
    ActiveTheme, Selectable, Sizable, Size, StyledExt, WindowExt, h_flex, v_flex,
};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, ClipboardItem, Context, Entity, SharedString, Window, div};
use money_core::remote::{self, RemoteConfig};

use crate::appearance::{self, ColorChoice, InterfaceSize, SEEDS, ThemeChoice};
use crate::onboarding::RemoteFields;
use crate::settings::Settings;
use crate::ui;

mod theme_rows;

use theme_rows::ThemeRows;

struct SettingsForm {
    themes: ThemeRows,
    config_string: Entity<InputState>,
    remote: RemoteFields,
    /// Whether a storage was set up when the dialog opened, which is what
    /// decides whether there is one to remove.
    had_remote: bool,
    error: Option<SharedString>,
}

impl SettingsForm {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let loaded = Settings::global(cx).remote();
        let error = loaded.as_ref().err().cloned().map(Into::into);
        let stored = loaded.unwrap_or_default();
        SettingsForm {
            themes: ThemeRows::new(window, cx),
            config_string: cx.new(|cx| InputState::new(window, cx).placeholder("tresse://…")),
            remote: RemoteFields::new(stored.as_ref(), window, cx),
            had_remote: stored.is_some(),
            error,
        }
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

    /// Fills the fields from the pasted string.
    fn apply_string(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.config_string.read(cx).value();
        match RemoteConfig::from_share_string(&text) {
            Ok(remote) => {
                self.remote.fill(&remote, window, cx);
                self.config_string
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.error = None;
            }
            Err(_) => {
                self.error = Some("That is not a config string. It starts with tresse://".into())
            }
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
                window.push_notification(
                    Notification::info("Config string copied. It holds the keys."),
                    cx,
                );
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

impl Render for SettingsForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let current = appearance::theme_choice(cx);
        let current_size = appearance::interface_size(cx);
        let has_key = self
            .remote
            .read(cx)
            .is_ok_and(|remote| !remote.encryption_key.is_empty());

        let section = |title: &'static str| div().font_semibold().child(title);
        // The dialog is as tall as a small window already, so its body is
        // what scrolls and the footer stays where it is.
        div()
            .id("settings-body")
            .max_h_96()
            .overflow_y_scroll()
            .child(
                v_flex()
                    .gap_4()
                    .pr_2()
                    .child(section("Appearance"))
                    .child(
                        h_flex()
                            .gap_3()
                            .child(
                                v_flex().flex_1().min_w_0().child("Theme").child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child("System follows the desktop as it changes"),
                                ),
                            )
                            .child(
                                ButtonGroup::new("theme")
                                    .outline()
                                    .small()
                                    .children(ThemeChoice::ALL.map(|choice| {
                                        Button::new(choice.label())
                                            .label(choice.label())
                                            .selected(choice == current)
                                    }))
                                    .on_click(|picked, window, cx| {
                                        if let Some(choice) = picked
                                            .first()
                                            .and_then(|index| ThemeChoice::ALL.get(*index))
                                        {
                                            appearance::set_theme_choice(*choice, window, cx);
                                        }
                                    }),
                            ),
                    )
                    // Eight choices do not fit beside their label the way
                    // three do, so they go under it.
                    .child(
                        v_flex()
                            .gap_2()
                            .child(
                                v_flex().child("Color").child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child("System follows the desktop's accent color"),
                                ),
                            )
                            .child(color_choices(appearance::color_choice(cx), cx)),
                    )
                    .child(self.themes.render(cx))
                    .child(
                        h_flex()
                            .gap_3()
                            .child(
                                v_flex().flex_1().min_w_0().child("Size").child(
                                    div()
                                        .text_xs()
                                        .text_color(muted)
                                        .child("Text, controls and charts together"),
                                ),
                            )
                            .child(
                                ButtonGroup::new("interface-size")
                                    .outline()
                                    .small()
                                    .children(InterfaceSize::ALL.map(|size| {
                                        Button::new(size.label())
                                            .label(size.label())
                                            .selected(size == current_size)
                                    }))
                                    .on_click(|picked, _, cx| {
                                        if let Some(size) = picked
                                            .first()
                                            .and_then(|index| InterfaceSize::ALL.get(*index))
                                        {
                                            appearance::set_interface_size(*size, cx);
                                        }
                                    }),
                            ),
                    )
                    .child(section("Sync storage"))
                    .child(div().text_sm().text_color(muted).child(
                        "A Tresse repository on S3 or an HTTP server. \
                         Leave every field empty to keep the ledger on this device only.",
                    ))
                    .child(
                        ui::field(
                            "Config string",
                            h_flex()
                                .gap_2()
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(Input::new(&self.config_string)),
                                )
                                .child(Button::new("apply-string").label("Fill in").on_click(
                                    cx.listener(|this, _, window, cx| {
                                        this.apply_string(window, cx)
                                    }),
                                )),
                        )
                        .child(div().text_xs().text_color(muted).child(
                            "Paste the string another device copied to fill in the fields below.",
                        )),
                    )
                    .child(self.remote.render(Size::Medium, cx))
                    .child(
                        h_flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                Button::new("copy-string")
                                    .label("Copy config string")
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.copy_string(window, cx)
                                    })),
                            )
                            .child(
                                Button::new("clear-sync")
                                    .label("Clear sync fields")
                                    .small()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.remote.fill(&RemoteConfig::default(), window, cx);
                                        this.error = None;
                                        cx.notify();
                                    })),
                            )
                            .when(!has_key, |row| {
                                row.child(
                                    Button::new("generate-key")
                                        .label("Generate encryption key")
                                        .small()
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.generate_key(window, cx)
                                        })),
                                )
                            }),
                    )
                    .when(self.had_remote, |column| {
                        column.child(div().text_xs().text_color(muted).child(
                            "Emptying the fields stops syncing. \
                             The ledger on this device and in the bucket both stay.",
                        ))
                    })
                    .children(self.error.clone().map(|error| ui::form_error(error, cx))),
            )
    }
}

/// The system's colour as a button, then ours as swatches. A swatch shows
/// the seed itself rather than what a scheme makes of it, since that is what
/// tells them apart; the one chosen is ringed, and the interface around it
/// has already taken its colour.
fn color_choices(current: ColorChoice, cx: &App) -> impl IntoElement {
    let (ring, gap) = (cx.theme().foreground, cx.theme().background);
    h_flex()
        .flex_wrap()
        .gap_2()
        .child(
            Button::new("color-system")
                .label("System")
                .outline()
                .small()
                .selected(current == ColorChoice::System)
                .on_click(|_, _, cx| appearance::set_color_choice(ColorChoice::System, cx)),
        )
        .children(SEEDS.into_iter().filter_map(|(stored, name)| {
            let color = Color::parse_hex(stored)?;
            let choice = ColorChoice::Seed(color);
            Some(
                div()
                    .id(stored)
                    .flex_none()
                    .size_6()
                    .rounded_full()
                    .cursor_pointer()
                    // The ring is a border in the text colour around a
                    // border in the background's, so it stands off the
                    // swatch whatever colour that is.
                    .when(current == choice, |swatch| {
                        swatch.border_2().border_color(ring)
                    })
                    .child(
                        div()
                            .size_full()
                            .rounded_full()
                            .border_2()
                            .border_color(gap)
                            .bg(gpui_adaptive_colors::hsla(color)),
                    )
                    .tooltip(move |window, cx| Tooltip::new(name).build(window, cx))
                    .on_click(move |_, _, cx| appearance::set_color_choice(choice, cx)),
            )
        }))
}

/// Opens the settings dialog. Nothing in it reads or writes the ledger: the
/// storage it records is used by the Sync command, not from here.
pub fn open(window: &mut Window, cx: &mut App) {
    let form = cx.new(|cx| SettingsForm::new(window, cx));
    ui::open_form(
        "Settings",
        "Save",
        form,
        None,
        SettingsForm::submit,
        window,
        cx,
    );
}
