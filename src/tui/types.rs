//! Core type definitions

/// Available tabs in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Accounts,
    ExpenseCategories,
    Expenses,
    TopUpCategories,
    TopUps,
    ExpensePieChart,
    TopUpPieChart,      // Pie charts together
    ExpenseBarChart,
    TopUpBarChart,      // Bar charts together
    ExpenseLineChart,
    Transfers,
}

/// Total number of tabs (must stay in sync with the enum variants).
const TOTAL_TABS: usize = 11;

impl Tab {
    /// Position of this tab in the canonical ordering (Accounts is 0).
    pub fn index(&self) -> usize {
        match self {
            Tab::Accounts => 0,
            Tab::ExpenseCategories => 1,
            Tab::Expenses => 2,
            Tab::TopUpCategories => 3,
            Tab::TopUps => 4,
            Tab::ExpensePieChart => 5,
            Tab::TopUpPieChart => 6,
            Tab::ExpenseBarChart => 7,
            Tab::TopUpBarChart => 8,
            Tab::ExpenseLineChart => 9,
            Tab::Transfers => 10,
        }
    }

    /// Canonical ordered list, so the cycle and the mouse index mapping share one
    /// definition instead of three duplicated `match`es.
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Tab::Accounts,
            1 => Tab::ExpenseCategories,
            2 => Tab::Expenses,
            3 => Tab::TopUpCategories,
            4 => Tab::TopUps,
            5 => Tab::ExpensePieChart,
            6 => Tab::TopUpPieChart,
            7 => Tab::ExpenseBarChart,
            8 => Tab::TopUpBarChart,
            9 => Tab::ExpenseLineChart,
            10 => Tab::Transfers,
            _ => Tab::ExpenseCategories,
        }
    }

    pub fn next(&self) -> Self {
        Self::from_index((self.index() + 1) % TOTAL_TABS)
    }

    pub fn previous(&self) -> Self {
        Self::from_index((self.index() + TOTAL_TABS - 1) % TOTAL_TABS)
    }
}
