//! The list of an account's records, of one kind at a time.
//!
//! A table where there is room to compare columns, a list of rows on a phone.
//! Both draw the same [`EntryRow`]s, so the two layouts cannot show different
//! records; only the table can be re-sorted, because only it has headers to
//! say how it is sorted.
//!
//! A row leads to one place, the record dialog ([`entry_dialog`]), which is
//! also where a record is deleted from — so nothing here depends on hovering
//! over a row to find out what can be done with it.

mod entry_dialog;
mod table;

use std::rc::Rc;

use gpui_kit::component::button::Button;
use gpui_kit::component::list::ListItem;
use gpui_kit::component::table::{DataTable, TableEvent, TableState};
use gpui_kit::component::{ActiveTheme, Sizable, Size, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Bounds, Context, Entity, Pixels, SharedString, Subscription,
    Window, div, uniform_list,
};
use money_core::format::{format_amount, format_money};

pub use entry_dialog::open_entry_dialog;
use table::EntryTable;

use crate::book::{Book, CategoryLook, CategoryStyle, Entry, Mode};
use crate::shell::Layout;
use crate::ui::{self, Lucide, TileSize};

/// A record prepared for drawing: everything a row shows, already as text.
#[derive(Debug, Clone)]
pub(crate) struct EntryRow {
    pub entry: Entry,
    pub category: SharedString,
    /// The colour and icon of its category, as the ledger has them.
    style: CategoryStyle,
    /// The amount, without a sign: the screen is of one kind of record, and
    /// its heading already says which.
    pub amount: SharedString,
    pub date: SharedString,
    pub short_id: SharedString,
}

impl EntryRow {
    fn new(entry: Entry, category: SharedString, style: CategoryStyle) -> Self {
        EntryRow {
            category,
            style,
            amount: format_amount(entry.amount).into(),
            date: entry.date.format("%Y-%m-%d").to_string().into(),
            short_id: entry.short_id().into(),
            entry,
        }
    }

    /// How its category is drawn, in the theme of this frame.
    pub fn look(&self, cx: &App) -> CategoryLook {
        CategoryLook::of(
            &self.style,
            &self.entry.category_id,
            &self.category,
            cx.theme().muted_foreground,
        )
    }
}

pub struct TransactionsView {
    book: Entity<Book>,
    mode: Mode,
    rows: Rc<Vec<EntryRow>>,
    table: Entity<TableState<EntryTable>>,
    /// Whether the table currently carries the comment column, which only
    /// the widest layout has room for.
    with_comments: bool,
    /// The width the table was last drawn in, which its columns share out.
    table_width: Pixels,
    _subscriptions: Vec<Subscription>,
}

impl TransactionsView {
    pub fn new(
        book: Entity<Book>,
        mode: Mode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // A first guess, until the table has been drawn once and measured.
        let table_width = window.viewport_size().width;
        let table = cx.new(|cx| {
            TableState::new(EntryTable::new(true, table_width), window, cx)
                .col_movable(false)
                .col_selectable(false)
        });
        let subscriptions = vec![
            cx.observe(&book, |this, _, cx| this.refresh(cx)),
            cx.subscribe_in(&table, window, |this, _, event: &TableEvent, window, cx| {
                match event {
                    // A second click on a row opens it.
                    TableEvent::DoubleClickedRow(ix) => this.open_row(*ix, window, cx),
                    // The Open command is for the selected row, so it has to
                    // hear that there is one.
                    _ => cx.notify(),
                }
            }),
        ];
        let mut view = TransactionsView {
            book,
            mode,
            rows: Rc::new(Vec::new()),
            table,
            with_comments: true,
            table_width,
            _subscriptions: subscriptions,
        };
        view.refresh(cx);
        view
    }

