//! The categories of one kind, or its tags, each with the colour and icon it
//! is drawn in.
//!
//! A list of rows, each its tile, its name and how many records it has. A
//! row opens the page of those records (see [`crate::workspace`]), where
//! the category or the tag is also edited, in [`category_dialog`]'s form:
//! the name, the colour and the icon.
//!
//! All three are part of the record and sync with it. The colour and the
//! icon are optional there: a category from a client that has no notion of
//! either is drawn the default way (see [`crate::book::CategoryLook`]).
//!
//! A tag is drawn like a category and lives beside it, but a record is in
//! one category and carries any number of tags.
//!
//! A hidden category or tag is listed after the others, under Hidden: it is
//! not offered for a new record, but its records are still in it. Its page
//! shows it again, or deletes it ([`delete_dialog`]).

mod category_dialog;
mod delete_dialog;

use std::rc::Rc;

use gpui_adaptive::{
    ActiveColors as _, Collection, Command, Empty, Icon, ListRow, SectionHeader, Sizable as _,
    Stack, TileSize,
};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Entity, SharedString, Subscription, Window, div};
use money_core::store::hex_encode;

pub use category_dialog::{Kind, open_category_dialog, open_tag_dialog};
pub use delete_dialog::open_delete_dialog;

use crate::book::{Book, CategoryInfo, CategoryLook, Filter, Mode};
use crate::ui::{self, Lucide};

/// Opens the page of the records a filter admits.
pub type OpenRecords = Rc<dyn Fn(Mode, Filter, &mut Window, &mut App)>;

/// Opens the form for a new one of `kind`.
pub fn open_new(book: &Entity<Book>, mode: Mode, kind: Kind, window: &mut Window, cx: &mut App) {
    match kind {
        Kind::Category => open_category_dialog(book, mode, None, window, cx),
        Kind::Tag => open_tag_dialog(book, mode, None, window, cx),
    }
}

pub struct CategoriesView {
    book: Entity<Book>,
    mode: Mode,
    kind: Kind,
    open_records: OpenRecords,
    _subscriptions: Vec<Subscription>,
}

impl CategoriesView {
    pub fn new(
        book: Entity<Book>,
        mode: Mode,
        kind: Kind,
        open_records: OpenRecords,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![cx.observe(&book, |_, _, cx| cx.notify())];
        CategoriesView {
            book,
            mode,
            kind,
            open_records,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.mode = mode;
            cx.notify();
        }
    }

    fn of(&self) -> &'static str {
        match self.mode {
            Mode::Expense => "expense",
            Mode::Income => "income",
        }
    }

    fn items(&self, cx: &App) -> Vec<CategoryInfo> {
        let book = self.book.read(cx);
        match self.kind {
            Kind::Category => book.categories(self.mode),
            Kind::Tag => book.tags(self.mode),
        }
    }

    /// "12 expense categories", "3 income tags": what the page's subtitle
    /// says.
    pub fn detail(&self, cx: &App) -> String {
        let (of, noun) = (self.of(), self.kind.noun());
        match self.items(cx).len() {
            0 => String::new(),
            1 => format!("1 {of} {noun}"),
            n => match self.kind {
                Kind::Category => format!("{n} {of} categories"),
                Kind::Tag => format!("{n} {of} tags"),
            },
        }
    }

    /// One category or tag, as a row that opens its records.
    fn render_item(&self, item: CategoryInfo, count: usize, cx: &App) -> ListRow {
        let muted = cx.colors().muted;
        let look = CategoryLook::of(&item.style, &item.id, &item.name, muted);
        let (open, mode) = (self.open_records.clone(), self.mode);
        let filter = match self.kind {
            Kind::Category => Filter::Category(item.id.clone()),
            Kind::Tag => Filter::Tag(item.id.clone()),
        };
        ListRow::new(
            SharedString::from(format!("{}-{}", self.kind.noun(), hex_encode(&item.id))),
            item.name.clone(),
        )
        .leading(ui::category_tile(look, TileSize::Leading))
        .trailing(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(count.to_string())
                .child(Icon::new(Lucide::ChevronRight).small().text_color(muted)),
        )
        .muted(item.hidden)
        .on_activate(move |window, cx| open(mode, filter.clone(), window, cx))
    }
}

impl Render for CategoriesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let items = self.items(cx);
        let (of, kind) = (self.of(), self.kind);
        if items.is_empty() {
            let (book, mode) = (self.book.clone(), self.mode);
            let (icon, title, body) = match kind {
                Kind::Category => (
                    Lucide::LayoutGrid,
                    format!("No {of} categories yet"),
                    format!("A category groups records, and every {of} belongs to one."),
                ),
                Kind::Tag => (
                    Lucide::Tags,
                    format!("No {of} tags yet"),
                    "A tag marks records across categories — a trip, a project — and a record can carry several."
                        .to_string(),
                ),
            };
            return Empty::new(icon, title)
                .body(body)
                .action(
                    Command::new(
                        SharedString::from(format!("first-{}", kind.noun())),
                        format!("New {}…", kind.noun()),
                    )
                    .on_trigger(move |window, cx| open_new(&book, mode, kind, window, cx)),
                )
                .into_any_element();
        }
        let entries = self.book.read(cx).entries(self.mode);
        let (hidden, shown): (Vec<CategoryInfo>, Vec<CategoryInfo>) =
            items.into_iter().partition(|item| item.hidden);
        let rows = |items: Vec<CategoryInfo>| -> Vec<ListRow> {
            items
                .into_iter()
                .map(|item| {
                    let count = entries
                        .iter()
                        .filter(|entry| match kind {
                            Kind::Category => entry.category_id == item.id,
                            Kind::Tag => entry.tags.contains(&item.id),
                        })
                        .count();
                    self.render_item(item, count, cx)
                })
                .collect()
        };
        let id = match kind {
            Kind::Category => "categories",
            Kind::Tag => "tags",
        };
        let mut collection = Collection::new(id).children(rows(shown));
        if !hidden.is_empty() {
            collection = collection.section(SectionHeader::new("Hidden"), rows(hidden));
        }
        Stack::new().child(collection).into_any_element()
    }
}
