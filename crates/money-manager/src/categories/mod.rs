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

use gpui_adaptive::prim::{key_pressable, testable};
use gpui_adaptive::{
    ActiveColors as _, ActiveScale as _, Collection, Command, Empty, Env, Icon, ListRow,
    SectionHeader, Sizable as _, Stack, Text, TileSize, WidthClass,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, Div, Entity, SharedString, Subscription, Window, div, rems,
};
use money_core::store::hex_encode;

pub use category_dialog::{Kind, open_category_dialog, open_tag_dialog};
pub use delete_dialog::open_delete_dialog;

use crate::book::{Book, CategoryInfo, CategoryLook, Filter, Mode};
use crate::settings::Settings;
use crate::ui::{self, Lucide};

/// How the Categories screen is drawn: the settings choose it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Rows, each its tile, its name and how many records it has.
    List,
    /// A grid of tiles with their names under them, as a record's form
    /// offers the category it is in.
    Icons,
}

impl View {
    pub const ALL: [View; 2] = [View::List, View::Icons];

    pub fn label(self) -> &'static str {
        match self {
            View::List => "List",
            View::Icons => "Icons",
        }
    }

    /// What `settings.json` keeps.
    pub fn stored(self) -> &'static str {
        match self {
            View::List => "list",
            View::Icons => "icons",
        }
    }

    /// The view chosen in the settings; a list, as the screen has always
    /// been, when none is.
    pub fn current(cx: &App) -> Self {
        match Settings::global(cx).category_view().as_deref() {
            Some("icons") => View::Icons,
            _ => View::List,
        }
    }
}

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
        let subscriptions = vec![
            cx.observe(&book, |_, _, cx| cx.notify()),
            cx.observe_global::<Settings>(|_, cx| cx.notify()),
        ];
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

    /// One category as a grid cell: its tile over its name, as the record
    /// form offers it. A press opens its records, like a row.
    fn render_icon(
        &self,
        item: CategoryInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (muted, hidden) = (cx.colors().muted, item.hidden);
        let look = CategoryLook::of(&item.style, &item.id, &item.name, muted);
        let (open, mode) = (self.open_records.clone(), self.mode);
        let filter = match self.kind {
            Kind::Category => Filter::Category(item.id.clone()),
            Kind::Tag => Filter::Tag(item.id.clone()),
        };
        let scale = cx.scale();
        let id = SharedString::from(format!("{}-{}", self.kind.noun(), hex_encode(&item.id)));
        testable(
            key_pressable(id, window, cx)
                .flex_col()
                .justify_center()
                .gap_1()
                .h(rems(5.))
                .px_0p5()
                .rounded(scale.radius_container)
                .when(hidden, |cell| cell.opacity(0.55))
                .child(ui::category_tile(look, TileSize::Medium))
                .child(Text::small(item.name.clone()).clamp(2).center())
                .on_click(
                    cx.listener(move |_, _, window, cx| open(mode, filter.clone(), window, cx)),
                ),
        )
    }

    /// The categories as the record form offers them: a grid of tiles with
    /// their names under them, three to a row on a phone.
    fn render_icon_grid(
        &self,
        items: Vec<CategoryInfo>,
        columns: u16,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        div().grid().grid_cols(columns).gap_1().children(
            items
                .into_iter()
                .map(|item| self.render_icon(item, window, cx)),
        )
    }
}

impl Render for CategoriesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let (hidden, shown): (Vec<CategoryInfo>, Vec<CategoryInfo>) =
            items.into_iter().partition(|item| item.hidden);
        // The Tags screen keeps its rows: only the categories were asked
        // for as icons.
        if kind == Kind::Category && View::current(cx) == View::Icons {
            let columns = match Env::of(window, cx).width() {
                WidthClass::Compact => 3,
                WidthClass::Medium => 4,
                WidthClass::Expanded => 6,
            };
            let scale = cx.scale();
            let mut grid = div()
                .flex()
                .flex_col()
                .w_full()
                .px(scale.space_m)
                .pt(scale.space_s)
                .pb(scale.space_m)
                .child(self.render_icon_grid(shown, columns, window, cx));
            if !hidden.is_empty() {
                grid = grid
                    .child(SectionHeader::new("Hidden"))
                    .child(self.render_icon_grid(hidden, columns, window, cx));
            }
            return grid.into_any_element();
        }
        let entries = self.book.read(cx).entries(self.mode);
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
