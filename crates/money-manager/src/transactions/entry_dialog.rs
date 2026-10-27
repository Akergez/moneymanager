//! The record dialog: making an expense or an income, changing one, and the
//! way to delete one.
//!
//! A new record is an amount and a category, in that order: the amount has
//! the focus as the form opens, and pressing a category saves the record in
//! it. Its day starts on that of the latest record and the comment stays
//! folded until asked for, so neither costs a press when it is not wanted.
//! On a phone the form is a sheet at the bottom of the window, over the
//! keyboard and under the thumb; elsewhere it is a dialog.
//!
//! A record rewritten here is rewritten whole — category, amount, comment,
//! date and account — because that is the only kind of update the ledger's
//! merge has: a newer write replaces the record, it does not patch a field.
//!
//! A transfer leg is not a record of its own. It can be looked at and its
//! transfer deleted, both legs at once; there is nothing of it to edit here.

use gpui_android_glue::keyboard::{self, Purpose};
use gpui_kit::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme, Placement, Selectable, Sizable, WindowExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, AppContext, Context, Entity, Focusable, Pixels, SharedString, Subscription, Window, div,
    rems,
};
use money_core::format::{format_amount, format_money, parse_amount};

use crate::book::{self, Book, CategoryInfo, CategoryLook, Entry, EntryDraft, Mode};
use crate::date_field::DateField;
use crate::shell::Layout;
use crate::ui::{self, Lucide, TileSize};

/// What the form is shown in, which decides how it closes and who hears
/// Enter: a dialog takes it as its commitment, a sheet has none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Surface {
    Dialog,
    Sheet,
}

impl Surface {
    /// Categories to a row of the grid. A sheet spans a phone's narrow
    /// window, where four to a row would break a long name inside a word.
    fn columns(self) -> usize {
        match self {
            Surface::Dialog => 4,
            Surface::Sheet => 3,
        }
    }

    fn close(self, window: &mut Window, cx: &mut App) {
        match self {
            Surface::Dialog => window.close_dialog(cx),
            Surface::Sheet => window.close_sheet(cx),
        }
    }
}

