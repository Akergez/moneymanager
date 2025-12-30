//! Expense creation form

use chrono::NaiveDate;
use diesel::SqliteConnection;
use crate::models::{Category, Expense};
use super::form_trait::{Form, FormResult};

/// Fields in the expense form
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExpenseField {
    Category,
    Amount,
    Date,
    Comment,
}

impl ExpenseField {
    pub fn next(&self) -> Self {
        match self {
            Self::Category => Self::Amount,
            Self::Amount => Self::Date,
            Self::Date => Self::Comment,
            Self::Comment => Self::Category,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Category => Self::Comment,
            Self::Amount => Self::Category,
            Self::Date => Self::Amount,
            Self::Comment => Self::Date,
        }
    }
}

/// Form for creating a new expense
#[derive(Debug, Clone)]
pub struct ExpenseForm {
    pub current_field: ExpenseField,
    pub category_index: usize,
    pub amount: String,
    pub date: String,
    pub comment: String,
    pub error_message: Option<String>,
    
    /// Reference to available categories (stored as slice of IDs and names)
    categories: Vec<(Vec<u8>, String)>,
}

impl ExpenseForm {
    pub fn new() -> Self {
        Self {
            current_field: ExpenseField::Category,
            category_index: 0,
            amount: String::new(),
            date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            comment: String::new(),
            error_message: None,
            categories: Vec::new(),
        }
    }

    /// Initialize form with available categories
    pub fn with_categories(categories: &[Category]) -> Self {
        let mut form = Self::new();
        form.set_categories(categories);
        form
    }

    /// Update available categories
    pub fn set_categories(&mut self, categories: &[Category]) {
        self.categories = categories
            .iter()
            .map(|c| (c.id.clone(), c.name.clone()))
            .collect();
    }

    /// Get currently selected category name
    pub fn selected_category_name(&self) -> Option<&str> {
        self.categories
            .get(self.category_index)
            .map(|(_, name)| name.as_str())
    }

    /// Get list of category names for display
    pub fn category_names(&self) -> Vec<&str> {
        self.categories.iter().map(|(_, name)| name.as_str()).collect()
    }

    /// Navigate to next category
    pub fn category_next(&mut self) {
        if self.category_index + 1 < self.categories.len() {
            self.category_index += 1;
        }
    }

    /// Navigate to previous category
    pub fn category_prev(&mut self) {
        self.category_index = self.category_index.saturating_sub(1);
    }

    /// Check if currently on category field
    pub fn is_on_category_field(&self) -> bool {
        self.current_field == ExpenseField::Category
    }

    fn current_input_mut(&mut self) -> Option<&mut String> {
        match self.current_field {
            ExpenseField::Category => None, // Not text editable
            ExpenseField::Amount => Some(&mut self.amount),
            ExpenseField::Date => Some(&mut self.date),
            ExpenseField::Comment => Some(&mut self.comment),
        }
    }

    fn validate(&self) -> Result<ValidatedExpense, String> {
        if self.categories.is_empty() {
            return Err("No categories available".to_string());
        }

        if self.category_index >= self.categories.len() {
            return Err("No category selected".to_string());
        }

        let amount: f64 = self.amount.trim()
            .parse()
            .map_err(|_| "Invalid amount".to_string())?;

        if amount <= 0.0 {
            return Err("Amount must be greater than 0".to_string());
        }

        let date = NaiveDate::parse_from_str(self.date.trim(), "%Y-%m-%d")
            .map_err(|_| "Invalid date format (use YYYY-MM-DD)".to_string())?;

        let comment = self.comment.trim();
        let comment = if comment.is_empty() { None } else { Some(comment.to_string()) };

        Ok(ValidatedExpense {
            category_id: self.categories[self.category_index].0.clone(),
            amount,
            date,
            comment,
        })
    }
}

impl Default for ExpenseForm {
    fn default() -> Self {
        Self::new()
    }
}

impl Form for ExpenseForm {
    fn clear(&mut self) {
        self.current_field = ExpenseField::Category;
        self.category_index = 0;
        self.amount.clear();
        self.date = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.comment.clear();
        self.error_message = None;
    }

    fn next_field(&mut self) {
        self.current_field = self.current_field.next();
    }

    fn prev_field(&mut self) {
        self.current_field = self.current_field.prev();
    }

    fn push_char(&mut self, c: char) {
        if let Some(input) = self.current_input_mut() {
            input.push(c);
            self.clear_error();
        }
    }

    fn pop_char(&mut self) {
        if let Some(input) = self.current_input_mut() {
            input.pop();
            self.clear_error();
        }
    }

    fn error(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
    }

    fn clear_error(&mut self) {
        self.error_message = None;
    }

    fn submit(&mut self, conn: &mut SqliteConnection) -> Result<FormResult, String> {
        // If on category field, just move to next
        if self.is_on_category_field() {
            self.next_field();
            return Ok(FormResult::FieldChanged);
        }

        let validated = self.validate()?;

        Expense::create(
            conn,
            &validated.category_id,
            validated.amount,
            validated.comment.as_deref(),
            validated.date,
        ).map_err(|e| format!("Database error: {}", e))?;

        self.clear();
        Ok(FormResult::Submitted)
    }
}

/// Validated expense data
struct ValidatedExpense {
    category_id: Vec<u8>,
    amount: f64,
    date: NaiveDate,
    comment: Option<String>,
}

