use diesel::prelude::*;
use crate::models::{Category, Expense, TopUpCategory, TopUp};
use chrono::{Datelike, NaiveDate};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode {
    Normal,
    CreatingCategory,
    CreatingExpense,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputField {
    CategoryName,
    ExpenseCategory,
    ExpenseAmount,
    ExpenseDate,
    ExpenseComment,
}

#[derive(Debug, Clone)]
pub struct FormState {
    pub current_field: InputField,
    pub category_name: String,
    pub expense_category_id: String,
    pub expense_amount: String,
    pub expense_date: String,
    pub expense_comment: String,
    pub error_message: Option<String>,
    pub category_list_selected: usize,
    pub category_list_open: bool,
}

impl FormState {
    pub fn new() -> Self {
        Self {
            current_field: InputField::CategoryName,
            category_name: String::new(),
            expense_category_id: String::new(),
            expense_amount: String::new(),
            expense_date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            expense_comment: String::new(),
            error_message: None,
            category_list_selected: 0,
            category_list_open: false,
        }
    }

    pub fn clear(&mut self) {
        self.category_name.clear();
        self.expense_category_id.clear();
        self.expense_amount.clear();
        self.expense_date = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.expense_comment.clear();
        self.error_message = None;
        self.category_list_selected = 0;
        self.category_list_open = false;
    }

    pub fn next_field(&mut self, mode: InputMode) {
        self.current_field = match mode {
            InputMode::CreatingExpense => match self.current_field {
                InputField::ExpenseCategory => InputField::ExpenseAmount,
                InputField::ExpenseAmount => InputField::ExpenseDate,
                InputField::ExpenseDate => InputField::ExpenseComment,
                InputField::ExpenseComment => InputField::ExpenseCategory,
                _ => InputField::ExpenseCategory,
            },
            _ => self.current_field.clone(),
        };
    }

    pub fn prev_field(&mut self, mode: InputMode) {
        self.current_field = match mode {
            InputMode::CreatingExpense => match self.current_field {
                InputField::ExpenseCategory => InputField::ExpenseComment,
                InputField::ExpenseAmount => InputField::ExpenseCategory,
                InputField::ExpenseDate => InputField::ExpenseAmount,
                InputField::ExpenseComment => InputField::ExpenseDate,
                _ => InputField::ExpenseCategory,
            },
            _ => self.current_field.clone(),
        };
    }

    pub fn current_input(&mut self) -> &mut String {
        match self.current_field {
            InputField::CategoryName => &mut self.category_name,
            InputField::ExpenseCategory => &mut self.expense_category_id,
            InputField::ExpenseAmount => &mut self.expense_amount,
            InputField::ExpenseDate => &mut self.expense_date,
            InputField::ExpenseComment => &mut self.expense_comment,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
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

    pub fn title(&self) -> &str {
        match self {
            Tab::ExpenseCategories => "Expense Categories",
            Tab::Expenses => "Expenses",
            Tab::TopUpCategories => "Top-Up Categories",
            Tab::TopUps => "Top-Ups",
            Tab::ExpensePieChart => "Expense Pie Chart",
            Tab::ExpenseBarChart => "Monthly Expenses",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortColumn {
    Id,
    CategoryId,
    Amount,
    Date,
    Comment,
}

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
}

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
    pub selected_expense_index: usize,
    pub selected_top_up_index: usize,
    pub scroll_offset: usize,
    pub input_mode: InputMode,
    pub form_state: FormState,
}

impl App {
    pub fn new(conn: &mut SqliteConnection) -> Result<Self, Box<dyn std::error::Error>> {
        let categories = Category::read_all(conn)?;
        let expenses = Expense::read_all(conn)?;
        let top_up_categories = TopUpCategory::read_all(conn)?;
        let top_ups = TopUp::read_all(conn)?;

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
            selected_expense_index: 0,
            selected_top_up_index: 0,
            scroll_offset: 0,
            input_mode: InputMode::Normal,
            form_state: FormState::new(),
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
        self.reset_scroll();
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = self.current_tab.previous();
        self.reset_scroll();
    }

    pub fn toggle_pie_chart_mode(&mut self) {
        self.pie_chart_mode = self.pie_chart_mode.toggle();
    }

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

    pub fn get_sorted_expenses(&self) -> Vec<Expense> {
        let mut expenses = self.expenses.clone();

        expenses.sort_by(|a, b| {
            let cmp = match self.expense_sort_column {
                SortColumn::Id => a.id.cmp(&b.id),
                SortColumn::CategoryId => a.category_id.cmp(&b.category_id),
                SortColumn::Amount => a.amount.partial_cmp(&b.amount).unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Date => a.date.cmp(&b.date),
                SortColumn::Comment => a.comment.cmp(&b.comment),
            };

            match self.expense_sort_order {
                SortOrder::Ascending => cmp,
                SortOrder::Descending => cmp.reverse(),
            }
        });

        expenses
    }

    pub fn get_sorted_top_ups(&self) -> Vec<TopUp> {
        let mut top_ups = self.top_ups.clone();

        top_ups.sort_by(|a, b| {
            let cmp = match self.top_up_sort_column {
                SortColumn::Id => a.id.cmp(&b.id),
                SortColumn::CategoryId => a.category_id.cmp(&b.category_id),
                SortColumn::Amount => a.amount.partial_cmp(&b.amount).unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Date => a.date.cmp(&b.date),
                SortColumn::Comment => a.comment.cmp(&b.comment),
            };

            match self.top_up_sort_order {
                SortOrder::Ascending => cmp,
                SortOrder::Descending => cmp.reverse(),
            }
        });

        top_ups
    }

    pub fn get_expense_by_category(&self) -> HashMap<Vec<u8>, (String, f64)> {
        let mut category_map: HashMap<Vec<u8>, (String, f64)> = HashMap::new();

        // Initialize with categories
        for category in &self.categories {
            category_map.insert(category.id.clone(), (category.name.clone(), 0.0));
        }

        // Filter expenses based on mode
        let filtered_expenses: Vec<&Expense> = match self.pie_chart_mode {
            PieChartMode::CurrentMonth => {
                let now = chrono::Local::now();
                let current_month = now.month();
                let current_year = now.year();

                self.expenses.iter().filter(|e| {
                    e.date.month() == current_month && e.date.year() == current_year
                }).collect()
            },
            PieChartMode::AllTime => self.expenses.iter().collect(),
        };

        // Sum expenses by category
        for expense in filtered_expenses {
            if let Some(entry) = category_map.get_mut(&expense.category_id) {
                entry.1 += expense.amount;
            }
        }

        category_map
    }

    pub fn get_monthly_expenses(&self) -> Vec<(String, f64)> {
        let mut monthly_map: HashMap<String, f64> = HashMap::new();

        for expense in &self.expenses {
            let month_key = format!("{}-{:02}", expense.date.year(), expense.date.month());
            *monthly_map.entry(month_key).or_insert(0.0) += expense.amount;
        }

        let mut monthly_vec: Vec<(String, f64)> = monthly_map.into_iter().collect();
        monthly_vec.sort_by(|a, b| a.0.cmp(&b.0));

        // Take last 12 months or all available
        let len = monthly_vec.len();
        if len > 12 {
            monthly_vec = monthly_vec.into_iter().skip(len - 12).collect();
        }

        monthly_vec
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    pub fn page_up(&mut self) {
        if self.scroll_offset >= 10 {
            self.scroll_offset -= 10;
        } else {
            self.scroll_offset = 0;
        }
    }

    pub fn page_down(&mut self) {
        self.scroll_offset += 10;
    }

    fn reset_scroll(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn get_category_name(&self, category_id: &[u8]) -> String {
        self.categories
            .iter()
            .find(|c| c.id.as_slice() == category_id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    pub fn get_top_up_category_name(&self, category_id: &[u8]) -> String {
        self.top_up_categories
            .iter()
            .find(|c| c.id.as_slice() == category_id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    pub fn start_creating_category(&mut self) {
        self.input_mode = InputMode::CreatingCategory;
        self.form_state.clear();
        self.form_state.current_field = InputField::CategoryName;
    }

    pub fn start_creating_expense(&mut self) {
        self.input_mode = InputMode::CreatingExpense;
        self.form_state.clear();
        self.form_state.current_field = InputField::ExpenseCategory;
    }

    pub fn cancel_input(&mut self) {
        self.input_mode = InputMode::Normal;
        self.form_state.clear();
    }

    pub fn submit_category(&mut self, conn: &mut SqliteConnection) -> Result<(), String> {
        let name = self.form_state.category_name.trim();
        if name.is_empty() {
            return Err("Category name cannot be empty".to_string());
        }

        Category::create(conn, name)
            .map_err(|e| format!("Database error: {}", e))?;

        self.reload_data(conn)
            .map_err(|e| format!("Failed to reload data: {}", e))?;

        self.input_mode = InputMode::Normal;
        self.form_state.clear();
        Ok(())
    }

    pub fn submit_expense(&mut self, conn: &mut SqliteConnection) -> Result<(), String> {
        // Get category ID from selected index
        if self.categories.is_empty() {
            return Err("No categories available".to_string());
        }

        if self.form_state.category_list_selected >= self.categories.len() {
            return Err("No category selected".to_string());
        }

        let category_id = self.categories[self.form_state.category_list_selected].id.clone();

        // Validate amount
        let amount: f64 = self.form_state.expense_amount.trim()
            .parse()
            .map_err(|_| "Invalid amount".to_string())?;

        if amount <= 0.0 {
            return Err("Amount must be greater than 0".to_string());
        }

        // Validate and parse date
        let date_str = self.form_state.expense_date.trim();
        let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|_| "Invalid date format (use YYYY-MM-DD)".to_string())?;

        let comment = if self.form_state.expense_comment.trim().is_empty() {
            None
        } else {
            Some(self.form_state.expense_comment.trim())
        };

        Expense::create(conn, &category_id, amount, comment, date)
            .map_err(|e| format!("Database error: {}", e))?;

        self.reload_data(conn)
            .map_err(|e| format!("Failed to reload data: {}", e))?;

        self.input_mode = InputMode::Normal;
        self.form_state.clear();
        Ok(())
    }

    pub fn category_list_next(&mut self) {
        if self.form_state.category_list_selected + 1 < self.categories.len() {
            self.form_state.category_list_selected += 1;
        }
    }

    pub fn category_list_previous(&mut self) {
        if self.form_state.category_list_selected > 0 {
            self.form_state.category_list_selected -= 1;
        }
    }

    pub fn toggle_category_list(&mut self) {
        self.form_state.category_list_open = !self.form_state.category_list_open;
    }

    pub fn select_category(&mut self) {
        if self.form_state.category_list_selected < self.categories.len() {
            let category = &self.categories[self.form_state.category_list_selected];
            self.form_state.expense_category_id = category.name.clone();
            self.form_state.category_list_open = false;
        }
    }
}

