//! Accounts, and moving money between them.
//!
//! Choosing the account is the workspace's (it is at the top of every
//! screen). What lives here are the page of one account's balance, the two
//! dialogs behind it — the amount it holds now, its name and currency and the
//! transfer — and the list for visibility and startup choice.

mod account_dialog;
mod account_view;
mod accounts_view;
mod balance_dialog;
mod transfer_dialog;

pub use account_dialog::open_account_dialog;
pub use account_view::AccountView;
pub use accounts_view::open_accounts;
pub use balance_dialog::open_balance_dialog;
pub use transfer_dialog::open_transfer_dialog;
