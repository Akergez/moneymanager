//! Main application state using StatefulWidgets

use crossterm::event::{KeyCode, KeyModifiers};
use diesel::prelude::*;
use crate::models::{Category, Expense, TopUpCategory, TopUp};

use super::types::Tab;
use super::views::{
    ExpensesViewState, TopUpsViewState,
    ExpenseCategoriesViewState, TopUpCategoriesViewState,
    PieChartViewState, BarChartViewState,
    ViewInputResult, ViewState,
};
use super::forms::{CategoryFormState, ExpenseFormState, FormInputResult};

/// Main application state
pub struct AppState {
    pub running: bool,
    pub current_tab: Tab,

    // Data
    pub categories: Vec<Category>,
    pub expenses: Vec<Expense>,
    pub top_up_categories: Vec<TopUpCategory>,
    pub top_ups: Vec<TopUp>,

    // View states
    pub expenses_view: ExpensesViewState,
    pub top_ups_view: TopUpsViewState,
    pub expense_categories_view: ExpenseCategoriesViewState,
    pub top_up_categories_view: TopUpCategoriesViewState,
    pub pie_chart_view: PieChartViewState,
    pub bar_chart_view: BarChartViewState,

    // Form states
    pub category_form: CategoryFormState,
    pub expense_form: ExpenseFormState,
}

impl AppState {
    pub fn new(conn: &mut SqliteConnection) -> Result<Self, Box<dyn std::error::Error>> {
        let categories = Category::read_all(conn)?;
        let expenses = Expense::read_all(conn)?;
        let top_up_categories = TopUpCategory::read_all(conn)?;
        let top_ups = TopUp::read_all(conn)?;

        Ok(Self {
            running: true,
            current_tab: Tab::ExpenseCategories,
            categories,
            expenses,
            top_up_categories,
            top_ups,
            expenses_view: ExpensesViewState::new(),
            top_ups_view: TopUpsViewState::new(),
            expense_categories_view: ExpenseCategoriesViewState::new(),
            top_up_categories_view: TopUpCategoriesViewState::new(),
            pie_chart_view: PieChartViewState::new(),
            bar_chart_view: BarChartViewState::new(),
            category_form: CategoryFormState::new(),
            expense_form: ExpenseFormState::new(),
        })
    }

    pub fn reload_data(&mut self, conn: &mut SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
        self.categories = Category::read_all(conn)?;
        self.expenses = Expense::read_all(conn)?;
        self.top_up_categories = TopUpCategory::read_all(conn)?;
        self.top_ups = TopUp::read_all(conn)?;
        Ok(())
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn next_tab(&mut self) {
        self.current_tab = self.current_tab.next();
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = self.current_tab.previous();
    }

    /// Get mutable reference to the current view's state
    fn current_view_mut(&mut self) -> &mut dyn ViewState {
        match self.current_tab {
            Tab::ExpenseCategories => &mut self.expense_categories_view,
            Tab::Expenses => &mut self.expenses_view,
            Tab::TopUpCategories => &mut self.top_up_categories_view,
            Tab::TopUps => &mut self.top_ups_view,
            Tab::ExpensePieChart => &mut self.pie_chart_view,
            Tab::ExpenseBarChart => &mut self.bar_chart_view,
        }
    }

    /// Handle all input - delegates to forms or views as appropriate
    pub fn handle_input(&mut self, key: KeyCode, modifiers: KeyModifiers, conn: &mut SqliteConnection) {
        // Handle form input first if a form is active
        if self.category_form.is_active {
            let result = self.category_form.handle_input(key, conn);
            if result == FormInputResult::SubmittedNeedsReload {
                let _ = self.reload_data(conn);
            }
            return;
        }

        if self.expense_form.is_active {
            let result = self.expense_form.handle_input(key, conn);
            if result == FormInputResult::SubmittedNeedsReload {
                let _ = self.reload_data(conn);
            }
            return;
        }

        // Delegate to current view
        let view_result = self.current_view_mut().handle_input(key);

        // Handle view results
        match view_result {
            ViewInputResult::OpenCategoryForm => {
                self.category_form.open();
                return;
            }
            ViewInputResult::OpenExpenseForm => {
                self.expense_form.open(&self.categories);
                return;
            }
            ViewInputResult::Consumed => return,
            ViewInputResult::NotConsumed => {}
        }

        // Global input handling
        match key {
            // Quit
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit(),
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => self.quit(),

            // Tab navigation
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.previous_tab(),

            // Direct tab selection
            KeyCode::Char('1') => self.current_tab = Tab::ExpenseCategories,
            KeyCode::Char('2') => self.current_tab = Tab::Expenses,
            KeyCode::Char('3') => self.current_tab = Tab::TopUpCategories,
            KeyCode::Char('4') => self.current_tab = Tab::TopUps,
            KeyCode::Char('5') => self.current_tab = Tab::ExpensePieChart,
            KeyCode::Char('6') => self.current_tab = Tab::ExpenseBarChart,

            // Reload
            KeyCode::Char('r') | KeyCode::Char('R') => { let _ = self.reload_data(conn); }

            _ => {}
        }
    }
}

