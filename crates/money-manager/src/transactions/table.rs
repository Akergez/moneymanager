//! The records as a table: what each column is and how it sorts.

use std::cmp::Ordering;

use gpui_kit::component::table::{Column, ColumnSort, TableDelegate, TableState};
use gpui_kit::component::{ActiveTheme, h_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Pixels, Window, div};

use super::EntryRow;
use crate::book::Entry;
use crate::ui::{self, TileSize};

const ID: &str = "id";
const AMOUNT: &str = "amount";
const CATEGORY: &str = "category";
const DATE: &str = "date";
const COMMENT: &str = "comment";

pub(super) struct EntryTable {
    rows: Vec<EntryRow>,
    columns: Vec<Column>,
    /// How the rows are currently ordered, so new rows can be put in the
    /// same order instead of resetting what the person chose.
    sorted_by: Option<(usize, ColumnSort)>,
}

/// How the table's width is shared out, in parts: (id, amount, category,
/// date, comment). The comment is rarely filled in, so it gets no more than
/// the others; the category, which is what a row is read by, gets the most.
const PARTS: [f32; 5] = [3.0, 4.0, 7.0, 4.0, 6.0];

/// What of the width is shared out. The rest is left for the table's own
/// border and scrollbar, so the columns never ask for a horizontal scroll.
const SHARED: f32 = 0.98;

/// The columns, in the order the brief names them: id, amount, category,
/// date — and the comment where there is room. Together they take the whole
/// `width` the table is given, whatever it is: nothing is left empty beside
/// them and nothing is sized apart from the window.
fn columns(with_comments: bool, width: Pixels) -> Vec<Column> {
    let count = if with_comments { 5 } else { 4 };
    let whole: f32 = PARTS[..count].iter().sum();
    let part = |ix: usize| width * (SHARED * PARTS[ix] / whole);
    let mut columns = vec![
        Column::new(ID, "ID").width(part(0)).resizable(false),
        Column::new(AMOUNT, "Amount")
            .width(part(1))
            .text_right()
            .sortable(),
        Column::new(CATEGORY, "Category").width(part(2)).sortable(),
        Column::new(DATE, "Date").width(part(3)).descending(),
    ];
    if with_comments {
        columns.push(Column::new(COMMENT, "Comment").width(part(4)));
    }
    columns
}

impl EntryTable {
    pub fn new(with_comments: bool, width: Pixels) -> Self {
        EntryTable {
            rows: Vec::new(),
            columns: columns(with_comments, width),
            sorted_by: None,
        }
    }

    pub fn set_columns(&mut self, with_comments: bool, width: Pixels) {
        self.columns = columns(with_comments, width);
        // The comment column is the last one, so an index into the others
        // still names the same column.
        if self
            .sorted_by
            .is_some_and(|(col_ix, _)| col_ix >= self.columns.len())
        {
            self.sorted_by = None;
        }
    }

    /// Replaces the rows, which arrive newest first, keeping the sort order
    /// the table was left in.
    pub fn set_rows(&mut self, rows: Vec<EntryRow>) {
        self.rows = rows;
        if let Some((col_ix, sort)) = self.sorted_by {
            self.sort(col_ix, sort);
        }
    }

    pub fn entry(&self, row_ix: usize) -> Option<Entry> {
        self.rows.get(row_ix).map(|row| row.entry.clone())
    }

    fn sort(&mut self, col_ix: usize, sort: ColumnSort) {
        let Some(key) = self.columns.get(col_ix).map(|column| column.key.clone()) else {
            return;
        };
        let newest_first = |a: &EntryRow, b: &EntryRow| {
            b.entry
                .date
                .cmp(&a.entry.date)
                .then(b.entry.id.cmp(&a.entry.id))
        };
        let by_column = |a: &EntryRow, b: &EntryRow| match key.as_ref() {
            AMOUNT => a
                .entry
                .amount
                .partial_cmp(&b.entry.amount)
                .unwrap_or(Ordering::Equal),
            CATEGORY => a.category.cmp(&b.category),
            DATE => a.entry.date.cmp(&b.entry.date),
            _ => Ordering::Equal,
        };
        match sort {
            ColumnSort::Ascending => self
                .rows
                .sort_by(|a, b| by_column(a, b).then_with(|| newest_first(a, b))),
            ColumnSort::Descending => self
                .rows
                .sort_by(|a, b| by_column(b, a).then_with(|| newest_first(a, b))),
            // No column chosen is the order the ledger gives: newest first.
            ColumnSort::Default => self.rows.sort_by(newest_first),
        }
    }
}

