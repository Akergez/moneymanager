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
use gpui_kit::component::{ActiveTheme, Disableable, Sizable, Size, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, SharedString, Subscription, Window, div,
    uniform_list,
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
    /// The amount with its sign: `−1 240.00`.
    pub amount: SharedString,
    pub date: SharedString,
    pub short_id: SharedString,
}

impl EntryRow {
    fn new(entry: Entry, category: SharedString, style: CategoryStyle, mode: Mode) -> Self {
        EntryRow {
            category,
            style,
            amount: format!("{}{}", mode.sign(), format_amount(entry.amount)).into(),
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
    _subscriptions: Vec<Subscription>,
}

impl TransactionsView {
    pub fn new(
        book: Entity<Book>,
        mode: Mode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let rem = window.rem_size();
        let table = cx.new(|cx| {
            TableState::new(EntryTable::new(true, rem), window, cx)
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
            _subscriptions: subscriptions,
        };
        view.refresh(cx);
        view
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
                    EntryRow::new(entry, category, style, mode)
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

    fn open_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(ix) = self.table.read(cx).selected_row() {
            self.open_row(ix, window, cx);
        }
    }

    fn render_heading(&self, layout: Layout, cx: &mut Context<Self>) -> impl IntoElement {
        let book = self.book.read(cx);
        let total: f64 = self.rows.iter().map(|row| row.entry.amount).sum();
        let detail = match self.rows.len() {
            0 => String::new(),
            1 => format!("1 record · {}", format_money(total, book.currency())),
            n => format!("{n} records · {}", format_money(total, book.currency())),
        };
        // The table has a selection, so its row has a visible command; on a
        // phone a tap on the row is that command.
        let open = (!layout.is_phone() && !self.rows.is_empty()).then(|| {
            Button::new("open-record")
                .label("Open…")
                .small()
                .disabled(self.table.read(cx).selected_row().is_none())
                .on_click(cx.listener(|this, _, window, cx| this.open_selected(window, cx)))
                .into_any_element()
        });
        ui::heading(self.mode.title(), detail, open, cx)
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
        let layout = Layout::for_width(f32::from(window.viewport_size().width));

        // The comment column comes and goes with the room for it. Changing
        // the columns is a change to the table's state, made once per
        // crossing of the breakpoint rather than on every frame.
        let with_comments = layout == Layout::Desktop;
        if with_comments != self.with_comments {
            self.with_comments = with_comments;
            let rem = window.rem_size();
            self.table.update(cx, |table, cx| {
                table.delegate_mut().set_columns(with_comments, rem);
                table.refresh(cx);
            });
        }

        let body = if self.rows.is_empty() {
            self.render_empty(cx)
        } else if layout.is_phone() {
            self.render_list()
        } else {
            DataTable::new(&self.table)
                .with_size(if layout == Layout::Tablet {
                    Size::Large
                } else {
                    Size::Medium
                })
                .bordered(true)
                .into_any_element()
        };

        v_flex()
            .size_full()
            .gap_3()
            .when(layout.is_phone(), |screen| screen.px_3().pt_1())
            .when(!layout.is_phone(), |screen| screen.p_4())
            .child(self.render_heading(layout, cx))
            .child(div().flex_1().min_h_0().w_full().child(body))
    }
}
