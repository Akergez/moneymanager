//! Type definitions for the TUI application

use std::cmp::Ordering;

/// Available tabs in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    ExpenseCategories,
    Expenses,
    TopUpCategories,
    TopUps,
    ExpensePieChart,
    ExpenseBarChart,
}

impl Tab {
    pub fn next(&self) -> Self {
        match self {
            Tab::ExpenseCategories => Tab::Expenses,
            Tab::Expenses => Tab::TopUpCategories,
            Tab::TopUpCategories => Tab::TopUps,
            Tab::TopUps => Tab::ExpensePieChart,
            Tab::ExpensePieChart => Tab::ExpenseBarChart,
            Tab::ExpenseBarChart => Tab::ExpenseCategories,
        }
    }

    pub fn previous(&self) -> Self {
        match self {
            Tab::ExpenseCategories => Tab::ExpenseBarChart,
            Tab::Expenses => Tab::ExpenseCategories,
            Tab::TopUpCategories => Tab::Expenses,
            Tab::TopUps => Tab::TopUpCategories,
            Tab::ExpensePieChart => Tab::TopUps,
            Tab::ExpenseBarChart => Tab::ExpensePieChart,
        }
    }
}

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
    /// Returns the next sort column in cycle order (right arrow)
    pub fn next(&self) -> Self {
        match self {
            SortColumn::Id => SortColumn::Comment,
            SortColumn::Comment => SortColumn::CategoryId,
            SortColumn::CategoryId => SortColumn::Amount,
            SortColumn::Amount => SortColumn::Date,
            SortColumn::Date => SortColumn::Id,
        }
    }

    /// Returns the previous sort column in cycle order (left arrow)
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

    /// Apply sort order to an existing comparison result
    pub fn apply(&self, cmp: Ordering) -> Ordering {
        match self {
            SortOrder::Ascending => cmp,
            SortOrder::Descending => cmp.reverse(),
        }
    }
}

/// Pie chart display mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PieChartMode {
    CurrentMonth,
    AllTime,
}

impl PieChartMode {
    pub fn toggle(&self) -> Self {
        match self {
            PieChartMode::CurrentMonth => PieChartMode::AllTime,
            PieChartMode::AllTime => PieChartMode::CurrentMonth,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            PieChartMode::CurrentMonth => "Current Month",
            PieChartMode::AllTime => "All Time",
        }
    }
}