struct EntryForm {
    book: Entity<Book>,
    mode: Mode,
    surface: Surface,
    /// The record being changed, or nothing for a new one.
    editing: Option<Entry>,
    /// The categories of `mode`, the most used first.
    categories: Vec<CategoryInfo>,
    /// The chosen category: for a new record, the one Enter saves in.
    category: Option<Vec<u8>>,
    amount: Entity<InputState>,
    date: Entity<DateField>,
    comment: Entity<InputState>,
    /// Whether the comment field is out. A record that has a comment shows
    /// it; a new one keeps it folded until asked for.
    commenting: bool,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl EntryForm {
    fn new(
        book: Entity<Book>,
        mode: Mode,
        surface: Surface,
        editing: Option<Entry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let entries = book.read(cx).entries(mode);
        let mut categories = book.read(cx).categories(mode);
        book::order_by_use(&mut categories, |c| c.id.as_slice(), &entries);
        // A new record starts on the category of the latest one, as it does
        // on its day; an edited one on its own, or on none if that category
        // no longer exists.
        let category = match &editing {
            Some(entry) => Some(entry.category_id.clone()),
            None => book::latest_category(&entries).map(<[u8]>::to_vec),
        }
        .filter(|id| categories.iter().any(|c| &c.id == id))
        .or_else(|| match editing {
            Some(_) => None,
            None => categories.first().map(|c| c.id.clone()),
        });
        let amount = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder("0.00");
            match &editing {
                // Without the thousands spaces, so it parses back as typed.
                Some(entry) => state.default_value(format_amount(entry.amount).replace(' ', "")),
                None => state,
            }
        });
        ui::keyboard_for(&amount, Purpose::Number, cx);
        // A new record starts on the day of the latest one of its kind, not
        // on today: what is being entered usually continues from there.
        let day = match &editing {
            Some(entry) => entry.date,
            None => book::latest_date(&entries).unwrap_or_else(crate::today),
        };
        let date = cx.new(|cx| DateField::new(day, window, cx));
        let comment = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder("Optional");
            match editing.as_ref().and_then(|entry| entry.comment.clone()) {
                Some(comment) => state.default_value(comment),
                None => state,
            }
        });
        ui::keyboard_for(&comment, Purpose::Message, cx);
        let commenting = editing
            .as_ref()
            .is_some_and(|entry| entry.comment.is_some());

        let mut subscriptions = Vec::new();
        // A sheet has no commitment for Enter to make: Enter in the amount —
        // the keyboard's own key on a phone — saves in the chosen category.
        if surface == Surface::Sheet {
            subscriptions.push(cx.subscribe_in(
                &amount,
                window,
                |this, _, event: &InputEvent, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.commit(window, cx);
                    }
                },
            ));
        }
        EntryForm {
            book,
            mode,
            surface,
            editing,
            categories,
            category,
            amount,
            date,
            comment,
            commenting,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    /// Saves the record and closes the form, or says what is wrong and
    /// leaves it up.
    fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.submit(cx) {
            self.surface.close(window, cx);
        } else {
            self.amount
                .update(cx, |amount, cx| amount.focus(window, cx));
        }
    }

    /// A category pressed: a new record is saved in it, an edited one only
    /// moves to it and waits for Save.
    fn choose(&mut self, id: Vec<u8>, window: &mut Window, cx: &mut Context<Self>) {
        self.category = Some(id);
        if self.editing.is_none() {
            self.commit(window, cx);
        }
        cx.notify();
    }

    fn set_date(&mut self, day: chrono::NaiveDate, window: &mut Window, cx: &mut Context<Self>) {
        self.date
            .update(cx, |date, cx| date.set_date(day, window, cx));
        cx.notify();
    }

    fn open_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commenting = true;
        self.comment
            .update(cx, |comment, cx| comment.focus(window, cx));
        cx.notify();
    }

    /// Reads the form and writes the record. Answers whether it did: `false`
    /// keeps the form up with what is wrong said in it.
    fn submit(&mut self, cx: &mut Context<Self>) -> bool {
        let result = self.read(cx).and_then(|draft| {
            // A record saved as it was is not rewritten: a write is a new
            // stamp, and a stamp that changes nothing would still outrank a
            // change to the same record made on another device.
            let unchanged = self.editing.as_ref().is_some_and(|entry| {
                entry.category_id == draft.category_id
                    && entry.amount == draft.amount
                    && entry.comment == draft.comment
                    && entry.date == draft.date
            });
            if unchanged {
                return Ok(());
            }
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
        let amount = parse_amount(&self.amount.read(cx).value())
            .filter(|amount| *amount > 0.0)
            .ok_or("Amount must be a number greater than 0.")?;
        let category_id = self.category.clone().ok_or("Choose a category.")?;
        let date = self
            .date
            .read(cx)
            .date(cx)
            .ok_or("Date is not a date. Write it like 2026-10-04.")?;
        let comment = self.comment.read(cx).value().trim().to_string();
        Ok(EntryDraft {
            category_id,
            amount,
            comment: (!comment.is_empty()).then_some(comment),
            date,
        })
    }

    /// The categories, each a button with its tile and name, and one more
    /// for making a category.
    fn render_categories(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let tiles = self.categories.iter().map(|category| {
            let look = CategoryLook::of(&category.style, &category.id, &category.name, muted);
            let selected = self.category.as_ref() == Some(&category.id);
            let id = category.id.clone();
            Button::new(SharedString::from(format!(
                "category-{}",
                money_core::store::hex_encode(&category.id)
            )))
            .ghost()
            .selected(selected)
            .w_full()
            .h(rems(5.))
            .px_0p5()
            .accessibility_label(category.name.clone())
            .child(category_cell(
                ui::category_tile(look, TileSize::Card, cx),
                category.name.clone(),
            ))
            .on_click(cx.listener(move |this, _, window, cx| this.choose(id.clone(), window, cx)))
        });
        let (book, mode, surface) = (self.book.clone(), self.mode, self.surface);
        let new_category = Button::new("new-category")
            .ghost()
            .w_full()
            .h(rems(5.))
            .px_0p5()
            .accessibility_label("New category…")
            .child(category_cell(
                div()
                    .size_10()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(cx.theme().radius)
                    .border_1()
                    .border_dashed()
                    .border_color(cx.theme().border)
                    .text_color(muted)
                    .child(gpui_kit::component::Icon::new(Lucide::Plus)),
                "New…".into(),
            ))
            .on_click(move |_, window, cx| {
                surface.close(window, cx);
                crate::categories::open_category_dialog(&book, mode, None, window, cx);
            });
        div()
            .grid()
            .grid_cols(self.surface.columns() as u16)
            .gap_1()
            .children(tiles)
            .child(new_category)
    }

    /// Today and yesterday a press away, any other day typed or picked.
    fn render_date(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let today = crate::today();
        let yesterday = today.pred_opt().unwrap_or(today);
        let chosen = self.date.read(cx).date(cx);
        h_flex()
            .gap_2()
            .child(
                Button::new("today")
                    .label("Today")
                    .outline()
                    .selected(chosen == Some(today))
                    .on_click(
                        cx.listener(move |this, _, window, cx| this.set_date(today, window, cx)),
                    ),
            )
            .child(
                Button::new("yesterday")
                    .label("Yesterday")
                    .outline()
                    .selected(chosen == Some(yesterday))
                    .on_click(
                        cx.listener(move |this, _, window, cx| {
                            this.set_date(yesterday, window, cx)
                        }),
                    ),
            )
            .child(div().flex_1().min_w_0().child(self.date.clone()))
    }
}

/// What a category button holds: its tile over its name.
fn category_cell(tile: impl IntoElement, name: SharedString) -> impl IntoElement {
    v_flex()
        .w_full()
        .min_w_0()
        .items_center()
        .gap_1()
        .child(tile)
        .child(
            div()
                .w_full()
                .text_xs()
                .text_center()
                .whitespace_normal()
                .line_clamp(2)
                .child(name),
        )
}

impl Render for EntryForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let currency = self.book.read(cx).currency().to_string();
        let muted = cx.theme().muted_foreground;
        v_flex()
            .gap_3()
            .child(ui::field(
                "Amount",
                keyboard::on_tap(
                    Input::new(&self.amount)
                        .large()
                        .suffix(div().text_color(muted).child(currency)),
                    &self.amount,
                    cx,
                ),
            ))
            .children(self.error.clone().map(|error| ui::form_error(error, cx)))
            .child(ui::field("Category", self.render_categories(cx)))
            .child(ui::field("Date", self.render_date(cx)))
            .child(if self.commenting {
                ui::field(
                    "Comment",
                    keyboard::on_tap(Input::new(&self.comment), &self.comment, cx),
                )
                .into_any_element()
            } else {
                h_flex()
                    .child(
                        Button::new("add-comment")
                            .label("Add comment")
                            .ghost()
                            .on_click(
                                cx.listener(|this, _, window, cx| this.open_comment(window, cx)),
                            ),
                    )
                    .into_any_element()
            })
            // Deleting is on the object it deletes, apart from the footer's
            // commitment so the two are never pressed for each other.
            .children(self.editing.clone().map(|entry| {
                let (book, mode, surface) = (self.book.clone(), self.mode, self.surface);
                h_flex().pt_2().child(
                    Button::new("delete-record")
                        .label("Delete…")
                        .danger()
                        .outline()
                        .on_click(move |_, window, cx| {
                            surface.close(window, cx);
                            confirm_delete(&book, mode, entry.clone(), window, cx);
                        }),
                )
            }))
    }
}

