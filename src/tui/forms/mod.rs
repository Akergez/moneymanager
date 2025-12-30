//! Form modules for different entity types
pub use form_trait::{Form, FormResult};
pub use expense_form::ExpenseForm;
pub use category_form::CategoryForm;

mod form_trait;
pub mod expense_form;
pub mod category_form;


