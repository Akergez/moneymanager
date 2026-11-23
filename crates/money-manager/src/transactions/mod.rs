//! The list of records, of one account or of all of them, of one kind at a
//! time.
//!
//! A table where there is room to compare columns, a list of rows on a phone:
//! both are one [`Records`], drawn from the same [`EntryRow`]s, so the two
//! show the same records in ledger order.
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

use std::rc::Rc;

use gpui_adaptive::records::ScrollStrategy;
use gpui_adaptive::{
    ActiveColors as _, Column, ColumnRole, Command, Empty, Env, InputMode, Pill, Record, Records,
    RecordsScroll, WidthClass,
};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, ElementId, Entity, SharedString, Subscription, Window};
use money_core::format::{format_amount, format_money};
use money_core::store::hex_encode;

pub use entry_dialog::{open_entry_dialog, open_new_entry_in};

use crate::book::{Book, CategoryInfo, CategoryLook, CategoryStyle, Entry, Filter, Mode};
use crate::settings::Settings;
use crate::ui::Lucide;

/// The day a new expense or income starts on: the settings choose it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultDate {
    /// The day of the latest record entered, what is usually continued from
    /// — and what the form has always started on.
    Latest,
    /// Today, whatever was entered before.
    Today,
}

impl DefaultDate {
    pub const ALL: [DefaultDate; 2] = [DefaultDate::Latest, DefaultDate::Today];

    pub fn label(self) -> &'static str {
        match self {
            DefaultDate::Latest => "Last record's day",
            DefaultDate::Today => "Today",
        }
    }

    /// What `settings.json` keeps.
    pub fn stored(self) -> &'static str {
        match self {
            DefaultDate::Latest => "latest",
            DefaultDate::Today => "today",
        }
    }

    /// The day chosen in the settings; the latest record's, as the form has
    /// always started, when none is.
    pub fn current(cx: &App) -> Self {
        Self::parse(Settings::global(cx).entry_date().as_deref())
    }

    fn parse(stored: Option<&str>) -> Self {
        match stored {
            Some("today") => DefaultDate::Today,
            _ => DefaultDate::Latest,
        }
    }
}

const ID: &str = "id";
const AMOUNT: &str = "amount";
const ACCOUNT: &str = "account";
const CATEGORY: &str = "category";
const DATE: &str = "date";
const COMMENT: &str = "comment";

/// A record prepared for drawing: everything a row shows, already as text.
#[derive(Debug, Clone)]
pub(crate) struct EntryRow {
    pub entry: Entry,
    /// The name of its account.
    pub account: SharedString,
    /// The currency of its account.
    pub currency: String,
    pub category: SharedString,
    /// The colour and icon of its category, as the ledger has them.
    style: CategoryStyle,
    /// The amount, without a sign: the screen is of one kind of record, and
    /// its heading already says which. Written with its currency when the
    /// rows are not all in one.
    pub amount: SharedString,
    pub date: SharedString,
    pub short_id: SharedString,
    /// Its tags, in the order of their names.
    tags: Vec<CategoryInfo>,
}