    /// Shares the table's width out among its columns again, when the width
    /// or the set of columns is no longer what they were laid out for.
    fn fit_columns(&mut self, with_comments: bool, width: Pixels, cx: &mut Context<Self>) {
        if with_comments == self.with_comments && width == self.table_width {
            return;
        }
        self.with_comments = with_comments;
        self.table_width = width;
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_columns(with_comments, width);
            table.refresh(cx);
        });
        cx.notify();
    }

    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.mode = mode;
            self.refresh(cx);
        }
    }

    /// Rebuilds the rows from the ledger. The table keeps its sort order and
    /// loses its selection: a row index means nothing once the rows changed.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let mode = self.mode;
        let rows: Vec<EntryRow> = {
            let book = self.book.read(cx);
            book.entries(mode)
                .into_iter()
                .map(|entry| {
                    let category = book.category_name(mode, &entry.category_id);
                    let style = book.category_style(mode, &entry.category_id);
                    EntryRow::new(entry, category, style)
                })
                .collect()
        };
        self.rows = Rc::new(rows.clone());
        self.table.update(cx, |table, cx| {
            table.delegate_mut().set_rows(rows);
            table.clear_selection(cx);
            table.refresh(cx);
        });
        cx.notify();
    }

    fn open_row(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        // The table may have been re-sorted, so the row is the table's.
        let Some(entry) = self.table.read(cx).delegate().entry(ix) else {
            return;
        };
        open_entry_dialog(&self.book, self.mode, Some(entry), window, cx);
    }

    /// Whether a row of the table is selected, which is what the Edit
    /// command is about.
    pub fn has_selection(&self, cx: &App) -> bool {
        !self.rows.is_empty() && self.table.read(cx).selected_row().is_some()
    }

    /// Opens the selected row's record. The command itself lives with the
    /// workspace's other commands, beside "new".
    pub fn open_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.table.read(cx).selected_row() {
            self.open_row(ix, window, cx);
        }
    }

    fn render_heading(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let book = self.book.read(cx);
        let total: f64 = self.rows.iter().map(|row| row.entry.amount).sum();
        let detail = match self.rows.len() {
            0 => String::new(),
            1 => format!("1 record · {}", format_money(total, book.currency())),
            n => format!("{n} records · {}", format_money(total, book.currency())),
        };
        ui::heading(self.mode.title(), detail, None, cx)
    }

    /// The phone layout: one tall row per record, each a whole tap target.
    fn render_list(&self) -> AnyElement {
        let rows = self.rows.clone();
        let book = self.book.clone();
        let mode = self.mode;
        uniform_list("entries", rows.len(), move |range, _, cx| {
            range
                .map(|ix| {
                    let row = &rows[ix];
                    let entry = row.entry.clone();
                    let book = book.clone();
                    let muted = cx.theme().muted_foreground;
                    ListItem::new(SharedString::from(format!("entry-{}", row.short_id)))
                        // The row's whole width, so the amount stands on the
                        // trailing edge in every row.
                        .w_full()
                        .py_2()
                        .child(
                            h_flex()
                                .w_full()
                                .gap_3()
                                .child(ui::category_tile(row.look(cx), TileSize::Card, cx))
                                .child(
                                    v_flex()
                                        .flex_1()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .truncate()
                                                .font_medium()
                                                .child(row.category.clone()),
                                        )
                                        .child(
                                            div()
                                                .truncate()
                                                .text_xs()
                                                .text_color(muted)
                                                .child(format!("{} · {}", row.short_id, row.date)),
                                        ),
                                )
                                .child(div().flex_none().font_medium().child(row.amount.clone())),
                        )
                        .on_click(move |_, window, cx| {
                            open_entry_dialog(&book, mode, Some(entry.clone()), window, cx)
                        })
                })
                .collect::<Vec<_>>()
        })
        .size_full()
        .into_any_element()
    }

    fn render_empty(&self, cx: &mut Context<Self>) -> AnyElement {
        let noun = self.mode.noun();
        let book = self.book.clone();
        let mode = self.mode;
        ui::empty_state(
            Lucide::List,
            format!("No {} yet", self.mode.title().to_lowercase()),
            format!("Every {noun} you record on this account is listed here."),
            Some(
                Button::new("first-record")
                    .label(format!("New {noun}…"))
                    .on_click(move |_, window, cx| open_entry_dialog(&book, mode, None, window, cx))
                    .into_any_element(),
            ),
            cx,
        )
    }
}

impl Render for TransactionsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::of(window);

        // The comment column comes and goes with the room for it. Changing
        // the columns is a change to the table's state, made when the room
        // changes rather than on every frame.
        let with_comments = layout == Layout::Desktop;
        self.fit_columns(with_comments, self.table_width, cx);
        // The room itself is only known once it has been laid out, so it is
        // read off the frame and used for the next one.
        let view = cx.entity().downgrade();
        let measure = move |bounds: Vec<Bounds<Pixels>>, _: &mut Window, cx: &mut App| {
            let Some(width) = bounds.first().map(|bounds| bounds.size.width) else {
                return;
            };
            let view = view.clone();
            cx.defer(move |cx| {
                view.update(cx, |this, cx| this.fit_columns(with_comments, width, cx))
                    .ok();
            });
        };

        let body = if self.rows.is_empty() {
            self.render_empty(cx)
        } else if layout.is_phone() {
            self.render_list()
        } else {
            DataTable::new(&self.table)
                // The library's rows have two heights of its own, not counted
                // in rems: the taller one wherever a finger may be what
                // presses a row, or the text has been made larger.
                .with_size(
                    if layout == Layout::Tablet
                        || crate::appearance::interface_size(cx)
                            == crate::appearance::InterfaceSize::Large
                    {
                        Size::Large
                    } else {
                        Size::Medium
                    },
                )
                .bordered(true)
                .into_any_element()
        };

        v_flex()
            .size_full()
            .gap_3()
            .when(layout.is_phone(), |screen| screen.px_3().pt_1())
            .when(!layout.is_phone(), |screen| screen.p_4())
            .child(self.render_heading(cx))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .on_children_prepainted(measure)
                    .child(div().size_full().child(body)),
            )
    }
}
