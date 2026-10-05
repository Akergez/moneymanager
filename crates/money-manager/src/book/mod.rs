//! The open ledger, as the interface sees it.
//!
//! [`Book`] is the one owner of the [`Store`]. Every screen reads from the
//! snapshot it keeps and asks it to make changes; nothing else touches the
//! store, which is what keeps the staging file, the in-memory document and
//! what is on screen from ever disagreeing.
//!
//! A sync is the one slow operation — the S3 client blocks — so it runs on a
//! background thread *with the store moved into it*. For as long as it runs
//! the book has no store, and a write is refused with a reason rather than
//! queued: a write that landed between sealing and the exchange would be
//! staged against a document that is about to be replaced.

mod demo;
mod entry;
mod style;

use std::collections::HashMap;
use std::path::Path;

use chrono::NaiveDate;
use gpui_kit::{AppContext, Context, SharedString};
use money_core::ledger::{self, AccountLedger, AccountSummary};
use money_core::models::{Account, Category, Expense, TopUp, TopUpCategory, Transfer};
use money_core::remote::RemoteConfig;
use money_core::store::{Store, hex_encode};

pub use entry::{Entry, EntryDraft, Mode, TransferDraft, latest_date};
pub use money_core::models::CategoryStyle;
pub use style::{CategoryLook, ICONS, PALETTE, color_of};

use crate::settings::Settings;

/// What the last sync came to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SyncState {
    /// None has been asked for since the ledger was opened.
    #[default]
    Idle,
    Running,
    Done {
        pulled: usize,
        pushed: usize,
    },
    Failed(SharedString),
}

/// A category of either kind, which is all a list or a chart needs of one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryInfo {
    pub id: Vec<u8>,
    pub name: SharedString,
    /// Its colour and icon, as the ledger has them.
    pub style: CategoryStyle,
}

pub struct Book {
    /// Gone while a sync has it.
    store: Option<Store>,
    accounts: Vec<Account>,
    expense_categories: Vec<Category>,
    income_categories: Vec<TopUpCategory>,
    expenses: Vec<Expense>,
    top_ups: Vec<TopUp>,
    transfers: Vec<Transfer>,
    summaries: HashMap<Vec<u8>, AccountSummary>,
    /// The account every list and chart is about.
    account: Vec<u8>,
    /// That account's records, transfer legs included.
    ledger: AccountLedger,
    sync: SyncState,
}

const SYNCING: &str = "A sync is running. Try again when it finishes.";

impl Book {
    /// Opens the ledger in `dir`, creating an empty one if nothing is there.
    /// No account is made: a ledger about to be filled by its first sync must
    /// not get a fresh default account that would outrank the synced one.
    pub fn open(dir: &Path, source: u64, last_account: Option<String>) -> Result<Self, String> {
        let store = Store::open(dir, source)?;
        let mut book = Book {
            store: Some(store),
            accounts: Vec::new(),
            expense_categories: Vec::new(),
            income_categories: Vec::new(),
            expenses: Vec::new(),
            top_ups: Vec::new(),
            transfers: Vec::new(),
            summaries: HashMap::new(),
            account: last_account
                .map(|hex| money_core::store::hex_decode(&hex))
                .unwrap_or_default(),
            ledger: AccountLedger {
                expenses: Vec::new(),
                top_ups: Vec::new(),
            },
            sync: SyncState::Idle,
        };
        book.reload()?;
        Ok(book)
    }

    /// Makes sure the default account exists. Idempotent, and the id is
    /// fixed, so two devices doing this merge to one account.
    pub fn ensure_default_account(&mut self, currency: &str) -> Result<(), String> {
        let store = self.store.as_mut().ok_or(SYNCING)?;
        Account::ensure_default(store, currency)?;
        self.reload()
    }

