//! Accounts, and moving money between them.
//!
//! Choosing the account is the workspace's (it is at the top of every
//! screen). What lives here are the two dialogs behind its menu: an account's
//! own details, and a transfer.

mod account_dialog;
mod transfer_dialog;

pub use account_dialog::open_account_dialog;
pub use transfer_dialog::open_transfer_dialog;
