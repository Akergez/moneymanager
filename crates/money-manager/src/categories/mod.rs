//! The categories of one kind, each with the colour and icon it is drawn in.
//!
//! A grid of cards where there is room, a list on a phone. A card leads to
//! the category dialog ([`category_dialog`]), where the name, the colour and
//! the icon are chosen.
//!
//! All three are part of the category's record and sync with it. The colour
//! and the icon are optional there: a category from a client that has no
//! notion of either is drawn the default way (see
//! [`crate::book::CategoryLook`]).

mod category_dialog;

use gpui_kit::component::button::Button;
use gpui_kit::component::list::ListItem;
use gpui_kit::component::{ActiveTheme, Icon, Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, Entity, SharedString, Subscription, Window, div};
use money_core::store::hex_encode;

pub use category_dialog::open_category_dialog;

use crate::book::{Book, CategoryInfo, CategoryLook, Mode};
use crate::shell::Layout;
use crate::ui::{self, Lucide, TileSize};

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

    /// One category, as a row that opens it. The same element is a card in
    /// the grid and a row in the list; only what surrounds it differs.
    fn render_category(
        &self,
        category: CategoryInfo,
        layout: Layout,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let look = CategoryLook::of(
            &category.style,
            &category.id,
            &category.name,
            cx.theme().muted_foreground,
        );
        let (book, mode) = (self.book.clone(), self.mode);
        let name = category.name.clone();
        let row = ListItem::new(SharedString::from(format!(
            "category-{}",
            hex_encode(&category.id)
        )))
        .w_full()
        .p_3()
        .rounded(cx.theme().radius_lg)
        .child(
            h_flex()
                .w_full()
                .gap_3()
                .child(ui::category_tile(look, TileSize::Card, cx))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_medium()
                        .child(name),
                )
                // A lane of its own, so the names stay on one spine.
                .child(
                    Icon::new(Lucide::ChevronRight)
                        .small()
                        .text_color(cx.theme().muted_foreground),
                ),
        )
        .on_click(move |_, window, cx| {
            open_category_dialog(&book, mode, Some(category.clone()), window, cx)
        });
        if layout.is_phone() {
            row.into_any_element()
        } else {
            div()
                .border_1()
                .border_color(cx.theme().border)
                .rounded(cx.theme().radius_lg)
                .child(row)
                .into_any_element()
        }
    }
}

impl Render for CategoriesView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(f32::from(window.viewport_size().width));
        let categories = self.book.read(cx).categories(self.mode);
        let kind = self.kind();
        let detail = match categories.len() {
            0 => String::new(),
            1 => format!("1 {kind} category"),
            n => format!("{n} {kind} categories"),
        };

        let body = if categories.is_empty() {
            let (book, mode) = (self.book.clone(), self.mode);
            ui::empty_state(
                Lucide::LayoutGrid,
                format!("No {kind} categories yet"),
                format!("A category groups records, and every {kind} belongs to one."),
                Some(
                    Button::new("first-category")
                        .label("New category…")
                        .on_click(move |_, window, cx| {
                            open_category_dialog(&book, mode, None, window, cx)
                        })
                        .into_any_element(),
                ),
                cx,
            )
        } else {
            let cards: Vec<AnyElement> = categories
                .into_iter()
                .map(|category| self.render_category(category, layout, cx))
                .collect();
            let grid = match layout {
                Layout::Phone => v_flex().w_full().gap_1(),
                Layout::Tablet => div().w_full().grid().grid_cols(2).gap_3(),
                Layout::Desktop => div().w_full().grid().grid_cols(3).gap_3(),
            };
            // The list is what scrolls, not the screen: the heading stays.
            div()
                .id("categories")
                .size_full()
                .overflow_y_scroll()
                .child(grid.children(cards))
                .into_any_element()
        };

        v_flex()
            .size_full()
            .gap_3()
            .when(layout.is_phone(), |screen| screen.px_3().pt_1())
            .when(!layout.is_phone(), |screen| screen.p_4())
            .child(ui::heading("Categories", detail, None, cx))
            .child(div().flex_1().min_h_0().w_full().child(body))
    }
}