impl TableDelegate for EntryTable {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        self.columns[col_ix].clone()
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        self.sorted_by = (sort != ColumnSort::Default).then_some((col_ix, sort));
        self.sort(col_ix, sort);
        cx.notify();
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let (Some(row), Some(column)) = (self.rows.get(row_ix), self.columns.get(col_ix)) else {
            return div().into_any_element();
        };
        let muted = cx.theme().muted_foreground;
        match column.key.as_ref() {
            ID => div()
                .text_color(muted)
                .child(row.short_id.clone())
                .into_any_element(),
            AMOUNT => div()
                .w_full()
                .text_right()
                .child(row.amount.clone())
                .into_any_element(),
            CATEGORY => h_flex()
                .gap_2()
                .min_w_0()
                .child(ui::category_tile(row.look(cx), TileSize::Row, cx))
                .child(div().min_w_0().truncate().child(row.category.clone()))
                .into_any_element(),
            DATE => div().child(row.date.clone()).into_any_element(),
            _ => div()
                .truncate()
                .text_color(muted)
                .child(row.entry.comment.clone().unwrap_or_default())
                .into_any_element(),
        }
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, _: &App) -> String {
        let (Some(row), Some(column)) = (self.rows.get(row_ix), self.columns.get(col_ix)) else {
            return String::new();
        };
        match column.key.as_ref() {
            ID => row.short_id.to_string(),
            AMOUNT => row.amount.to_string(),
            CATEGORY => row.category.to_string(),
            DATE => row.date.to_string(),
            _ => row.entry.comment.clone().unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    /// Some width for a table to be laid out in.
    fn width() -> Pixels {
        Pixels::from(960.0)
    }

    fn row(id: u8, amount: f64, category: &str, day: u32) -> EntryRow {
        EntryRow::new(
            Entry {
                id: vec![id],
                category_id: vec![1],
                amount,
                comment: None,
                date: NaiveDate::from_ymd_opt(2026, 10, day).unwrap(),
                is_transfer: false,
            },
            category.to_string().into(),
            Default::default(),
        )
    }

    fn ids(table: &EntryTable) -> Vec<u8> {
        table.rows.iter().map(|row| row.entry.id[0]).collect()
    }

    #[test]
    fn the_comment_column_is_the_only_one_that_comes_and_goes() {
        let wide = columns(true, width());
        let narrow = columns(false, width());
        assert_eq!(wide.len(), 5);
        assert_eq!(narrow.len(), 4);
        for (a, b) in wide.iter().zip(&narrow) {
            assert_eq!(a.key, b.key);
        }
        assert_eq!(wide[4].key.as_ref(), COMMENT);
    }

    #[test]
    fn the_columns_take_the_width_they_are_given_and_no_more() {
        for with_comments in [true, false] {
            let total: f32 = columns(with_comments, width())
                .iter()
                .map(|column| f32::from(column.width))
                .sum();
            let given = f32::from(width());
            assert!(total <= given, "{total} of {given}");
            assert!(total > given * 0.95, "{total} of {given}");
        }
    }

    #[test]
    fn rows_sort_by_a_column_and_fall_back_to_newest_first() {
        let mut table = EntryTable::new(true, width());
        table.set_rows(vec![
            row(1, 50.0, "Food", 3),
            row(2, 10.0, "Transport", 5),
            row(3, 50.0, "Food", 4),
        ]);
        let amount = 1;
        table.sort(amount, ColumnSort::Ascending);
        // The two fifties tie, and the newer one comes first.
        assert_eq!(ids(&table), [2, 3, 1]);
        table.sort(amount, ColumnSort::Descending);
        assert_eq!(ids(&table), [3, 1, 2]);
        table.sort(amount, ColumnSort::Default);
        assert_eq!(ids(&table), [2, 3, 1]);
    }

    #[test]
    fn new_rows_keep_the_order_the_table_was_left_in() {
        let mut table = EntryTable::new(false, width());
        table.sorted_by = Some((1, ColumnSort::Ascending));
        table.set_rows(vec![row(1, 30.0, "A", 1), row(2, 20.0, "B", 2)]);
        assert_eq!(ids(&table), [2, 1]);
        table.set_rows(vec![
            row(1, 30.0, "A", 1),
            row(2, 20.0, "B", 2),
            row(3, 5.0, "C", 3),
        ]);
        assert_eq!(ids(&table), [3, 2, 1]);
    }
}
