//! The record form: making an expense or an income, changing one, and the
//! way to delete one.
//!
//! A new record is an amount and a category: the amount has the focus as the
//! form opens, the category starts on that of the latest record and the
//! categories are a grid of their tiles to change it with, and Add saves
//! (Enter in the amount too, the keyboard's own key on a phone). Its day
//! starts on that of the latest record, today and yesterday a press away and
//! any other typed or picked; the comment stays folded until asked for, so
//! none of these costs a press when it is not wanted. Tags are chips at
//! the categories, any number of them on, and one more makes a tag. It is a
//! [`Form`]: a dialog for a pointer, a sheet over the keyboard for a finger.
//!
//! A record rewritten here is rewritten whole — category, amount, comment,
//! date and account — because that is the only kind of update the ledger's
//! merge has: a newer write replaces the record, it does not patch a field.
//!
//! A transfer leg is not a record of its own. It can be looked at and its
//! transfer deleted, both legs at once; there is nothing of it to edit here.

use gpui_adaptive::prim::{column, key_pressable, row, testable};
use gpui_adaptive::{
    ActiveColors as _, ActiveScale as _, Chips, Command, Confirm, DateField, DateFieldEvent,
    DateFieldState, Env, FieldEvent, Fields, Form, Icon, Labelled, Note, Notice, Purpose, Segments,
    Text, TextField, TextFieldState, TileSize, Tone,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    App, AppContext, Context, Entity, Focusable, SharedString, Subscription, Window, div, rems,
};
use money_core::format::{format_amount, format_money, parse_amount};

use crate::book::{self, Book, CategoryInfo, CategoryLook, Entry, EntryDraft, Filter, Mode};
use crate::ui::{self, Lucide};

