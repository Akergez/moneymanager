//! Core type definitions

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

