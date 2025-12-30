//! Common trait for all forms

use diesel::SqliteConnection;

/// Result of form submission
#[derive(Debug, Clone, PartialEq)]
pub enum FormResult {
    /// Form was submitted successfully
    Submitted,
    /// Moved to next field (not submitted yet)
    FieldChanged,
    /// Nothing happened
    None,
}

/// Common trait for all forms
pub trait Form {
    /// Reset form to initial state
    fn clear(&mut self);
    
    /// Move to next field
    fn next_field(&mut self);
    
    /// Move to previous field  
    fn prev_field(&mut self);
    
    /// Add character to current input
    fn push_char(&mut self, c: char);
    
    /// Remove last character from current input
    fn pop_char(&mut self);
    
    /// Get current error message
    fn error(&self) -> Option<&str>;
    
    /// Set error message
    fn set_error(&mut self, message: String);
    
    /// Clear error message
    fn clear_error(&mut self);
    
    /// Submit the form
    fn submit(&mut self, conn: &mut SqliteConnection) -> Result<FormResult, String>;
}

