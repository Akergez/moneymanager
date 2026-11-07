//! The list of an account's records, of one kind at a time.
//!
//! A table where there is room to compare columns, a list of rows on a phone:
//! both are one [`Records`], drawn from the same [`EntryRow`]s, so the two
//! cannot show different records. The table's headers sort it.
//!
//! A record's tags are small pills on its row, after the category in a
//! table and on the second line of a list. The same view, given a
//! [`Filter`], is the page of one category's or one tag's records.
//!
//! A row leads to one place, the record form ([`entry_dialog`]), which is
//! also where a record is deleted from — so nothing here depends on hovering
//! over a row to find out what can be done with it. The keyboard has the
//! same way in: Up and Down move the selection, Enter opens it.

mod entry_dialog;

use std::cmp::Ordering;
use std::rc::Rc;

use gpui_adaptive::records::ScrollStrategy;
use gpui_adaptive::{
    ActiveColors as _, Column, ColumnRole, Command, Empty, Env, Pill, Record, Records,
    RecordsScroll, WidthClass,
};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, ElementId, Entity, SharedString, Subscription, Window};
use money_core::format::{format_amount, format_money};
use money_core::store::hex_encode;

pub use entry_dialog::{open_entry_dialog, open_new_entry_in};

use crate::book::{Book, CategoryInfo, CategoryLook, CategoryStyle, Entry, Filter, Mode};
use crate::ui::Lucide;

const ID: &str = "id";
const AMOUNT: &str = "amount";
const CATEGORY: &str = "category";
const DATE: &str = "date";
const COMMENT: &str = "comment";

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
    /// Its tags, in the order of their names.
    tags: Vec<CategoryInfo>,
}

impl EntryRow {
    fn new(entry: Entry, category: SharedString, style: CategoryStyle) -> Self {
        EntryRow {
            tags: Vec::new(),
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
            cx.colors().muted,
        )
    }

    /// Its tags as pills, in the theme of this frame.
    fn pills(&self, cx: &App) -> Vec<Pill> {
        let muted = cx.colors().muted;
        self.tags
            .iter()
            .map(|tag| {
                let look = CategoryLook::of(&tag.style, &tag.id, &tag.name, muted);
                Pill::new(tag.name.clone(), look.color).icon(look.icon)
            })
            .collect()
    }

    /// The id its row goes by, from the record's own.
    fn element_id(&self) -> ElementId {
        SharedString::from(format!("entry-{}", hex_encode(&self.entry.id))).into()
    }
}

/// Which way a column sorts the records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    Ascending,
    Descending,
}

