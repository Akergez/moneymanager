//! The settings dialog: how the window looks, and where the ledger syncs.
//!
//! The sync storage is the same seven fields the first-run form asks for
//! ([`RemoteFields`]), and takes the same one-string form: a string can be
//! applied to fill the fields, and the fields can be copied out as a string
//! for the next device. Nothing is contacted from here — saving only records
//! the storage; the Sync command is what uses it.

use gpui_kit::component::button::{Button, ButtonGroup};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{
    ActiveTheme, Selectable, Sizable, Size, StyledExt, WindowExt, h_flex, v_flex,
};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, ClipboardItem, Context, Entity, SharedString, Window, div};
use money_core::remote::{self, RemoteConfig};

use crate::appearance::{self, ThemeChoice};
use crate::onboarding::RemoteFields;
use crate::settings::Settings;
use crate::ui;

struct SettingsForm {
    config_string: Entity<InputState>,
    remote: RemoteFields,
    /// Whether a storage was set up when the dialog opened, which is what
    /// decides whether there is one to remove.
    had_remote: bool,
    error: Option<SharedString>,
}

impl SettingsForm {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let stored = Settings::global(cx).remote();
        SettingsForm {
            config_string: cx.new(|cx| InputState::new(window, cx).placeholder("tresse1:…")),
            remote: RemoteFields::new(stored.as_ref(), window, cx),
            had_remote: stored.is_some(),
            error: None,
        }
    }

    /// Whether every field is empty, which is how "no storage" is said.
    fn is_blank(remote: &RemoteConfig) -> bool {
        *remote == RemoteConfig::default()
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
                self.error = Some("That is not a config string. It starts with tresse1:".into())
            }
        }
        cx.notify();
    }

    /// Puts the fields on the clipboard as one string, for the next device.
    fn copy_string(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let remote = self.remote.read(cx);
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
        let mut remote = self.remote.read(cx);
        if remote.encryption_key.is_some() {
            return;
        }
        match remote::generate_encryption_key() {
            Ok(key) => {
                remote.encryption_key = Some(key);
                self.remote.fill(&remote, window, cx);
                self.error = None;
            }
            Err(error) => self.error = Some(error.into()),
        }
        cx.notify();
    }

    /// Records the storage. Answers whether it did.
    fn submit(&mut self, cx: &mut Context<Self>) -> bool {
        let remote = self.remote.read(cx);
        let remote = if Self::is_blank(&remote) {
            None
        } else if let Err(error) = remote.validate() {
            self.error = Some(error.into());
            cx.notify();
            return false;
        } else {
            Some(remote)
        };
        Settings::update(cx, |settings| settings.set_remote(remote));
        true
    }
}

impl Render for SettingsForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let current = appearance::theme_choice(cx);
        let has_key = self.remote.read(cx).encryption_key.is_some();

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
                    .child(section("Sync storage"))
                    .child(div().text_sm().text_color(muted).child(
                        "An S3-compatible bucket the ledger is kept in step with. \
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
                    .child(self.remote.render(Size::Medium))
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
