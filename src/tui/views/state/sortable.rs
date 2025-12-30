//! Sortable view state for tables with sortable columns

use std::cmp::Ordering;

/// Columns that can be sorted
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortColumn {
    Id,
    CategoryId,
    Amount,
    Date,
    Comment,
}

impl SortColumn {
    pub fn next(&self) -> Self {
        match self {
            SortColumn::Id => SortColumn::Comment,
            SortColumn::Comment => SortColumn::CategoryId,
            SortColumn::CategoryId => SortColumn::Amount,
            SortColumn::Amount => SortColumn::Date,
            SortColumn::Date => SortColumn::Id,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            SortColumn::Id => SortColumn::Date,
            SortColumn::Date => SortColumn::Amount,
            SortColumn::Amount => SortColumn::CategoryId,
            SortColumn::CategoryId => SortColumn::Comment,
            SortColumn::Comment => SortColumn::Id,
        }
    }
}

/// Sort order direction
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn toggle(&self) -> Self {
        match self {
            SortOrder::Ascending => SortOrder::Descending,
            SortOrder::Descending => SortOrder::Ascending,
        }
    }

    pub fn apply(&self, cmp: Ordering) -> Ordering {
        match self {
            SortOrder::Ascending => cmp,
            SortOrder::Descending => cmp.reverse(),
        }
    }
}

/// State for a sortable table view
#[derive(Debug, Clone)]
pub struct SortableState {
    pub column: SortColumn,
    pub order: SortOrder,
}

impl SortableState {
    pub fn new() -> Self {
        Self {
            column: SortColumn::Date,
            order: SortOrder::Descending,
        }
    }

    /// Toggle sort on a column - if same column, toggle order; otherwise switch column
    pub fn toggle(&mut self, column: SortColumn) {
        if self.column == column {
            self.order = self.order.toggle();
        } else {
            self.column = column;
            self.order = SortOrder::Ascending;
        }
    }

    /// Cycle to next column
    pub fn next_column(&mut self) {
        self.toggle(self.column.next());
    }

    /// Cycle to previous column
    pub fn prev_column(&mut self) {
        self.toggle(self.column.prev());
    }

    /// Apply sorting to a comparison result
    pub fn apply(&self, cmp: Ordering) -> Ordering {
        self.order.apply(cmp)
    }
}

impl Default for SortableState {
    fn default() -> Self {
        Self::new()
    }
}

