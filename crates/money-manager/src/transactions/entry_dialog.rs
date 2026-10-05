//! The record dialog: making an expense or an income, changing one, and the
//! way to delete one.
//!
//! A record rewritten here is rewritten whole — category, amount, comment,
//! date and account — because that is the only kind of update the ledger's
//! merge has: a newer write replaces the record, it does not patch a field.
//!
//! A transfer leg is not a record of its own. It can be looked at and its
//! transfer deleted, both legs at once; there is nothing of it to edit here.

use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::calendar::Date;
use gpui_kit::component::date_picker::{DatePicker, DatePickerState};
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::select::{Select, SelectState};
use gpui_kit::component::{ActiveTheme, IndexPath, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Entity, SharedString, Window, div};
use money_core::format::{format_amount, format_money, parse_amount};

use crate::book::{self, Book, Entry, EntryDraft, Mode};
use crate::ui::{self, Choice};

struct EntryForm {
    book: Entity<Book>,
    mode: Mode,
    /// The record being changed, or nothing for a new one.
    editing: Option<Entry>,
    category: Entity<SelectState<Vec<Choice>>>,
    amount: Entity<InputState>,
    date: Entity<DatePickerState>,
    comment: Entity<InputState>,
    error: Option<SharedString>,
}

impl EntryForm {
    fn new(
        book: Entity<Book>,
        mode: Mode,
        editing: Option<Entry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let choices: Vec<Choice> = book
            .read(cx)
            .categories(mode)
            .into_iter()
            .map(|category| Choice::new(category.id, category.name))
            .collect();
        // A new record starts on the first category; an edited one on its
        // own, or on none if that category no longer exists.
        let selected = match &editing {
            Some(entry) => choices.iter().position(|c| c.id == entry.category_id),
            None => (!choices.is_empty()).then_some(0),
        };
        let category = cx.new(|cx| {
            SelectState::new(
                choices,
                selected.map(|row| IndexPath::default().row(row)),
                window,
                cx,
            )
        });
        let amount = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder("0.00");
            match &editing {
                // Without the thousands spaces, so it parses back as typed.
                Some(entry) => state.default_value(format_amount(entry.amount).replace(' ', "")),
                None => state,
            }
        });
        // A new record starts on the day of the latest one of its kind, not
        // on today: what is being entered usually continues from there.
        let day = match &editing {
            Some(entry) => entry.date,
            None => book::latest_date(&book.read(cx).entries(mode)).unwrap_or_else(crate::today),
        };
        let date = cx.new(|cx| {
            let mut picker = DatePickerState::new(window, cx).date_format("%Y-%m-%d");
            picker.set_date(day, window, cx);
            picker
        });
        let comment = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder("Optional");
            match editing.as_ref().and_then(|entry| entry.comment.clone()) {
                Some(comment) => state.default_value(comment),
                None => state,
            }
        });
        EntryForm {
            book,
            mode,
            editing,
            category,
            amount,
            date,
            comment,
            error: None,
        }
    }

    /// Reads the form and writes the record. Answers whether it did: `false`
    /// keeps the dialog up with what is wrong said under the fields.
    fn submit(&mut self, cx: &mut Context<Self>) -> bool {
        let result = self.read(cx).and_then(|draft| {
            let (mode, id) = (
                self.mode,
                self.editing.as_ref().map(|entry| entry.id.clone()),
            );
            self.book.update(cx, |book, cx| {
                book.save_entry(mode, id.as_deref(), &draft, cx)
            })
        });
        match result {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
                false
            }
        }
    }

    fn read(&self, cx: &App) -> Result<EntryDraft, String> {
        let category_id = self
            .category
            .read(cx)
            .selected_value()
            .cloned()
            .ok_or("Choose a category.")?;
        let amount = parse_amount(&self.amount.read(cx).value())
            .filter(|amount| *amount > 0.0)
            .ok_or("Amount must be a number greater than 0.")?;
        let Date::Single(Some(date)) = self.date.read(cx).date() else {
            return Err("Choose a date.".to_string());
        };
        let comment = self.comment.read(cx).value().trim().to_string();
        Ok(EntryDraft {
            category_id,
            amount,
            comment: (!comment.is_empty()).then_some(comment),
            date,
        })
    }
}

