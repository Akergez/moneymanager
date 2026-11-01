//! The categories of one kind, each with the colour and icon it is drawn in.
//!
//! A list of rows, each its tile and name. A row leads to the category form
//! ([`category_dialog`]), where the name, the colour and the icon are
//! chosen.
//!
//! All three are part of the category's record and sync with it. The colour
//! and the icon are optional there: a category from a client that has no
//! notion of either is drawn the default way (see
//! [`crate::book::CategoryLook`]).

mod category_dialog;

use gpui_adaptive::{
    ActiveColors as _, Collection, Command, Empty, Icon, ListRow, Sizable as _, Stack, TileSize,
};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Entity, SharedString, Subscription, Window};
use money_core::store::hex_encode;

pub use category_dialog::open_category_dialog;

use crate::book::{Book, CategoryInfo, CategoryLook, Mode};
use crate::ui::{self, Lucide};

pub struct CategoriesView {
    book: Entity<Book>,
    mode: Mode,
    _subscriptions: Vec<Subscription>,
}

impl CategoriesView {
    pub fn new(book: Entity<Book>, mode: Mode, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.observe(&book, |_, _, cx| cx.notify())];
        CategoriesView {
            book,
            mode,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.mode = mode;
            cx.notify();
        }
    }

    fn kind(&self) -> &'static str {
        match self.mode {
            Mode::Expense => "expense",
            Mode::Income => "income",
        }
    }

    /// "12 expense categories": what the page's subtitle says.
    pub fn detail(&self, cx: &App) -> String {
        let kind = self.kind();
        match self.book.read(cx).categories(self.mode).len() {
            0 => String::new(),
            1 => format!("1 {kind} category"),
            n => format!("{n} {kind} categories"),
        }
    }

    /// One category, as a row that opens it.
    fn render_category(&self, category: CategoryInfo, cx: &App) -> ListRow {
        let muted = cx.colors().muted;
        let look = CategoryLook::of(&category.style, &category.id, &category.name, muted);
        let (book, mode) = (self.book.clone(), self.mode);
        ListRow::new(
            SharedString::from(format!("category-{}", hex_encode(&category.id))),
            category.name.clone(),
        )
        .leading(ui::category_tile(look, TileSize::Leading))
        .trailing(Icon::new(Lucide::ChevronRight).small().text_color(muted))
        .on_activate(move |window, cx| {
            open_category_dialog(&book, mode, Some(category.clone()), window, cx)
        })
    }
}

impl Render for CategoriesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let categories = self.book.read(cx).categories(self.mode);
        if categories.is_empty() {
            let kind = self.kind();
            let (book, mode) = (self.book.clone(), self.mode);
            return Empty::new(Lucide::LayoutGrid, format!("No {kind} categories yet"))
                .body(format!("A category groups records, and every {kind} belongs to one."))
                .action(
                    Command::new("first-category", "New category…").on_trigger(move |window, cx| {
                        open_category_dialog(&book, mode, None, window, cx)
                    }),
                )
                .into_any_element();
        }
        let rows: Vec<ListRow> = categories
            .into_iter()
            .map(|category| self.render_category(category, cx))
            .collect();
        Stack::new()
            .child(Collection::new("categories").children(rows))
            .into_any_element()
    }
}
