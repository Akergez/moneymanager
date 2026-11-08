//! Deleting a category or a tag: it goes from the interface, and the form
//! asks what becomes of its records — moved into another one, or left
//! without it.
//!
//! Nothing is erased. The ledger keeps the category's record with a state
//! (see `money_core::models::CategoryState`), and the records keep pointing
//! at it: a merged category's records are shown in the one it went into, a
//! removed category's without a name; a merged tag is shown as the tag it
//! went into, a removed one not at all. A deletion is not undone from here.

use gpui_adaptive::{Choice, ChoiceOption, ChoiceState, Fields, Form, Note, Tone};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Entity, SharedString, Window};
use money_core::models::CategoryState;
use money_core::store::{hex_decode, hex_encode};

use super::Kind;
use crate::book::{Book, CategoryInfo, Mode};

/// The choice's key for "no other one".
const NONE: &str = "none";

struct DeleteForm {
    book: Entity<Book>,
    mode: Mode,
    kind: Kind,
    item: CategoryInfo,
    /// How many records are in it, or carry it.
    count: usize,
    into: Entity<ChoiceState>,
    error: Option<SharedString>,
}

impl DeleteForm {
    fn new(
        book: Entity<Book>,
        mode: Mode,
        kind: Kind,
        item: CategoryInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (others, count) = {
            let book = book.read(cx);
            let others = match kind {
                Kind::Category => book.categories(mode),
                Kind::Tag => book.tags(mode),
            };
            let entries = book.entries(mode);
            let count = entries
                .iter()
                .filter(|entry| match kind {
                    Kind::Category => entry.category_id == item.id,
                    Kind::Tag => entry.tags.contains(&item.id),
                })
                .count();
            (others, count)
        };
        let none = match kind {
            Kind::Category => "Leave them uncategorized",
            Kind::Tag => "Take the tag off them",
        };
        let options = std::iter::once(ChoiceOption::new(NONE, none))
            .chain(
                others
                    .into_iter()
                    .filter(|other| other.id != item.id)
                    .map(|other| {
                        let option = ChoiceOption::new(hex_encode(&other.id), other.name);
                        match other.hidden {
                            true => option.detail("Hidden"),
                            false => option,
                        }
                    }),
            )
            .collect();
        let into = cx.new(|cx| ChoiceState::new(options, Some(NONE.into()), window, cx));
        DeleteForm {
            book,
            mode,
            kind,
            item,
            count,
            into,
            error: None,
        }
    }

    /// Deletes it, moving its records where the choice says. Answers whether
    /// it did.
    fn submit(&mut self, cx: &mut Context<Self>) -> bool {
        let state = match self.into.read(cx).selected().map(|key| key.to_string()) {
            Some(key) if key != NONE => CategoryState::Merged(hex_decode(&key)),
            _ => CategoryState::Removed,
        };
        let (mode, kind, id) = (self.mode, self.kind, self.item.id.clone());
        let done = self.book.update(cx, |book, cx| match kind {
            Kind::Category => book.set_category_state(mode, &id, state, cx),
            Kind::Tag => book.set_tag_state(mode, &id, state, cx),
        });
        match done {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
                false
            }
        }
    }

    fn explanation(&self) -> String {
        let records = match self.count {
            1 => "its 1 record".to_string(),
            n => format!("its {n} records"),
        };
        match self.kind {
            Kind::Category => format!(
                "It goes from the lists and the choice for a new record. Move {records} into another category, or leave them uncategorized. This can’t be undone."
            ),
            Kind::Tag => format!(
                "It goes from the lists and the choice for a new record. Put another tag on {records} in its place, or just take it off. This can’t be undone."
            ),
        }
    }
}

impl Render for DeleteForm {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let label = match self.kind {
            Kind::Category => "Move its records to",
            Kind::Tag => "On its records instead",
        };
        Fields::new()
            .child(Note::new(self.explanation()))
            .child(Choice::new(&self.into).id("delete-into").label(label))
            .children(
                self.error
                    .clone()
                    .map(|error| Note::new(error).tone(Tone::Danger)),
            )
    }
}

/// Opens the form that deletes `item`, a category or a tag of `mode`.
pub fn open_delete_dialog(
    book: &Entity<Book>,
    mode: Mode,
    kind: Kind,
    item: CategoryInfo,
    window: &mut Window,
    cx: &mut App,
) {
    let title = format!("Delete the {} “{}”", kind.noun(), item.name);
    let form = cx.new(|cx| DeleteForm::new(book.clone(), mode, kind, item, window, cx));
    let saving = form.clone();
    Form::new(title, form)
        .commit("Delete", move |_, cx| {
            saving.update(cx, |form, cx| form.submit(cx))
        })
        .open(window, cx);
}