struct EntryForm {
    book: Entity<Book>,
    mode: Mode,
    /// The record being changed, or nothing for a new one.
    editing: Option<Entry>,
    /// The categories of `mode`, the most used first.
    categories: Vec<CategoryInfo>,
    /// The chosen category: for a new record, the one Enter saves in.
    category: Option<Vec<u8>>,
    /// The tags of `mode`, by name.
    tags: Vec<CategoryInfo>,
    /// The ones the record is to carry.
    chosen_tags: Vec<Vec<u8>>,
    /// Whether New tag… has opened the form for a tag, which is put on
    /// once it is made.
    making_tag: bool,
    amount: Entity<TextFieldState>,
    date: Entity<DateFieldState>,
    comment: Entity<TextFieldState>,
    /// Whether the comment field is out. A record that has a comment shows
    /// it; a new one keeps it folded until asked for.
    commenting: bool,
    /// Whether New… has opened the form for a category, which is chosen
    /// once it is made.
    making_category: bool,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl EntryForm {
    fn new(
        book: Entity<Book>,
        mode: Mode,
        editing: Option<Entry>,
        preset: Option<&Filter>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let tags = book.read(cx).tags(mode);
        // A new record made on a tag's page carries that tag.
        let chosen_tags = match (&editing, preset) {
            (Some(entry), _) => entry.tags.clone(),
            // Not a hidden one: it is not offered for a new record.
            (None, Some(Filter::Tag(id))) if tags.iter().any(|t| &t.id == id && !t.hidden) => {
                vec![id.clone()]
            }
            (None, _) => Vec::new(),
        };
        let entries = book.read(cx).entries(mode);
        let mut categories = book.read(cx).categories(mode);
        book::order_by_use(&mut categories, |c| c.id.as_slice(), &entries);
        // A new record starts on the category of the latest one, as it does
        // on its day, if that one is still offered; an edited one stays in
        // its own, hidden or removed as it may be, until another is chosen.
        let offered = |id: &Vec<u8>| categories.iter().any(|c| &c.id == id && !c.hidden);
        let category = match (&editing, preset) {
            (Some(entry), _) => Some(entry.category_id.clone()),
            // On a category's page, in that category.
            (None, Some(Filter::Category(id))) => Some(id.clone()).filter(offered),
            (None, _) => book::latest_category(&entries)
                .map(<[u8]>::to_vec)
                .filter(offered),
        }
        .or_else(|| match editing {
            Some(_) => None,
            None => categories.iter().find(|c| !c.hidden).map(|c| c.id.clone()),
        });
        let amount = cx.new(|cx| {
            let mut state = TextFieldState::amount(window, cx);
            state.set_placeholder("0.00", window, cx);
            if let Some(entry) = &editing {
                // Without the thousands spaces, so it parses back as typed.
                state.set_value(format_amount(entry.amount).replace(' ', ""), window, cx);
            }
            state
        });
        // A new record starts on the day of the latest one of its kind, not
        // on today: what is being entered usually continues from there.
        let day = match &editing {
            Some(entry) => entry.date,
            None => book::latest_date(&entries).unwrap_or_else(crate::today),
        };
        let date = cx.new(|cx| DateFieldState::new(Some(day), window, cx));
        let comment = cx.new(|cx| {
            let mut state = TextFieldState::new(Purpose::Message, window, cx);
            state.set_placeholder("Optional", window, cx);
            if let Some(comment) = editing.as_ref().and_then(|entry| entry.comment.clone()) {
                state.set_value(comment, window, cx);
            }
            state
        });
        let commenting = editing
            .as_ref()
            .is_some_and(|entry| entry.comment.is_some());

        let subscriptions = vec![
            // Typing an amount again is fixing what was said wrong with it.
            cx.subscribe(&amount, |this, _, event: &FieldEvent, cx| {
                if matches!(event, FieldEvent::Change) && this.error.take().is_some() {
                    cx.notify();
                }
            }),
            // A day picked or typed whole moves Today and Yesterday with it.
            cx.subscribe(&date, |_, _, _: &DateFieldEvent, cx| cx.notify()),
            // A category made while the form is open is offered at once; one
            // made from its own New… is chosen, which is what it was made for.
            cx.observe(&book, |this: &mut Self, book, cx| {
                let fresh = book.read(cx).categories(this.mode);
                let made = fresh
                    .iter()
                    .find(|category| !this.categories.iter().any(|known| known.id == category.id))
                    .map(|category| category.id.clone());
                // The order the form opened with stays; what is new goes last.
                this.categories
                    .retain(|known| fresh.iter().any(|c| c.id == known.id));
                for category in fresh {
                    match this
                        .categories
                        .iter_mut()
                        .find(|known| known.id == category.id)
                    {
                        Some(known) => *known = category,
                        None => this.categories.push(category),
                    }
                }
                if this.making_category && made.is_some() {
                    this.making_category = false;
                    this.category = made;
                }
                // Tags the same way: a new one is offered, and put on if it
                // was made from here; a deleted one comes off.
                let fresh = book.read(cx).tags(this.mode);
                let made = fresh
                    .iter()
                    .find(|tag| !this.tags.iter().any(|known| known.id == tag.id))
                    .map(|tag| tag.id.clone());
                this.chosen_tags
                    .retain(|id| fresh.iter().any(|tag| &tag.id == id));
                if let Some(made) = made.filter(|_| this.making_tag) {
                    this.making_tag = false;
                    this.chosen_tags.push(made);
                }
                this.tags = fresh;
                cx.notify();
            }),
        ];
        EntryForm {
            book,
            mode,
            editing,
            categories,
            category,
            tags,
            chosen_tags,
            making_tag: false,
            amount,
            date,
            comment,
            commenting,
            making_category: false,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    /// A category pressed: the record is to be in it, once Add or Save says
    /// so.
    fn choose(&mut self, id: Vec<u8>, cx: &mut Context<Self>) {
        self.category = Some(id);
        cx.notify();
    }

    /// A tag pressed: on if it was off, off if it was on.
    fn toggle_tag(&mut self, id: Vec<u8>, cx: &mut Context<Self>) {
        match self.chosen_tags.iter().position(|on| *on == id) {
            Some(at) => {
                self.chosen_tags.remove(at);
            }
            None => self.chosen_tags.push(id),
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
                    && same_tags(&entry.tags, &draft.tags)
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
        let amount = parse_amount(&self.amount.read(cx).value(cx))
            .filter(|amount| *amount > 0.0)
            .ok_or("Amount must be a number greater than 0.")?;
        let category_id = self.category.clone().ok_or("Choose a category.")?;
        let date = self
            .date
            .read(cx)
            .date()
            .ok_or("Date is not a date. Write it like 2026-10-04.")?;
        let comment = self.comment.read(cx).value(cx).trim().to_string();
        Ok(EntryDraft {
            category_id,
            amount,
            comment: (!comment.is_empty()).then_some(comment),
            date,
            // In the order the tags are listed, whatever order they were
            // pressed in.
            tags: self
                .tags
                .iter()
                .filter(|tag| self.chosen_tags.contains(&tag.id))
                .map(|tag| tag.id.clone())
                .collect(),
        })
    }

    /// The categories, each a tile over its name that chooses it, and one
    /// more for making a category.
    fn render_categories(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (colors, scale) = (cx.colors(), cx.scale());
        // A phone's narrow window has room for three to a row; four would
        // break a long name inside a word.
        let columns: u16 = if Env::of(window, cx).is_compact() {
            3
        } else {
            4
        };
        let cell = |id: SharedString, window: &mut Window, cx: &mut App| {
            key_pressable(id, window, cx)
                .flex_col()
                .justify_center()
                .gap_1()
                .h(rems(5.))
                .px_0p5()
                .rounded(scale.radius_container)
        };
        // A hidden category is offered only to the record already in it.
        let tiles: Vec<_> = self
            .categories
            .iter()
            .filter(|category| !category.hidden || self.category.as_ref() == Some(&category.id))
            .map(|category| {
                let look =
                    CategoryLook::of(&category.style, &category.id, &category.name, colors.muted);
                let selected = self.category.as_ref() == Some(&category.id);
                let id = category.id.clone();
                let key = format!("category-{}", money_core::store::hex_encode(&category.id));
                testable(
                    cell(key.into(), window, cx)
                        .when(selected, |cell| {
                            cell.bg(colors.mine).text_color(colors.on_mine)
                        })
                        .child(ui::category_tile(look, TileSize::Medium))
                        .child(Text::small(category.name.clone()).clamp(2).center())
                        .on_click(cx.listener(move |this, _, _, cx| this.choose(id.clone(), cx))),
                )
            })
            .collect();
        let new_category = testable(
            cell("new-category".into(), window, cx)
                .child(
                    div()
                        .size(rems(2.5))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(scale.radius_container)
                        .border_1()
                        .border_dashed()
                        .border_color(colors.border)
                        .text_color(colors.muted)
                        .child(Icon::new(Lucide::Plus)),
                )
                .child(Text::small("New…").muted().center())
                .on_click(cx.listener(|this, _, window, cx| {
                    // The category's form opens over this one, which keeps
                    // what has been written in it.
                    this.making_category = true;
                    crate::categories::open_category_dialog(
                        &this.book, this.mode, None, window, cx,
                    );
                })),
        );
        div()
            .grid()
            .grid_cols(columns)
            .gap_1()
            .children(tiles)
            .child(new_category)
    }

    /// The tags, any number of them on, and the way to make one.
    fn render_tags(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.colors().muted;
        // A hidden tag is offered only to a record that carries it.
        let offered: Vec<_> = self
            .tags
            .iter()
            .filter(|tag| !tag.hidden || self.chosen_tags.contains(&tag.id))
            .collect();
        let any = !offered.is_empty();
        let chips = offered.into_iter().fold(Chips::new("tags"), |chips, tag| {
            let look = CategoryLook::of(&tag.style, &tag.id, &tag.name, muted);
            chips
                .chip(
                    money_core::store::hex_encode(&tag.id),
                    tag.name.clone(),
                    self.chosen_tags.contains(&tag.id),
                )
                .icon(look.icon, look.color)
        });
        let form = cx.entity().downgrade();
        let chips = chips.on_toggle(move |key, _, cx| {
            if let Some(key) = key {
                let id = money_core::store::hex_decode(key);
                form.update(cx, |this, cx| this.toggle_tag(id, cx)).ok();
            }
        });
        let form = cx.entity().downgrade();
        column()
            .gap(cx.scale().space_s)
            .when(any, |tags| tags.child(chips))
            .child(
                row().child(
                    Command::new("new-tag", "New tag…")
                        .icon(Lucide::Plus)
                        .on_trigger(move |window, cx| {
                            form.update(cx, |this, cx| {
                                // The tag's form opens over this one, which
                                // keeps what has been written in it.
                                this.making_tag = true;
                                crate::categories::open_tag_dialog(
                                    &this.book, this.mode, None, window, cx,
                                );
                            })
                            .ok();
                        })
                        .as_button()
                        .ghost(),
                ),
            )
    }

    /// Today and yesterday a press away, any other day typed or picked.
    fn render_date(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let today = crate::today();
        let yesterday = today.pred_opt().unwrap_or(today);
        let chosen = self.date.read(cx).date();
        let on = match chosen {
            Some(day) if day == today => "today",
            Some(day) if day == yesterday => "yesterday",
            _ => "",
        };
        let form = cx.entity().downgrade();
        column()
            .gap(cx.scale().space_s)
            .child(
                Segments::new("day", on)
                    .segment("today", "Today")
                    .segment("yesterday", "Yesterday")
                    .on_change(move |key, window, cx| {
                        let day = if key.as_ref() == "today" {
                            today
                        } else {
                            yesterday
                        };
                        form.update(cx, |this, cx| this.set_date(day, window, cx))
                            .ok();
                    }),
            )
            .child(DateField::new(&self.date))
    }
}

impl Render for EntryForm {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let currency = self.book.read(cx).currency().to_string();
        let form = cx.entity().downgrade();
        Fields::new()
            .child(
                TextField::new(&self.amount)
                    .label("Amount")
                    .suffix(currency),
            )
            .children(
                self.error
                    .clone()
                    .map(|error| Note::new(error).tone(Tone::Danger)),
            )
            .child(Labelled::new(
                "Category",
                self.render_categories(window, cx),
            ))
            .child(Labelled::new("Date", self.render_date(cx)))
            .child(if self.commenting {
                TextField::new(&self.comment)
                    .label("Comment")
                    .into_any_element()
            } else {
                row()
                    .child(
                        Command::new("add-comment", "Add comment")
                            .icon(Lucide::MessageSquare)
                            .on_trigger(move |window, cx| {
                                form.update(cx, |this, cx| this.open_comment(window, cx))
                                    .ok();
                            })
                            .as_button()
                            .ghost(),
                    )
                    .into_any_element()
            })
            .child(Labelled::new("Tags", self.render_tags(cx)))
    }
}

/// Whether two lists of tags are the same tags, in any order.
fn same_tags(a: &[Vec<u8>], b: &[Vec<u8>]) -> bool {
    a.len() == b.len() && a.iter().all(|tag| b.contains(tag))
}

/// Opens the form for a new record, or for `entry`.
pub fn open_entry_dialog(
    book: &Entity<Book>,
    mode: Mode,
    entry: Option<Entry>,
    window: &mut Window,
    cx: &mut App,
) {
    open(book, mode, entry, None, window, cx);
}

/// Opens the form for a new record on the page of `filter`: in its
/// category, or carrying its tag.
pub fn open_new_entry_in(
    book: &Entity<Book>,
    mode: Mode,
    filter: &Filter,
    window: &mut Window,
    cx: &mut App,
) {
    open(book, mode, None, Some(filter), window, cx);
}

fn open(
    book: &Entity<Book>,
    mode: Mode,
    entry: Option<Entry>,
    preset: Option<&Filter>,
    window: &mut Window,
    cx: &mut App,
) {
    if let Some(leg) = entry.as_ref().filter(|entry| entry.is_transfer) {
        return open_transfer_leg(book, mode, leg.clone(), window, cx);
    }
    // A record has to be of some category, so with none the first step is
    // making one — said, rather than left as a form that cannot be saved.
    // Hidden ones are not offered, so they do not count.
    let offered = book
        .read(cx)
        .categories(mode)
        .iter()
        .any(|category| !category.hidden);
    if entry.is_none() && !offered {
        Notice::new(format!(
            "Make a category first: every {} belongs to one.",
            mode.noun()
        ))
        .push(window, cx);
        return crate::categories::open_category_dialog(book, mode, None, window, cx);
    }

    let title = match &entry {
        Some(_) => format!("Edit {}", mode.noun()),
        None => format!("New {}", mode.noun()),
    };
    let commit = if entry.is_some() { "Save" } else { "Add" };
    let delete = entry.clone().map(|entry| {
        let book = book.clone();
        Command::new("delete-record", "Delete…")
            .destructive()
            .on_trigger(move |window, cx| confirm_delete(&book, mode, entry.clone(), window, cx))
    });
    let form = cx.new(|cx| EntryForm::new(book.clone(), mode, entry, preset, window, cx));
    // Typing lands in the amount. It is focused after the form opens: the
    // form remembers what had the focus before it and gives it back on
    // closing, and that has to be the window behind it.
    let first = form.read(cx).amount.focus_handle(cx);
    let saving = form.clone();
    let opened = Form::new(title, form)
        .commit(commit, move |_, cx| {
            saving.update(cx, |form, cx| form.submit(cx))
        })
        .focus(first);
    match delete {
        Some(delete) => opened.also(delete),
        None => opened,
    }
    .open(window, cx);
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
    let delete = {
        let (book, leg) = (book.clone(), leg.clone());
        Command::new("delete-transfer", "Delete transfer…")
            .destructive()
            .on_trigger(move |window, cx| confirm_delete(&book, mode, leg.clone(), window, cx))
    };
    let view = cx.new(|_| TransferLeg {
        mode,
        leg,
        currency,
    });
    Form::new("Transfer", view).also(delete).open(window, cx);
}

/// What a transfer leg is: its amount and day, its comment, and what
/// deleting it does.
struct TransferLeg {
    mode: Mode,
    leg: Entry,
    currency: String,
}

impl Render for TransferLeg {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Fields::new()
            .child(Text::title(format!(
                "{}{} on {}",
                self.mode.sign(),
                format_money(self.leg.amount, &self.currency),
                self.leg.date.format("%Y-%m-%d")
            )))
            .children(
                self.leg
                    .comment
                    .clone()
                    .map(|comment| Text::body(comment).muted()),
            )
            .child(Note::new(
                "This is one side of a transfer between two accounts. \
                 Deleting it removes both sides.",
            ))
    }
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
    Confirm::new("delete", title)
        .body(consequence)
        .confirm(
            Command::new("confirm-delete", "Delete")
                .destructive()
                .on_trigger(move |window, cx| {
                    let deleted = book.update(cx, |book, cx| book.delete_entry(mode, &entry, cx));
                    if let Err(error) = deleted {
                        Notice::error(error).push(window, cx);
                    }
                }),
        )
        .open(window, cx);
}