    /// Reads everything back out of the store. Cheap next to a frame for any
    /// ledger a person keeps by hand, and it is what guarantees the snapshot
    /// is what was actually written.
    fn reload(&mut self) -> Result<(), String> {
        let Some(store) = self.store.as_ref() else {
            return Ok(());
        };
        self.accounts = Account::read_all(store)?;
        self.accounts
            .sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        self.expense_categories = Category::read_all(store)?;
        self.expense_categories.sort_by(|a, b| a.name.cmp(&b.name));
        self.income_categories = TopUpCategory::read_all(store)?;
        self.income_categories.sort_by(|a, b| a.name.cmp(&b.name));
        self.expenses = Expense::read_all(store)?;
        self.top_ups = TopUp::read_all(store)?;
        self.transfers = Transfer::read_all(store)?;
        self.summaries = ledger::summaries(
            &self.accounts,
            &self.expenses,
            &self.top_ups,
            &self.transfers,
        );

        // The account being looked at may be gone after a sync, or never
        // have been chosen: the default one, then whichever comes first.
        if !self.accounts.iter().any(|a| a.id == self.account) {
            self.account = self
                .accounts
                .iter()
                .find(|a| a.id == money_core::store::DEFAULT_ACCOUNT_ID)
                .or(self.accounts.first())
                .map(|a| a.id.clone())
                .unwrap_or_default();
        }
        self.ledger = match self.current_account() {
            Some(account) => ledger::ledger_for(
                account,
                &self.accounts,
                &self.expenses,
                &self.top_ups,
                &self.transfers,
            ),
            None => AccountLedger {
                expenses: Vec::new(),
                top_ups: Vec::new(),
            },
        };
        Ok(())
    }

