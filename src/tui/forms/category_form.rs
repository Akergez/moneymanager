//! Category creation form

use diesel::SqliteConnection;
use crate::models::Category;
use super::form_trait::{Form, FormResult};

/// Form for creating a new category
#[derive(Debug, Clone)]
pub struct CategoryForm {
    pub name: String,
    pub error_message: Option<String>,
}

impl CategoryForm {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            error_message: None,
        }
    }
}

impl Default for CategoryForm {
    fn default() -> Self {
        Self::new()
    }
}

impl Form for CategoryForm {
    fn clear(&mut self) {
        self.name.clear();
        self.error_message = None;
    }

    fn next_field(&mut self) {
        // Single field form - no navigation
    }

    fn prev_field(&mut self) {
        // Single field form - no navigation
    }

    fn push_char(&mut self, c: char) {
        self.name.push(c);
        self.clear_error();
    }

    fn pop_char(&mut self) {
        self.name.pop();
        self.clear_error();
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
        // Validate
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Category name cannot be empty".to_string());
        }

        // Create
        Category::create(conn, name)
            .map_err(|e| format!("Database error: {}", e))?;

        self.clear();
        Ok(FormResult::Submitted)
    }
}