impl Render for EntryForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let currency = self.book.read(cx).currency().to_string();
        v_flex()
            .gap_3()
            .child(ui::field("Category", Select::new(&self.category)))
            .child(ui::field(
                format!("Amount, {currency}"),
                Input::new(&self.amount),
            ))
            .child(ui::field("Date", DatePicker::new(&self.date)))
            .child(ui::field("Comment", Input::new(&self.comment)))
            .children(self.error.clone().map(|error| ui::form_error(error, cx)))
            // Deleting is on the object it deletes, apart from the footer's
            // commitment so the two are never pressed for each other.
            .children(self.editing.clone().map(|entry| {
                let (book, mode) = (self.book.clone(), self.mode);
                h_flex().pt_2().child(
                    Button::new("delete-record")
                        .label("Delete…")
                        .danger()
                        .outline()
                        .on_click(move |_, window, cx| {
                            window.close_dialog(cx);
                            confirm_delete(&book, mode, entry.clone(), window, cx);
                        }),
                )
            }))
    }
}

/// Opens the dialog for a new record, or for `entry`.
pub fn open_entry_dialog(
    book: &Entity<Book>,
    mode: Mode,
    entry: Option<Entry>,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(leg) = entry.as_ref().filter(|entry| entry.is_transfer) {
        return open_transfer_leg(book, mode, leg.clone(), window, cx);
    }
    // A record has to be of some category, so with none the first step is
    // making one — said, rather than left as a form that cannot be saved.
    if entry.is_none() && book.read(cx).categories(mode).is_empty() {
        window.push_notification(
            Notification::info(format!(
                "Make a category first: every {} belongs to one.",
                mode.noun()
            )),
            cx,
        );
        return crate::categories::open_category_dialog(book, mode, None, window, cx);
    }

    let (title, commit) = match &entry {
        Some(_) => (format!("Edit {}", mode.noun()), "Save"),
        None => (format!("New {}", mode.noun()), "Add"),
    };
    let form = cx.new(|cx| EntryForm::new(book.clone(), mode, entry, window, cx));
    // The amount is what is typed first, nearly every time.
    let amount = form.read(cx).amount.clone();
    ui::open_form(
        title,
        commit,
        form,
        Some(amount),
        EntryForm::submit,
        window,
        cx,
    );
}

/// What a transfer leg opens: what it is, and the one thing to do with it.
fn open_transfer_leg(
    book: &Entity<Book>,
    mode: Mode,
    leg: Entry,
    window: &mut Window,
    cx: &mut App,
) {
    let currency = book.read(cx).currency().to_string();
    let book = book.clone();
    window.open_dialog(cx, move |dialog, _, cx| {
        let muted = cx.theme().muted_foreground;
        let (book, leg_to_delete) = (book.clone(), leg.clone());
        dialog
            .title("Transfer")
            .child(
                v_flex()
                    .gap_2()
                    .child(format!(
                        "{}{} on {}",
                        mode.sign(),
                        format_money(leg.amount, &currency),
                        leg.date.format("%Y-%m-%d")
                    ))
                    .children(
                        leg.comment
                            .clone()
                            .map(|comment| div().text_sm().text_color(muted).child(comment)),
                    )
                    .child(div().text_sm().text_color(muted).child(
                        "This is one side of a transfer between two accounts. \
                         Deleting it removes both sides.",
                    )),
            )
            .footer(
                DialogFooter::new()
                    .child(
                        Button::new("delete-transfer")
                            .label("Delete transfer…")
                            .danger()
                            .outline()
                            .on_click(move |_, window, cx| {
                                window.close_dialog(cx);
                                confirm_delete(&book, mode, leg_to_delete.clone(), window, cx);
                            }),
                    )
                    .child(DialogClose::new().child(Button::new("close").label("Close"))),
            )
    });
}

/// Asks before deleting: there is no undo, and on a synced ledger the
/// deletion reaches every device.
fn confirm_delete(
    book: &Entity<Book>,
    mode: Mode,
    entry: Entry,
    window: &mut Window,
    cx: &mut App,
) {
    let currency = book.read(cx).currency().to_string();
    let amount = format_money(entry.amount, &currency);
    let (title, consequence) = if entry.is_transfer {
        (
            format!("Delete this transfer of {amount}?"),
            "Both accounts lose their side of it. This can’t be undone.",
        )
    } else {
        (
            format!("Delete this {} of {amount}?", mode.noun()),
            "This can’t be undone.",
        )
    };
    let book = book.clone();
    window.open_alert_dialog(cx, move |alert, _, _| {
        let (book, entry) = (book.clone(), entry.clone());
        alert
            .title(title.clone())
            .description(consequence)
            .confirm()
            .ok_text("Delete")
            .ok_variant(ButtonVariant::Danger)
            .on_ok(move |_, window, cx| {
                let deleted = book.update(cx, |book, cx| book.delete_entry(mode, &entry, cx));
                if let Err(error) = deleted {
                    window.push_notification(Notification::error(error), cx);
                }
                true
            })
    });
}