/// What a press on a column's header does to the order: the column's own
/// first direction (the newest dates, the smallest amounts), then the other,
/// then back to the ledger's order.
fn next_sort(
    current: Option<(&'static str, Direction)>,
    pressed: &str,
) -> Option<(&'static str, Direction)> {
    let key = [AMOUNT, CATEGORY, DATE]
        .into_iter()
        .find(|key| *key == pressed)?;
    let first = if key == DATE {
        Direction::Descending
    } else {
        Direction::Ascending
    };
    match current {
        Some((sorted, direction)) if sorted == key && direction == first => Some((
            key,
            match first {
                Direction::Ascending => Direction::Descending,
                Direction::Descending => Direction::Ascending,
            },
        )),
        Some((sorted, _)) if sorted == key => None,
        _ => Some((key, first)),
    }
}

/// Puts `rows` in the order of `sort`; ties, and no column at all, are the
/// ledger's own order: the newest first.
fn sort_rows(rows: &mut [EntryRow], sort: Option<(&'static str, Direction)>) {
    let newest_first = |a: &EntryRow, b: &EntryRow| {
        b.entry
            .date
            .cmp(&a.entry.date)
            .then(b.entry.id.cmp(&a.entry.id))
    };
    let Some((key, direction)) = sort else {
        rows.sort_by(newest_first);
        return;
    };
    let by_column = |a: &EntryRow, b: &EntryRow| match key {
        AMOUNT => a
            .entry
            .amount
            .partial_cmp(&b.entry.amount)
            .unwrap_or(Ordering::Equal),
        CATEGORY => a.category.cmp(&b.category),
        DATE => a.entry.date.cmp(&b.entry.date),
        _ => Ordering::Equal,
    };
    match direction {
        Direction::Ascending => {
            rows.sort_by(|a, b| by_column(a, b).then_with(|| newest_first(a, b)))
        }
        Direction::Descending => {
            rows.sort_by(|a, b| by_column(b, a).then_with(|| newest_first(a, b)))
        }
    }
}

pub struct TransactionsView {
    book: Entity<Book>,
    mode: Mode,
    /// What the rows are narrowed to: none on the transactions screen, a
    /// category or a tag on the page of one.
    filter: Option<Filter>,
    rows: Rc<Vec<EntryRow>>,
    /// How the rows are ordered, so that new rows come in the same order
    /// instead of resetting what the person chose.
    sort: Option<(&'static str, Direction)>,
    /// The record the keyboard is on.
    selected: Option<Vec<u8>>,
    scroll: RecordsScroll,
    _subscriptions: Vec<Subscription>,
}

impl TransactionsView {
    pub fn new(
        book: Entity<Book>,
        mode: Mode,
        filter: Option<Filter>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![cx.observe(&book, |this, _, cx| this.refresh(cx))];
        let mut view = TransactionsView {
            book,
            mode,
            filter,
            rows: Rc::new(Vec::new()),
            sort: None,
            selected: None,
            scroll: RecordsScroll::new(),
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

    /// Rebuilds the rows from the ledger, in the order the table was left
    /// in. The selection stays on its record while that is still there.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let mode = self.mode;
        let mut rows: Vec<EntryRow> = {
            let book = self.book.read(cx);
            let tags = book.tags(mode);
            book.entries(mode)
                .into_iter()
                .filter(|entry| {
                    self.filter
                        .as_ref()
                        .is_none_or(|filter| filter.admits(entry))
                })
                .map(|entry| {
                    let category = book.category_name(mode, &entry.category_id);
                    let style = book.category_style(mode, &entry.category_id);
                    let on: Vec<CategoryInfo> = entry
                        .tags
                        .iter()
                        .filter_map(|id| tags.iter().find(|tag| &tag.id == id).cloned())
                        .collect();
                    let mut row = EntryRow::new(entry, category, style);
                    row.tags = on;
                    row
                })
                .collect()
        };
        sort_rows(&mut rows, self.sort);
        if let Some(selected) = &self.selected
            && !rows.iter().any(|row| &row.entry.id == selected)
        {
            self.selected = None;
        }
        self.rows = Rc::new(rows);
        cx.notify();
    }

    fn sort_by(&mut self, column: &str, cx: &mut Context<Self>) {
        self.sort = next_sort(self.sort, column);
        let mut rows = (*self.rows).clone();
        sort_rows(&mut rows, self.sort);
        self.rows = Rc::new(rows);
        cx.notify();
    }

    /// "12 records · 1 234.50 ₽": what the page's subtitle says.
    pub fn detail(&self, cx: &App) -> String {
        let book = self.book.read(cx);
        let total: f64 = self.rows.iter().map(|row| row.entry.amount).sum();
        match self.rows.len() {
            0 => String::new(),
            1 => format!("1 record · {}", format_money(total, book.currency())),
            n => format!("{n} records · {}", format_money(total, book.currency())),
        }
    }

    fn open(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        self.selected = Some(entry.id.clone());
        cx.notify();
        open_entry_dialog(&self.book, self.mode, Some(entry), window, cx);
    }

    /// Whether a record is selected, which is what Open is about.
    pub fn has_selection(&self, _: &App) -> bool {
        self.selected.is_some()
    }

    /// Opens the selected record. The command itself lives with the
    /// workspace's other commands.
    pub fn open_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entry = self
            .rows
            .iter()
            .find(|row| Some(&row.entry.id) == self.selected.as_ref())
            .map(|row| row.entry.clone());
        if let Some(entry) = entry {
            self.open(entry, window, cx);
        }
    }

    /// Moves the selection one record on or back, from the first record
    /// when there is none, and keeps it in view.
    pub fn step_selection(&mut self, forward: bool, cx: &mut Context<Self>) {
        if self.rows.is_empty() {
            return;
        }
        let at = self
            .rows
            .iter()
            .position(|row| Some(&row.entry.id) == self.selected.as_ref());
        let last = self.rows.len() - 1;
        let next = match (at, forward) {
            (None, _) => 0,
            (Some(at), true) => (at + 1).min(last),
            (Some(at), false) => at.saturating_sub(1),
        };
        self.selected = Some(self.rows[next].entry.id.clone());
        self.scroll.scroll_to_item(next, ScrollStrategy::Nearest);
        cx.notify();
    }

    pub fn filter(&self) -> Option<&Filter> {
        self.filter.as_ref()
    }

    /// How many records are listed.
    pub fn count(&self) -> usize {
        self.rows.len()
    }

    fn render_empty(&self) -> Empty {
        let noun = self.mode.noun();
        let (book, mode) = (self.book.clone(), self.mode);
        if let Some(filter) = self.filter.clone() {
            let what = match filter {
                Filter::Category(_) => "in this category",
                Filter::Tag(_) => "with this tag",
            };
            return Empty::new(
                Lucide::List,
                format!("No {} {what}", self.mode.title().to_lowercase()),
            )
            .body("Records on this account are listed here as they are made.")
            .action(
                Command::new("first-record", format!("New {noun}…")).on_trigger(
                    move |window, cx| open_new_entry_in(&book, mode, &filter, window, cx),
                ),
            );
        }
        Empty::new(
            Lucide::List,
            format!("No {} yet", self.mode.title().to_lowercase()),
        )
        .body(format!(
            "Every {noun} you record on this account is listed here."
        ))
        .action(
            Command::new("first-record", format!("New {noun}…"))
                .on_trigger(move |window, cx| open_entry_dialog(&book, mode, None, window, cx)),
        )
    }
}

impl Render for TransactionsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.rows.is_empty() {
            return self.render_empty().into_any_element();
        }
        // The comment comes and goes with the room for it.
        let with_comments = Env::of(window, cx).width() == WidthClass::Expanded;
        let records: Vec<Record> = self
            .rows
            .iter()
            .map(|row| {
                let look = row.look(cx);
                let mut cells = vec![
                    row.short_id.clone(),
                    row.amount.clone(),
                    row.category.clone(),
                    row.date.clone(),
                ];
                if with_comments {
                    cells.push(row.entry.comment.clone().unwrap_or_default().into());
                }
                Record::new(row.element_id(), cells)
                    .tile(look.icon, look.color)
                    .pills(row.pills(cx))
            })
            .collect();
        let selected = self
            .rows
            .iter()
            .find(|row| Some(&row.entry.id) == self.selected.as_ref())
            .map(EntryRow::element_id);
        // The columns in the order the brief names them, sharing the width
        // by what each is read for: the category most, the comment, rarely
        // filled in, no more than the others.
        let mut table = Records::new("entries")
            .column(Column::new(ID, "ID").grow(3.).muted())
            .column(
                Column::new(AMOUNT, "Amount")
                    .grow(4.)
                    .numeric()
                    .sortable()
                    .role(ColumnRole::Trailing),
            )
            .column(
                Column::new(CATEGORY, "Category")
                    .grow(7.)
                    .sortable()
                    .role(ColumnRole::Title),
            )
            .column(
                Column::new(DATE, "Date")
                    .grow(4.)
                    .sortable()
                    .role(ColumnRole::Subtitle),
            );
        if with_comments {
            table = table.column(Column::new(COMMENT, "Comment").grow(6.).muted());
        }
        if let Some((key, direction)) = self.sort {
            table = table.sorted_by(key, direction == Direction::Ascending);
        }
        let rows = self.rows.clone();
        let view = cx.entity().downgrade();
        let sorter = view.clone();
        table
            .shared(Rc::new(records))
            .selected(selected)
            .fill(&self.scroll)
            .on_sort(move |column, _, cx| {
                sorter.update(cx, |this, cx| this.sort_by(column, cx)).ok();
            })
            .on_activate(move |id, window, cx| {
                let Some(row) = rows.iter().find(|row| &row.element_id() == id) else {
                    return;
                };
                let entry = row.entry.clone();
                view.update(cx, |this, cx| this.open(entry, window, cx))
                    .ok();
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn row(id: u8, amount: f64, category: &str, day: u32) -> EntryRow {
        EntryRow::new(
            Entry {
                id: vec![id],
                category_id: vec![1],
                amount,
                comment: None,
                date: NaiveDate::from_ymd_opt(2026, 10, day).unwrap(),
                is_transfer: false,
                tags: Vec::new(),
            },
            category.to_string().into(),
            Default::default(),
        )
    }

    fn ids(rows: &[EntryRow]) -> Vec<u8> {
        rows.iter().map(|row| row.entry.id[0]).collect()
    }

    #[test]
    fn rows_sort_by_a_column_and_fall_back_to_newest_first() {
        let mut rows = vec![
            row(1, 50.0, "Food", 3),
            row(2, 10.0, "Transport", 5),
            row(3, 50.0, "Food", 4),
        ];
        sort_rows(&mut rows, Some((AMOUNT, Direction::Ascending)));
        // The two fifties tie, and the newer one comes first.
        assert_eq!(ids(&rows), [2, 3, 1]);
        sort_rows(&mut rows, Some((AMOUNT, Direction::Descending)));
        assert_eq!(ids(&rows), [3, 1, 2]);
        sort_rows(&mut rows, None);
        assert_eq!(ids(&rows), [2, 3, 1]);
    }

    #[test]
    fn a_header_sorts_its_own_way_first_then_the_other_then_not_at_all() {
        let amount = next_sort(None, AMOUNT);
        assert_eq!(amount, Some((AMOUNT, Direction::Ascending)));
        let amount = next_sort(amount, AMOUNT);
        assert_eq!(amount, Some((AMOUNT, Direction::Descending)));
        assert_eq!(next_sort(amount, AMOUNT), None);
        // The newest dates first is what a date is sorted for.
        assert_eq!(next_sort(amount, DATE), Some((DATE, Direction::Descending)));
        // An id is not a column to sort by.
        assert_eq!(next_sort(None, ID), None);
    }
}
