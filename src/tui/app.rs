//! Main application state and logic

use diesel::prelude::*;
use chrono::Datelike;
use std::collections::HashMap;

use crate::models::{Category, Expense, TopUpCategory, TopUp};
pub use super::types::{PieChartMode, SortColumn, SortOrder, Tab};
use super::forms::{CategoryForm, ExpenseForm};

/// Which form is currently active
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveForm {
    None,
    Category,
    Expense,
}

/// Main application state
pub struct App {
    pub running: bool,
    pub current_tab: Tab,
    pub categories: Vec<Category>,
    pub expenses: Vec<Expense>,
    pub top_up_categories: Vec<TopUpCategory>,
    pub top_ups: Vec<TopUp>,
    pub expense_sort_column: SortColumn,
    pub expense_sort_order: SortOrder,
    pub top_up_sort_column: SortColumn,
    pub top_up_sort_order: SortOrder,
    pub pie_chart_mode: PieChartMode,
    pub scroll_offset: usize,
    
    // Forms
    pub active_form: ActiveForm,
    pub category_form: CategoryForm,
    pub expense_form: ExpenseForm,
}

impl App {
    pub fn new(conn: &mut SqliteConnection) -> Result<Self, Box<dyn std::error::Error>> {
        let categories = Category::read_all(conn)?;
        let expenses = Expense::read_all(conn)?;
        let top_up_categories = TopUpCategory::read_all(conn)?;
        let top_ups = TopUp::read_all(conn)?;

        let expense_form = ExpenseForm::with_categories(&categories);

        Ok(App {
            running: true,
            current_tab: Tab::ExpenseCategories,
            categories,
            expenses,
            top_up_categories,
            top_ups,
            expense_sort_column: SortColumn::Date,
            expense_sort_order: SortOrder::Descending,
            top_up_sort_column: SortColumn::Date,
            top_up_sort_order: SortOrder::Descending,
            pie_chart_mode: PieChartMode::CurrentMonth,
            scroll_offset: 0,
            active_form: ActiveForm::None,
            category_form: CategoryForm::new(),
            expense_form,
        })
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Data Management
    // ─────────────────────────────────────────────────────────────────────────────

    pub fn reload_data(&mut self, conn: &mut SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
        self.categories = Category::read_all(conn)?;
        self.expenses = Expense::read_all(conn)?;
        self.top_up_categories = TopUpCategory::read_all(conn)?;
        self.top_ups = TopUp::read_all(conn)?;
        self.expense_form.set_categories(&self.categories);
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Navigation
    // ─────────────────────────────────────────────────────────────────────────────

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn next_tab(&mut self) {
        self.current_tab = self.current_tab.next();
        self.scroll_offset = 0;
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = self.current_tab.previous();
        self.scroll_offset = 0;
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Scrolling
    // ─────────────────────────────────────────────────────────────────────────────

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    pub fn page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(10);
    }

    pub fn page_down(&mut self) {
        self.scroll_offset += 10;
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Sorting
    // ─────────────────────────────────────────────────────────────────────────────

    pub fn toggle_expense_sort(&mut self, column: SortColumn) {
        if self.expense_sort_column == column {
            self.expense_sort_order = self.expense_sort_order.toggle();
        } else {
            self.expense_sort_column = column;
            self.expense_sort_order = SortOrder::Ascending;
        }
    }

    pub fn toggle_top_up_sort(&mut self, column: SortColumn) {
        if self.top_up_sort_column == column {
            self.top_up_sort_order = self.top_up_sort_order.toggle();
        } else {
            self.top_up_sort_column = column;
            self.top_up_sort_order = SortOrder::Ascending;
        }
    }

    pub fn toggle_pie_chart_mode(&mut self) {
        self.pie_chart_mode = self.pie_chart_mode.toggle();
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Data Queries
    // ─────────────────────────────────────────────────────────────────────────────

    pub fn get_sorted_expenses(&self) -> Vec<Expense> {
        let mut expenses = self.expenses.clone();
        let order = self.expense_sort_order;

        expenses.sort_by(|a, b| {
            let cmp = match self.expense_sort_column {
                SortColumn::Id => a.id.cmp(&b.id),
                SortColumn::CategoryId => a.category_id.cmp(&b.category_id),
                SortColumn::Amount => a.amount.partial_cmp(&b.amount).unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Date => a.date.cmp(&b.date),
                SortColumn::Comment => a.comment.cmp(&b.comment),
            };
            order.apply(cmp)
        });

        expenses
    }

    pub fn get_sorted_top_ups(&self) -> Vec<TopUp> {
        let mut top_ups = self.top_ups.clone();
        let order = self.top_up_sort_order;

        top_ups.sort_by(|a, b| {
            let cmp = match self.top_up_sort_column {
                SortColumn::Id => a.id.cmp(&b.id),
                SortColumn::CategoryId => a.category_id.cmp(&b.category_id),
                SortColumn::Amount => a.amount.partial_cmp(&b.amount).unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Date => a.date.cmp(&b.date),
                SortColumn::Comment => a.comment.cmp(&b.comment),
            };
            order.apply(cmp)
        });

        top_ups
    }

    pub fn get_expense_by_category(&self) -> HashMap<Vec<u8>, (String, f64)> {
        let mut category_map: HashMap<Vec<u8>, (String, f64)> = self.categories
            .iter()
            .map(|c| (c.id.clone(), (c.name.clone(), 0.0)))
            .collect();

        let expenses = self.filter_expenses_by_mode();

        for expense in expenses {
            if let Some(entry) = category_map.get_mut(&expense.category_id) {
                entry.1 += expense.amount;
            }
        }

        category_map
    }

    fn filter_expenses_by_mode(&self) -> Vec<&Expense> {
        match self.pie_chart_mode {
            PieChartMode::CurrentMonth => {
                let now = chrono::Local::now();
                let (current_month, current_year) = (now.month(), now.year());

                self.expenses
                    .iter()
                    .filter(|e| e.date.month() == current_month && e.date.year() == current_year)
                    .collect()
            }
            PieChartMode::AllTime => self.expenses.iter().collect(),
        }
    }

    pub fn get_monthly_expenses(&self) -> Vec<(String, f64)> {
        let mut monthly_map: HashMap<String, f64> = HashMap::new();

        for expense in &self.expenses {
            let month_key = format!("{}-{:02}", expense.date.year(), expense.date.month());
            *monthly_map.entry(month_key).or_insert(0.0) += expense.amount;
        }

        let mut monthly_vec: Vec<_> = monthly_map.into_iter().collect();
        monthly_vec.sort_by(|a, b| a.0.cmp(&b.0));

        // Take last 12 months
        if monthly_vec.len() > 12 {
            monthly_vec.drain(..monthly_vec.len() - 12);
        }

        monthly_vec
    }
}

