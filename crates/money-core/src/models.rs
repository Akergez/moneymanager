//! Domain records. These are plain data structs; persistence lives in
//! [`crate::store`], backed by an RDX CRDT document instead of a SQL table.
//!
//! `id` / `category_id` remain raw bytes (originally UUID v4) so the rest of the
//! app and the on-disk identity are unchanged; the store renders them as hex
//! strings for the RDX record key.

use chrono::NaiveDate;

/// How a category is drawn. Both fields are optional and sit after the name
/// in the record, so a record written before they existed — or by a client
/// that knows nothing of them — reads as an unstyled category.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CategoryStyle {
    /// `#rrggbb`, or nothing for a colour the interface picks itself.
    pub color: Option<String>,
    /// The name of a built-in icon, or nothing for the default one.
    pub icon: Option<String>,
}

impl CategoryStyle {
    pub fn is_empty(&self) -> bool {
        self.color.is_none() && self.icon.is_none()
    }
}

/// Whether a category (or a tag) is offered, kept out of the way, or gone
/// from the interface. It is never deleted from the ledger: its records keep
/// pointing at it, and the state says how they are shown.
///
/// Stored after the colour and the icon (see
/// [`crate::store::category_fields`]); a record without it, as every earlier
/// version wrote, is [`CategoryState::Visible`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CategoryState {
    #[default]
    Visible,
    /// Listed, and its records shown in it, but not offered for a new one.
    Hidden,
    /// Gone from the interface; its records are shown in the category with
    /// this id. They are not rewritten: one that arrives later from a device
    /// that did not know of the merge goes there too.
    Merged(Vec<u8>),
    /// Gone from the interface; its records are shown without a category.
    Removed,
}

impl CategoryState {
    /// Still in the interface: offered, or hidden.
    pub fn is_live(&self) -> bool {
        matches!(self, CategoryState::Visible | CategoryState::Hidden)
    }
}

/// Where records of a category are shown, the merges followed.
#[derive(Clone, Debug, Default)]
pub struct Redirects(std::collections::HashMap<Vec<u8>, CategoryState>);

impl Redirects {
    pub fn new(states: impl IntoIterator<Item = (Vec<u8>, CategoryState)>) -> Self {
        Redirects(states.into_iter().collect())
    }

    /// The category a record in `id` is shown in: `id` itself, or where a
    /// merge leads, through a merge of the merged-into one and on. A merge
    /// into a category that is not there, or back into one passed, stops
    /// where it is.
    pub fn resolve(&self, id: &[u8]) -> Vec<u8> {
        let mut at = id.to_vec();
        let mut passed = vec![at.clone()];
        while let Some(CategoryState::Merged(into)) = self.0.get(&at) {
            if passed.contains(into) || !self.0.contains_key(into) {
                break;
            }
            at = into.clone();
            passed.push(at.clone());
        }
        at
    }

    /// Whether a record in `id` is shown in a category the interface has:
    /// where it resolves to is visible or hidden.
    pub fn is_live(&self, id: &[u8]) -> bool {
        self.0
            .get(&self.resolve(id))
            .is_some_and(CategoryState::is_live)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Category {
    pub id: Vec<u8>,
    pub name: String,
    pub style: CategoryStyle,
    pub state: CategoryState,
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

#[derive(Clone, Debug, Default)]
pub struct TopUpCategory {
    pub id: Vec<u8>,
    pub name: String,
    pub style: CategoryStyle,
    pub state: CategoryState,
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