/// Opens the form for a new record, or for `entry`: a sheet on a phone, a
/// dialog elsewhere.
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

    let title = match &entry {
        Some(_) => format!("Edit {}", mode.noun()),
        None => format!("New {}", mode.noun()),
    };
    let surface = if Layout::of(window).is_phone() {
        Surface::Sheet
    } else {
        Surface::Dialog
    };
    let editing = entry.is_some();
    let form = cx.new(|cx| EntryForm::new(book.clone(), mode, surface, entry, window, cx));
    // Typing lands in the amount. It is focused after the form opens: the
    // dialog or sheet remembers what had the focus before it and gives it
    // back on closing, and that has to be the window behind it.
    let first = form.read(cx).amount.focus_handle(cx);
    match surface {
        Surface::Dialog => {
            let commit = if editing { "Save" } else { "Add" };
            ui::open_form(
                title,
                commit,
                form,
                Some(first),
                EntryForm::submit,
                window,
                cx,
            )
        }
        Surface::Sheet => {
            open_sheet(title.into(), form, editing, window, cx);
            first.focus(window, cx);
        }
    }
}

/// Sheet height, in rems, for what is around the categories — the title,
/// the amount, the date, the comment and the gaps — and for each row of
/// categories; what an edited record adds — its comment out, Delete, and the
/// footer with Save; and the least of the window it leaves above itself.
const SHEET_CHROME: f32 = 18.0;
const SHEET_EDITING: f32 = 9.0;
const SHEET_ROW: f32 = 5.25;
const SHEET_GAP: f32 = 2.0;

/// The form as a sheet at the bottom of a phone's window. It is as tall as
/// the form when there is room and scrolls inside when there is not; with
/// the keyboard up it reaches under the keyboard and keeps the form above
/// it.
fn open_sheet(
    title: SharedString,
    form: Entity<EntryForm>,
    editing: bool,
    window: &mut Window,
    cx: &mut App,
) {
    window.open_sheet_at(Placement::Bottom, cx, move |sheet, window, cx| {
        let rem = window.rem_size();
        let keyboard = crate::shell::keyboard_inset(window);
        let tiles = form.read(cx).categories.len() + 1;
        let rows = tiles.div_ceil(Surface::Sheet.columns()) as f32;
        let extra = if editing { SHEET_EDITING } else { 0.0 };
        let wanted = rem * (SHEET_CHROME + SHEET_ROW * rows + extra);
        let room = window.viewport_size().height
            - keyboard
            - crate::shell::status_bar_inset()
            - rem * SHEET_GAP;
        let height = wanted.min(room).max(Pixels::ZERO) + keyboard;
        let sheet = sheet
            .title(title.clone())
            .size(height)
            .resizable(false)
            .rounded_t(cx.theme().radius_lg)
            .pb(keyboard)
            .child(div().pb_4().child(form.clone()));
        if !editing {
            return sheet;
        }
        let form = form.clone();
        sheet.footer(
            h_flex().w_full().justify_end().child(
                Button::new("save")
                    .label("Save")
                    .primary()
                    .on_click(move |_, window, cx| {
                        form.update(cx, |form, cx| form.commit(window, cx))
                    }),
            ),
        )
    });
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