    /// One write to the store, then the snapshot again.
    fn write(
        &mut self,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Store) -> Result<(), String>,
    ) -> Result<(), String> {
        let store = self.store.as_mut().ok_or(SYNCING)?;
        change(store)?;
        self.reload()?;
        cx.notify();
        Ok(())
    }

    // ------------------------------------------------------------ reading

    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    pub fn current_account(&self) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == self.account)
    }

    /// The currency amounts on screen are in.
    pub fn currency(&self) -> &str {
        self.current_account().map_or("", |a| a.currency.as_str())
    }

    pub fn summary(&self, account_id: &[u8]) -> AccountSummary {
        self.summaries.get(account_id).copied().unwrap_or_default()
    }

    /// The current account's records of one kind, newest first.
    pub fn entries(&self, mode: Mode) -> Vec<Entry> {
        let mut entries: Vec<Entry> = match mode {
            Mode::Expense => self.ledger.expenses.iter().map(Entry::from).collect(),
            Mode::Income => self.ledger.top_ups.iter().map(Entry::from).collect(),
        };
        entries.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
        entries
    }

    /// The categories a person made, without the virtual transfer one.
    pub fn categories(&self, mode: Mode) -> Vec<CategoryInfo> {
        match mode {
            Mode::Expense => self
                .expense_categories
                .iter()
                .map(|c| CategoryInfo {
                    id: c.id.clone(),
                    name: c.name.clone().into(),
                    style: c.style.clone(),
                })
                .collect(),
            Mode::Income => self
                .income_categories
                .iter()
                .map(|c| CategoryInfo {
                    id: c.id.clone(),
                    name: c.name.clone().into(),
                    style: c.style.clone(),
                })
                .collect(),
        }
    }

    /// A category's colour and icon. None for a transfer leg, for a category
    /// deleted on another device, and for one nobody styled.
    pub fn category_style(&self, mode: Mode, category_id: &[u8]) -> CategoryStyle {
        let style = match mode {
            Mode::Expense => self
                .expense_categories
                .iter()
                .find(|c| c.id == category_id)
                .map(|c| &c.style),
            Mode::Income => self
                .income_categories
                .iter()
                .find(|c| c.id == category_id)
                .map(|c| &c.style),
        };
        style.cloned().unwrap_or_default()
    }

    /// What a record's category is called: a transfer leg is a transfer, and
    /// a category that was deleted on another device is unknown.
    pub fn category_name(&self, mode: Mode, category_id: &[u8]) -> SharedString {
        if category_id == ledger::TRANSFER_CATEGORY_ID {
            return "Transfer".into();
        }
        self.categories(mode)
            .into_iter()
            .find(|c| c.id == category_id)
            .map(|c| c.name)
            .unwrap_or_else(|| "Unknown".into())
    }

    pub fn sync_state(&self) -> &SyncState {
        &self.sync
    }

    pub fn is_syncing(&self) -> bool {
        self.sync == SyncState::Running
    }

    /// Whether anything at all has been recorded, for the empty states.
    pub fn is_empty(&self) -> bool {
        self.expenses.is_empty() && self.top_ups.is_empty() && self.transfers.is_empty()
    }

    // ------------------------------------------------------------ writing

    pub fn select_account(&mut self, account_id: &[u8], cx: &mut Context<Self>) {
        if self.account == account_id || !self.accounts.iter().any(|a| a.id == account_id) {
            return;
        }
        self.account = account_id.to_vec();
        let hex = hex_encode(account_id);
        Settings::update(cx, |settings| settings.set_last_account(&hex));
        if let Err(error) = self.reload() {
            tracing::error!(%error, "could not reload the ledger");
        }
        cx.notify();
    }

    /// Records an expense or an income on the current account. With an `id`
    /// it rewrites that record whole — every field, the account included, as
    /// a CRDT update of a record has to.
    pub fn save_entry(
        &mut self,
        mode: Mode,
        id: Option<&[u8]>,
        draft: &EntryDraft,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let (category_id, amount, date) = (draft.category_id.as_slice(), draft.amount, draft.date);
        let comment = draft.comment.as_deref();
        if !(amount.is_finite() && amount > 0.0) {
            return Err("Amount must be greater than 0.".to_string());
        }
        let account = self.account.clone();
        self.write(cx, |store| match (mode, id) {
            (Mode::Expense, None) => {
                Expense::create(store, category_id, amount, comment, date, &account).map(drop)
            }
            (Mode::Expense, Some(id)) => {
                Expense::create_with_id(store, id, category_id, amount, comment, date, &account)
            }
            (Mode::Income, None) => {
                TopUp::create(store, category_id, amount, comment, date, &account).map(drop)
            }
            (Mode::Income, Some(id)) => {
                TopUp::create_with_id(store, id, category_id, amount, comment, date, &account)
            }
        })
    }

    /// Deletes a record. A transfer leg is the transfer: both legs go.
    pub fn delete_entry(
        &mut self,
        mode: Mode,
        entry: &Entry,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let id = entry.id.clone();
        let is_transfer = entry.is_transfer;
        self.write(cx, |store| {
            if is_transfer {
                Transfer::delete(store, &id).map(drop)
            } else {
                match mode {
                    Mode::Expense => Expense::delete(store, &id).map(drop),
                    Mode::Income => TopUp::delete(store, &id).map(drop),
                }
            }
        })
    }

    /// Makes a category, or rewrites one: its name, colour and icon together,
    /// since a record is always written whole. Answers its id.
    pub fn save_category(
        &mut self,
        mode: Mode,
        id: Option<&[u8]>,
        name: &str,
        style: &CategoryStyle,
        cx: &mut Context<Self>,
    ) -> Result<Vec<u8>, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Name is required.".to_string());
        }
        let mut saved = id.map(<[u8]>::to_vec);
        self.write(cx, |store| {
            match (mode, id) {
                (Mode::Expense, None) => {
                    saved = Some(Category::create_styled(store, name, style)?.id)
                }
                (Mode::Expense, Some(id)) => Category::create_with_id(store, id, name, style)?,
                (Mode::Income, None) => {
                    saved = Some(TopUpCategory::create_styled(store, name, style)?.id)
                }
                (Mode::Income, Some(id)) => TopUpCategory::create_with_id(store, id, name, style)?,
            }
            Ok(())
        })?;
        Ok(saved.unwrap_or_default())
    }

    /// Moves the colours and icons an earlier version kept in the settings
    /// into the ledger, once. A category the ledger already has a style for
    /// keeps it — that one may have come from another device — and a style
    /// for a category that is gone is dropped.
    ///
    /// The settings forget theirs only after the ledger took them, so a
    /// failed write leaves them to be tried on the next launch.
    pub fn adopt_legacy_styles(&mut self, cx: &mut Context<Self>) {
        let legacy = Settings::global(cx).legacy_category_styles();
        if legacy.is_empty() {
            return;
        }
        let rewrites: Vec<(Mode, CategoryInfo, CategoryStyle)> = Mode::ALL
            .into_iter()
            .flat_map(|mode| {
                styles_to_adopt(&self.categories(mode), &legacy)
                    .into_iter()
                    .map(move |(category, style)| (mode, category, style))
            })
            .collect();
        let moved = self.write(cx, |store| {
            for (mode, category, style) in &rewrites {
                match mode {
                    Mode::Expense => {
                        Category::create_with_id(store, &category.id, &category.name, style)?
                    }
                    Mode::Income => {
                        TopUpCategory::create_with_id(store, &category.id, &category.name, style)?
                    }
                }
            }
            Ok(())
        });
        match moved {
            Ok(()) => Settings::update(cx, Settings::forget_legacy_category_styles),
            Err(error) => tracing::warn!(%error, "could not move the category styles"),
        }
    }

    /// Makes an account, or rewrites one. Answers its id.
    pub fn save_account(
        &mut self,
        id: Option<&[u8]>,
        name: &str,
        currency: &str,
        opening_balance: f64,
        cx: &mut Context<Self>,
    ) -> Result<Vec<u8>, String> {
        let name = name.trim();
        let currency = currency.trim().to_uppercase();
        if name.is_empty() {
            return Err("Name is required.".to_string());
        }
        if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err("Currency is a three-letter code, like USD.".to_string());
        }
        if !opening_balance.is_finite() {
            return Err("Opening balance is not a number.".to_string());
        }
        let mut saved = id.map(<[u8]>::to_vec);
        self.write(cx, |store| {
            match id {
                None => saved = Some(Account::create(store, name, &currency, opening_balance)?.id),
                Some(id) => Account::create_with_id(store, id, name, &currency, opening_balance)?,
            }
            Ok(())
        })?;
        Ok(saved.unwrap_or_default())
    }

    pub fn add_transfer(
        &mut self,
        draft: &TransferDraft,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.write(cx, |store| {
            Transfer::create(
                store,
                &draft.from,
                &draft.to,
                draft.amount_from,
                draft.amount_to,
                draft.comment.as_deref(),
                draft.date,
            )
            .map(drop)
            .map_err(|error| match error.as_str() {
                "transfer must be between two different accounts" => {
                    "Choose two different accounts.".to_string()
                }
                _ => "Both amounts must be greater than 0.".to_string(),
            })
        })
    }

    /// Fills an empty ledger with sample records. See [`demo`].
    pub fn seed_demo(&mut self, today: NaiveDate, cx: &mut Context<Self>) -> Result<(), String> {
        if !self.is_empty() {
            return Ok(());
        }
        self.write(cx, |store| demo::seed(store, today))
    }

    // ------------------------------------------------------------ syncing

    /// Exchanges chunks with `remote` on a background thread. The store goes
    /// with the work and comes back with the result; see the module comment.
    ///
    /// The default account is made only *after* the exchange, and only if the
    /// remote did not bring one: made before, its fresh stamp would win over
    /// the synced account's name, currency and opening balance.
    pub fn sync(&mut self, remote: RemoteConfig, default_currency: String, cx: &mut Context<Self>) {
        if self.is_syncing() {
            return;
        }
        if let Err(message) = remote.validate() {
            self.sync = SyncState::Failed(message.into());
            cx.notify();
            return;
        }
        let Some(mut store) = self.store.take() else {
            return;
        };
        self.sync = SyncState::Running;
        cx.notify();

        cx.spawn(async move |book, cx| {
            let (store, result) = cx
                .background_spawn(async move {
                    let result = store.sync(&remote).and_then(|report| {
                        Account::ensure_default(&mut store, &default_currency)?;
                        Ok((report.pulled.len(), report.pushed.len()))
                    });
                    (store, result)
                })
                .await;
            book.update(cx, |book, cx| {
                book.store = Some(store);
                book.sync = match result {
                    Ok((pulled, pushed)) => SyncState::Done { pulled, pushed },
                    Err(error) => SyncState::Failed(describe_sync_error(&error).into()),
                };
                if let Err(error) = book.reload() {
                    book.sync = SyncState::Failed(error.into());
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

/// Which categories take a style from `legacy` (hex id, style): those the
/// ledger has no style for yet.
fn styles_to_adopt(
    categories: &[CategoryInfo],
    legacy: &[(String, CategoryStyle)],
) -> Vec<(CategoryInfo, CategoryStyle)> {
    categories
        .iter()
        .filter(|category| category.style.is_empty())
        .filter_map(|category| {
            let hex = hex_encode(&category.id);
            let (_, style) = legacy.iter().find(|(id, _)| *id == hex)?;
            Some((category.clone(), style.clone()))
        })
        .collect()
}

/// Turns a sync failure into something worth showing a person. The store's
/// own message stays at the end: it names the stage that failed.
fn describe_sync_error(error: &str) -> String {
    let lowered = error.to_lowercase();
    if lowered.contains("403") || lowered.contains("signature") || lowered.contains("accessdenied")
    {
        format!("The storage did not accept these credentials. ({error})")
    } else if lowered.contains("404") || lowered.contains("nosuchbucket") {
        format!("That bucket was not found. ({error})")
    } else if lowered.contains("decrypt") || lowered.contains("crypto") {
        format!("The encryption key does not open this ledger. ({error})")
    } else if lowered.contains("connect")
        || lowered.contains("dns")
        || lowered.contains("timed out")
    {
        format!("Couldn’t reach the storage. Check the endpoint and your connection. ({error})")
    } else {
        format!("Sync failed. ({error})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("money-manager-book-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_fresh_ledger_has_no_account_until_one_is_asked_for() {
        let dir = scratch("fresh");
        let mut book = Book::open(&dir, 7, None).unwrap();
        assert!(book.accounts().is_empty());
        assert!(book.current_account().is_none());

        book.ensure_default_account("EUR").unwrap();
        assert_eq!(book.accounts().len(), 1);
        assert_eq!(book.currency(), "EUR");
        // Asking again changes nothing, not even the currency.
        book.ensure_default_account("USD").unwrap();
        assert_eq!(book.accounts().len(), 1);
        assert_eq!(book.currency(), "EUR");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_account_last_looked_at_is_restored_when_it_still_exists() {
        let dir = scratch("last-account");
        let other = {
            let mut store = Store::open(&dir, 7).unwrap();
            Account::ensure_default(&mut store, "RUB").unwrap();
            Account::create(&mut store, "Savings", "USD", 10.0)
                .unwrap()
                .id
        };
        let book = Book::open(&dir, 7, Some(hex_encode(&other))).unwrap();
        assert_eq!(book.current_account().unwrap().name, "Savings");

        // One that is gone falls back to the default account.
        let book = Book::open(&dir, 7, Some("abcd".to_string())).unwrap();
        assert_eq!(
            book.current_account().unwrap().id,
            money_core::store::DEFAULT_ACCOUNT_ID.to_vec()
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_style_from_the_settings_goes_only_to_a_category_without_one() {
        let style = |color: &str| CategoryStyle {
            color: Some(color.to_string()),
            icon: None,
        };
        let category = |id: u8, style: CategoryStyle| CategoryInfo {
            id: vec![id],
            name: "Food".into(),
            style,
        };
        let categories = [
            category(0xaa, CategoryStyle::default()),
            // Already styled in the ledger, perhaps on another device.
            category(0xbb, style("#111111")),
            category(0xcc, CategoryStyle::default()),
        ];
        let legacy = [
            ("aa".to_string(), style("#e8853b")),
            ("bb".to_string(), style("#222222")),
            // A category that is gone.
            ("dd".to_string(), style("#333333")),
        ];
        let adopted = styles_to_adopt(&categories, &legacy);
        assert_eq!(adopted.len(), 1);
        assert_eq!(adopted[0].0.id, vec![0xaa]);
        assert_eq!(adopted[0].1, style("#e8853b"));
    }

    #[test]
    fn a_saved_category_keeps_its_style_in_the_ledger() {
        let dir = scratch("category-style");
        let id = {
            let mut store = Store::open(&dir, 7).unwrap();
            let style = CategoryStyle {
                color: Some("#4c8df6".into()),
                icon: Some("car".into()),
            };
            Category::create_styled(&mut store, "Taxi", &style)
                .unwrap()
                .id
        };
        let book = Book::open(&dir, 7, None).unwrap();
        let taxi = &book.categories(Mode::Expense)[0];
        assert_eq!(taxi.style.icon.as_deref(), Some("car"));
        assert_eq!(
            book.category_style(Mode::Expense, &id).color.as_deref(),
            Some("#4c8df6")
        );
        // Of the other kind, or unknown: no style, not an error.
        assert!(book.category_style(Mode::Income, &id).is_empty());
        assert!(book.category_style(Mode::Expense, &[9]).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_sync_failure_says_what_to_check() {
        assert!(describe_sync_error("sync: HTTP 403 Forbidden").starts_with("The storage did not"));
        assert!(describe_sync_error("open S3 store: NoSuchBucket").starts_with("That bucket"));
        assert!(describe_sync_error("sync: error trying to connect").starts_with("Couldn’t reach"));
        assert!(describe_sync_error("merge chunks: bad").starts_with("Sync failed."));
    }
}