impl EntryRow {
    fn new(
        entry: Entry,
        account: SharedString,
        currency: String,
        category: SharedString,
        style: CategoryStyle,
        displayed_in: &str,
    ) -> Self {
        let amount = match currency == displayed_in {
            true => format_amount(entry.amount),
            false => format_money(entry.amount, &currency),
        };
        EntryRow {
            tags: Vec::new(),
            account,
            currency,
            category,
            style,
            amount: amount.into(),
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

pub struct TransactionsView {
    book: Entity<Book>,
    mode: Mode,
    /// What the rows are narrowed to: none on the transactions screen, a
    /// category or a tag on the page of one.
    filter: Option<Filter>,
    rows: Rc<Vec<EntryRow>>,
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

    /// Rebuilds the rows in ledger order, keeping the selection by record ID.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let mode = self.mode;
        let rows: Vec<EntryRow> = {
            let book = self.book.read(cx);
            let tags = book.tags(mode);
            let displayed_in = book.currency().to_string();
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
                    let account = book.account_name(&entry.account_id);
                    let currency = book.account_currency(&entry.account_id).to_string();
                    let on: Vec<CategoryInfo> = entry
                        .tags
                        .iter()
                        .filter_map(|id| tags.iter().find(|tag| &tag.id == id).cloned())
                        .collect();
                    let mut row =
                        EntryRow::new(entry, account, currency, category, style, &displayed_in);
                    row.tags = on;
                    row
                })
                .collect()
        };
        if let Some(selected) = &self.selected
            && !rows.iter().any(|row| &row.entry.id == selected)
        {
            self.selected = None;
        }
        self.rows = Rc::new(rows);
        cx.notify();
    }

    /// "12 records · 1 234.50 ₽": what the page's subtitle says. With
    /// records in several currencies, each currency's total of its own.
    pub fn detail(&self, _: &App) -> String {
        if self.rows.is_empty() {
            return String::new();
        }
        let mut totals: std::collections::BTreeMap<&str, f64> =
            std::collections::BTreeMap::new();
        for row in self.rows.iter() {
            *totals.entry(row.currency.as_str()).or_default() += row.entry.amount;
        }
        let money = totals
            .into_iter()
            .map(|(currency, total)| format_money(total, currency))
            .collect::<Vec<_>>()
            .join(" · ");
        match self.rows.len() {
            1 => format!("1 record · {money}"),
            n => format!("{n} records · {money}"),
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
            .body("Records on these accounts are listed here as they are made.")
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
            "Every {noun} you record is listed here, whichever account it is on."
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
        let env = Env::of(window, cx);
        // The comment comes and goes with the room for it.
        let with_comments = env.width() == WidthClass::Expanded;
        // On a pointer with room for columns, each account has one; on a
        // phone the account goes under the date.
        let table = env.input() == InputMode::Pointer && env.width() != WidthClass::Compact;
        let all_accounts = self.book.read(cx).shows_all_accounts();
        let account_column = all_accounts && table;
        let records: Vec<Record> = self
            .rows
            .iter()
            .map(|row| {
                let look = row.look(cx);
                // On a phone the account goes under the date; with columns
                // it has one of its own.
                let date = match all_accounts && !table {
                    true => SharedString::from(format!("{} · {}", row.date, row.account)),
                    false => row.date.clone(),
                };
                let mut cells = vec![
                    row.short_id.clone(),
                    row.amount.clone(),
                    row.category.clone(),
                    date,
                ];
                if account_column {
                    cells.push(row.account.clone());
                }
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
                    .role(ColumnRole::Trailing),
            )
            .column(
                Column::new(CATEGORY, "Category")
                    .grow(7.)
                    .role(ColumnRole::Title),
            )
            .column(
                Column::new(DATE, "Date")
                    .grow(4.)
                    .role(ColumnRole::Subtitle),
            );
        if account_column {
            table = table.column(Column::new(ACCOUNT, "Account").grow(5.).muted());
        }
        if with_comments {
            table = table.column(Column::new(COMMENT, "Comment").grow(6.).muted());
        }
        let rows = self.rows.clone();
        let view = cx.entity().downgrade();
        table
            .shared(Rc::new(records))
            .selected(selected)
            .fill(&self.scroll)
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

    #[test]
    fn a_default_date_survives_being_stored() {
        for day in DefaultDate::ALL {
            assert_eq!(DefaultDate::parse(Some(day.stored())), day);
        }
        // The form's own habit when nothing is chosen, and for a value a
        // later version may write.
        assert_eq!(DefaultDate::parse(None), DefaultDate::Latest);
        assert_eq!(DefaultDate::parse(Some("yesterday")), DefaultDate::Latest);
    }
}
