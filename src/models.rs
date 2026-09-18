//! Domain records. These are plain data structs; persistence lives in
//! [`crate::store`], backed by an RDX CRDT document instead of a SQL table.
//!
//! `id` / `category_id` remain raw bytes (originally UUID v4) so the rest of the
//! app and the on-disk identity are unchanged; the store renders them as hex
//! strings for the RDX record key.

use chrono::NaiveDate;

#[derive(Clone, Debug)]
pub struct Category {
    pub id: Vec<u8>,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct Expense {
    pub id: Vec<u8>,
    pub category_id: Vec<u8>,
    pub amount: f64,
    pub comment: Option<String>,
    pub date: NaiveDate,
    /// Which account this expense belongs to. Old records without a field get
    /// [`store::DEFAULT_ACCOUNT_ID`].
    pub account_id: Vec<u8>,
    /// `Some` when this expense is the virtual outbound leg of a transfer.
    pub transfer: Option<TransferLeg>,
}

#[derive(Clone, Debug)]
pub struct TopUpCategory {
    pub id: Vec<u8>,
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct TopUp {
    pub id: Vec<u8>,
    pub category_id: Vec<u8>,
    pub amount: f64,
    pub comment: Option<String>,
    pub date: NaiveDate,
    /// Which account this top-up belongs to. Old records without a field get
    /// [`store::DEFAULT_ACCOUNT_ID`].
    pub account_id: Vec<u8>,
    /// `Some` when this top-up is the virtual inbound leg of a transfer.
    pub transfer: Option<TransferLeg>,
}

/// An account in the ledger: a currency and its own opening balance.
#[derive(Clone, Debug)]
pub struct Account {
    pub id: Vec<u8>,
    pub name: String,
    /// ISO-4217 currency code, e.g. "RUB", "USD".
    pub currency: String,
    pub opening_balance: f64,
}

/// A transfer between two accounts, stored as a single record in the transfers
/// collection. Its two "legs" (the expense on the source side, the top-up on
/// the destination side) are not stored; they are derived at read time.
#[derive(Clone, Debug)]
pub struct Transfer {
    pub id: Vec<u8>,
    pub from_account_id: Vec<u8>,
    pub to_account_id: Vec<u8>,
    /// Amount debited, in the source account's currency.
    pub amount_from: f64,
    /// Amount credited, in the destination account's currency.
    pub amount_to: f64,
    pub comment: Option<String>,
    pub date: NaiveDate,
}

impl Transfer {
    /// Implied exchange rate for the transfer: 1 unit of `from` currency = `rate`
    /// units of `to` currency.
    pub fn rate(&self) -> f64 {
        self.amount_to / self.amount_from
    }
}

/// A derived, virtual leg of a transfer shown in the ledger of the account it
/// belongs to. Deleting a leg means deleting the whole transfer.
#[derive(Clone, Debug)]
pub struct TransferLeg {
    pub transfer_id: Vec<u8>,
}
