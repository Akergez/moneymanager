//! The open ledger, as the interface sees it.
//!
//! [`Book`] is the one owner of the [`Store`]. Every screen reads from the
//! snapshot it keeps and asks it to make changes; nothing else touches the
//! store, which is what keeps the native file, the in-memory document and
//! what is on screen from ever disagreeing.
//!
//! Sync and CSV file operations run on a background thread *with the store
//! moved into it*. For as long as one runs
//! the book has no store, and a write is refused with a reason rather than
//! queued: a write that landed during the exchange would be
//! written against a document that is about to be replaced.

mod demo;
mod entry;
mod style;

use std::collections::HashMap;
use std::path::Path;

use chrono::NaiveDate;
use gpui_kit::{AppContext, Context, SharedString};
use money_core::ledger::{self, AccountLedger, AccountSummary};
use money_core::models::{
    Account, Category, CategoryState, Expense, Redirects, TopUp, TopUpCategory, Transfer,
};
use money_core::store::{Store, hex_encode};
use money_core::tags::{Tag, TagKind};

pub use entry::{
    Entry, EntryDraft, Filter, Mode, TransferDraft, latest_category, latest_date, order_by_use,
};
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

/// The kind of tag a record of `mode` carries.
pub fn tag_kind(mode: Mode) -> TagKind {
    match mode {
        Mode::Expense => TagKind::Expense,
        Mode::Income => TagKind::TopUp,
    }
}

/// A category of either kind, which is all a list or a chart needs of one.
/// A tag is one too: a name, a colour and an icon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CategoryInfo {
    pub id: Vec<u8>,
    pub name: SharedString,
    /// Its colour and icon, as the ledger has them.
    pub style: CategoryStyle,
    /// Kept out of the choice for a new record, but listed, and its records
    /// shown in it.
    pub hidden: bool,
}

impl CategoryInfo {
    fn of(id: &[u8], name: &str, style: &CategoryStyle, state: &CategoryState) -> Option<Self> {
        state.is_live().then(|| CategoryInfo {
            id: id.to_vec(),
            name: name.to_string().into(),
            style: style.clone(),
            hidden: *state == CategoryState::Hidden,
        })
    }

    /// The state it is in, which a rewrite of its name or style keeps.
    fn state(&self) -> CategoryState {
        match self.hidden {
            true => CategoryState::Hidden,
            false => CategoryState::Visible,
        }
    }
}

/// What a record in a removed category, or in one the ledger does not have,
/// is shown as: every one of them alike.
pub const UNNAMED_CATEGORY: &str = "Uncategorized";

pub struct Book {
    /// Gone while a sync has it.
    store: Option<Store>,
    accounts: Vec<Account>,
    account_preferences: money_core::account_preferences::AccountPreferences,
    expense_categories: Vec<Category>,
    income_categories: Vec<TopUpCategory>,
    expenses: Vec<Expense>,
    top_ups: Vec<TopUp>,
    transfers: Vec<Transfer>,
    /// The tags of each kind, by name.
    tags: HashMap<TagKind, Vec<Tag>>,
    /// The tags on each record, by record id; each in the order of `tags`.
    links: HashMap<TagKind, HashMap<Vec<u8>, Vec<Vec<u8>>>>,
    summaries: HashMap<Vec<u8>, AccountSummary>,
    /// The account every list and chart is about.
    account: Vec<u8>,
    /// That account's records, transfer legs included.
    ledger: AccountLedger,
    sync: SyncState,
}

const SYNCING: &str = "The ledger is busy. Try again when it finishes.";

impl Book {
    /// Whether an operation currently owns the store on a background thread.
    pub fn is_busy(&self) -> bool {
        self.store.is_none()
    }

    /// Import or export the complete ledger without blocking the window.
    pub fn csv_file(
        &mut self,
        path: Option<std::path::PathBuf>,
        export: bool,
        cx: &mut Context<Self>,
    ) -> Result<gpui_kit::Task<Result<Option<money_core::csv::ImportReport>, String>>, String> {
        let mut store = self.store.take().ok_or(SYNCING)?;
        cx.notify();
        Ok(cx.spawn(async move |book, cx| {
            let (store, result) = cx
                .background_spawn(async move {
                    let result = (|| {
                        if export {
                            let mut bytes = Vec::new();
                            money_core::csv::export(&store, &mut bytes)?;
                            #[cfg(target_os = "android")]
                            {
                                gpui_android_glue::documents::save(
                                    &bytes,
                                    "text/csv",
                                    "transactions.csv",
                                )
                                .map(|saved| saved.then_some(Default::default()))
                            }
                            #[cfg(not(target_os = "android"))]
                            {
                                let path = path.ok_or("No export file selected")?;
                                std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
                                Ok(Some(Default::default()))
                            }
                        } else {
                            let path = path.ok_or("No import file selected")?;
                            let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
                            money_core::csv::import(&mut store, file).map(Some)
                        }
                    })();
                    (store, result)
                })
                .await;
            book.update(cx, |book, cx| {
                book.store = Some(store);
                let reload = book.reload();
                cx.notify();
                reload?;
                result
            })
            .map_err(|e| e.to_string())?
        }))
    }

    /// Opens the ledger in `dir`, creating an empty one if nothing is there.
    /// The synced main account takes precedence over legacy selection memory.
    /// No account is made: a ledger about to be filled by its first sync must
    /// not get a fresh default account that would outrank the synced one.
    pub fn open(dir: &Path, source: u64, last_account: Option<String>) -> Result<Self, String> {
        let store = Store::open(dir, source)?;
        let mut book = Book {
            store: Some(store),
            accounts: Vec::new(),
            account_preferences: Default::default(),
            expense_categories: Vec::new(),
            income_categories: Vec::new(),
            expenses: Vec::new(),
            top_ups: Vec::new(),
            transfers: Vec::new(),
            tags: HashMap::new(),
            links: HashMap::new(),
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
        if let Some(main) = book.account_preferences.main_id() {
            book.account = main.to_vec();
            book.reload()?;
        }
        Ok(book)
    }

    /// Makes sure the default account exists. Idempotent, and the id is
    /// fixed, so two devices doing this merge to one account.
    pub fn ensure_default_account(&mut self, currency: &str) -> Result<(), String> {
        let store = self.store.as_mut().ok_or(SYNCING)?;
        Account::ensure_default(store, currency)?;
        self.reload()
    }

    pub fn create_first_account(&mut self, name: &str, currency: &str) -> Result<(), String> {
        let store = self.store.as_mut().ok_or(SYNCING)?;
        Account::create_with_id(
            store,
            &money_core::store::DEFAULT_ACCOUNT_ID,
            name,
            currency,
            0.0,
        )?;
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
        self.account_preferences =
            money_core::account_preferences::AccountPreferences::read(store, &self.accounts);
        self.expense_categories = Category::read_all(store)?;
        self.expense_categories.sort_by(|a, b| a.name.cmp(&b.name));
        self.income_categories = TopUpCategory::read_all(store)?;
        self.income_categories.sort_by(|a, b| a.name.cmp(&b.name));
        // A record of a merged category is shown in the one it went into,
        // as if it were there: lists, counts, charts and edits all see it so.
        // The record itself is not rewritten.
        let expense_redirects = Redirects::new(
            self.expense_categories
                .iter()
                .map(|c| (c.id.clone(), c.state.clone())),
        );
        let income_redirects = Redirects::new(
            self.income_categories
                .iter()
                .map(|c| (c.id.clone(), c.state.clone())),
        );
        self.expenses = Expense::read_all(store)?;
        for expense in &mut self.expenses {
            expense.category_id = expense_redirects.resolve(&expense.category_id);
        }
        self.top_ups = TopUp::read_all(store)?;
        for top_up in &mut self.top_ups {
            top_up.category_id = income_redirects.resolve(&top_up.category_id);
        }
        self.transfers = Transfer::read_all(store)?;
        for kind in [TagKind::Expense, TagKind::TopUp] {
            let mut tags = Tag::read_all(store, kind);
            tags.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
            let order = |id: &Vec<u8>| tags.iter().position(|tag| &tag.id == id);
            let mut links = Tag::shown_links(store, kind);
            for on in links.values_mut() {
                on.sort_by_key(order);
            }
            self.links.insert(kind, links);
            self.tags.insert(kind, tags);
        }
        self.summaries = ledger::summaries(
            &self.accounts,
            &self.expenses,
            &self.top_ups,
            &self.transfers,
        );

        // A hidden or missing selection falls back to the resolved main account.
        if !self.visible_accounts().any(|a| a.id == self.account) {
            self.account = self
                .account_preferences
                .main_id()
                .unwrap_or_default()
                .to_vec();
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

    pub fn visible_accounts(&self) -> impl Iterator<Item = &Account> {
        self.accounts
            .iter()
            .filter(|account| !self.is_account_hidden(&account.id))
    }

    pub fn is_account_hidden(&self, id: &[u8]) -> bool {
        self.account_preferences.is_hidden(id)
    }

    pub fn main_account(&self) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|account| self.account_preferences.main_id() == Some(account.id.as_slice()))
    }

    pub fn set_account_hidden(
        &mut self,
        id: &[u8],
        hidden: bool,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.write(cx, |store| {
            money_core::account_preferences::AccountPreferences::set_hidden(store, id, hidden)
        })
    }

    pub fn set_main_account(&mut self, id: &[u8], cx: &mut Context<Self>) -> Result<(), String> {
        self.write(cx, |store| {
            money_core::account_preferences::AccountPreferences::set_main(store, id)
        })
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
        if let Some(links) = self.links.get(&tag_kind(mode)) {
            for entry in entries.iter_mut().filter(|entry| !entry.is_transfer) {
                if let Some(tags) = links.get(&entry.id) {
                    entry.tags = tags.clone();
                }
            }
        }
        entries.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id)));
        entries
    }

    /// The categories the interface has, hidden ones included, without the
    /// virtual transfer one. A merged or removed category is not among them.
    pub fn categories(&self, mode: Mode) -> Vec<CategoryInfo> {
        match mode {
            Mode::Expense => self
                .expense_categories
                .iter()
                .filter_map(|c| CategoryInfo::of(&c.id, &c.name, &c.style, &c.state))
                .collect(),
            Mode::Income => self
                .income_categories
                .iter()
                .filter_map(|c| CategoryInfo::of(&c.id, &c.name, &c.style, &c.state))
                .collect(),
        }
    }

    /// The tags of `mode`'s records the interface has, hidden ones
    /// included, by name.
    pub fn tags(&self, mode: Mode) -> Vec<CategoryInfo> {
        self.tags
            .get(&tag_kind(mode))
            .into_iter()
            .flatten()
            .filter_map(|tag| CategoryInfo::of(&tag.id, &tag.name, &tag.style, &tag.state))
            .collect()
    }

    /// One tag of `mode`, if it is still there.
    pub fn tag(&self, mode: Mode, id: &[u8]) -> Option<CategoryInfo> {
        self.tags(mode).into_iter().find(|tag| tag.id == id)
    }

    /// One category of `mode`, if it is still there.
    pub fn category(&self, mode: Mode, id: &[u8]) -> Option<CategoryInfo> {
        self.categories(mode).into_iter().find(|c| c.id == id)
    }

    /// A category's colour and icon. None for a transfer leg, for a removed
    /// category, so that every one of them looks alike, and for one nobody
    /// styled.
    pub fn category_style(&self, mode: Mode, category_id: &[u8]) -> CategoryStyle {
        self.category(mode, category_id)
            .map(|c| c.style)
            .unwrap_or_default()
    }

    /// What a record's category is called: a transfer leg is a transfer, and
    /// a removed category, or one the ledger does not have, has no name.
    pub fn category_name(&self, mode: Mode, category_id: &[u8]) -> SharedString {
        if category_id == ledger::TRANSFER_CATEGORY_ID {
            return "Transfer".into();
        }
        self.category(mode, category_id)
            .map(|c| c.name)
            .unwrap_or_else(|| UNNAMED_CATEGORY.into())
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
        if self.account == account_id || !self.visible_accounts().any(|a| a.id == account_id) {
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
        let tags = draft.tags.clone();
        // An edit that only changed the tags leaves the record as it was.
        let record_unchanged = id.is_some_and(|id| {
            self.entries(mode).iter().any(|entry| {
                entry.id == id
                    && entry.category_id == draft.category_id
                    && entry.amount == amount
                    && entry.comment == draft.comment
                    && entry.date == date
            })
        });
        self.write(cx, |store| {
            let id = match (mode, id) {
                (_, Some(id)) if record_unchanged => id.to_vec(),
                (Mode::Expense, None) => {
                    Expense::create(store, category_id, amount, comment, date, &account)?.id
                }
                (Mode::Expense, Some(id)) => {
                    Expense::create_with_id(
                        store,
                        id,
                        category_id,
                        amount,
                        comment,
                        date,
                        &account,
                    )?;
                    id.to_vec()
                }
                (Mode::Income, None) => {
                    TopUp::create(store, category_id, amount, comment, date, &account)?.id
                }
                (Mode::Income, Some(id)) => {
                    TopUp::create_with_id(store, id, category_id, amount, comment, date, &account)?;
                    id.to_vec()
                }
            };
            Tag::set_for_record(store, tag_kind(mode), &id, &tags)
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
                    Mode::Expense => Expense::delete(store, &id).map(drop)?,
                    Mode::Income => TopUp::delete(store, &id).map(drop)?,
                }
                Tag::set_for_record(store, tag_kind(mode), &id, &[])
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
        // A hidden category stays hidden through a rename.
        let state = id
            .and_then(|id| self.category(mode, id))
            .map_or(CategoryState::Visible, |c| c.state());
        self.write(cx, |store| {
            match (mode, id) {
                (Mode::Expense, None) => {
                    saved = Some(Category::create_styled(store, name, style)?.id)
                }
                (Mode::Expense, Some(id)) => {
                    Category::create_with_id(store, id, name, style, &state)?
                }
                (Mode::Income, None) => {
                    saved = Some(TopUpCategory::create_styled(store, name, style)?.id)
                }
                (Mode::Income, Some(id)) => {
                    TopUpCategory::create_with_id(store, id, name, style, &state)?
                }
            }
            Ok(())
        })?;
        Ok(saved.unwrap_or_default())
    }

    /// Hides a category, shows it again, merges it into another or removes
    /// it. Its record stays, with its name and style; only the state
    /// changes, and its records are not touched.
    pub fn set_category_state(
        &mut self,
        mode: Mode,
        id: &[u8],
        state: CategoryState,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let category = self
            .category(mode, id)
            .ok_or("That category is no longer there.")?;
        self.check_merge(id, &state, self.category_ids(mode))?;
        self.write(cx, |store| match mode {
            Mode::Expense => {
                Category::create_with_id(store, id, &category.name, &category.style, &state)
            }
            Mode::Income => {
                TopUpCategory::create_with_id(store, id, &category.name, &category.style, &state)
            }
        })
    }

    /// The same for a tag: a merged one is shown on its records as the tag
    /// it went into, a removed one not at all.
    pub fn set_tag_state(
        &mut self,
        mode: Mode,
        id: &[u8],
        state: CategoryState,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let tag = self.tag(mode, id).ok_or("That tag is no longer there.")?;
        let ids = self.tags(mode).into_iter().map(|t| t.id).collect();
        self.check_merge(id, &state, ids)?;
        let kind = tag_kind(mode);
        self.write(cx, |store| {
            Tag::create_with_id(store, kind, id, &tag.name, &tag.style, &state)
        })
    }

    fn category_ids(&self, mode: Mode) -> Vec<Vec<u8>> {
        self.categories(mode).into_iter().map(|c| c.id).collect()
    }

    /// A merge goes into another one the interface has, of the same kind.
    fn check_merge(
        &self,
        id: &[u8],
        state: &CategoryState,
        live: Vec<Vec<u8>>,
    ) -> Result<(), String> {
        match state {
            CategoryState::Merged(into) if into == id || !live.contains(into) => {
                Err("Choose another one to move the records to.".to_string())
            }
            _ => Ok(()),
        }
    }

    /// Makes a tag, or rewrites one: its name, colour and icon together.
    /// Answers its id.
    pub fn save_tag(
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
        // The CSV file lists a record's tags separated by it.
        if name.contains(money_core::csv::TAG_SEPARATOR) {
            return Err(format!(
                "A tag’s name can’t have “{}” in it.",
                money_core::csv::TAG_SEPARATOR
            ));
        }
        let kind = tag_kind(mode);
        let mut saved = id.map(<[u8]>::to_vec);
        // A hidden tag stays hidden through a rename.
        let state = id
            .and_then(|id| self.tag(mode, id))
            .map_or(CategoryState::Visible, |t| t.state());
        self.write(cx, |store| {
            match id {
                None => saved = Some(Tag::create(store, kind, name, style)?.id),
                Some(id) => Tag::create_with_id(store, kind, id, name, style, &state)?,
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
                let state = category.state();
                match mode {
                    Mode::Expense => Category::create_with_id(
                        store,
                        &category.id,
                        &category.name,
                        style,
                        &state,
                    )?,
                    Mode::Income => TopUpCategory::create_with_id(
                        store,
                        &category.id,
                        &category.name,
                        style,
                        &state,
                    )?,
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

    /// Writes the ledger into fresh Tresse storage: every record stays,
    /// the version history and the sync settings on this device go. See
    /// `money_core::store::rebuild_storage`. Answers how many records were
    /// carried; refused while a sync has the store.
    pub fn rebuild_storage(&mut self, cx: &mut Context<Self>) -> Result<usize, String> {
        let store = self
            .store
            .as_mut()
            .ok_or_else(|| "Wait for the sync to finish first.".to_string())?;
        let records = store.rebuild()?;
        self.sync = SyncState::Idle;
        self.reload()?;
        cx.notify();
        Ok(records)
    }

    /// Exchanges Tresse objects through the configured origin on a background thread. The store goes
    /// with the work and comes back with the result; see the module comment.
    ///
    /// The default account is made only *after* the exchange, and only if the
    /// remote did not bring one: made before, its fresh stamp would win over
    /// the synced account's name, currency and opening balance.
    pub fn sync(&mut self, default_currency: String, cx: &mut Context<Self>) {
        if self.is_syncing() {
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
                    let result = store.sync().and_then(|report| {
                        Account::ensure_default(&mut store, &default_currency)?;
                        Ok((report.pulled, report.pushed))
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
    fn the_main_account_is_opened_instead_of_the_last_viewed_account() {
        let dir = scratch("last-account");
        let other = {
            let mut store = Store::open(&dir, 7).unwrap();
            Account::ensure_default(&mut store, "RUB").unwrap();
            Account::create(&mut store, "Savings", "USD", 10.0)
                .unwrap()
                .id
        };
        let mut store = Store::open(&dir, 7).unwrap();
        money_core::account_preferences::AccountPreferences::set_main(&mut store, &other).unwrap();
        let book = Book::open(
            &dir,
            7,
            Some(hex_encode(&money_core::store::DEFAULT_ACCOUNT_ID)),
        )
        .unwrap();
        assert_eq!(book.current_account().unwrap().name, "Savings");

        // A stale local setting cannot override the synced main account.
        let book = Book::open(&dir, 7, Some("abcd".to_string())).unwrap();
        assert_eq!(book.current_account().unwrap().id, other);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn hiding_the_selected_account_switches_to_the_main_without_changing_transfers() {
        let dir = scratch("hidden-account");
        let mut store = Store::open(&dir, 7).unwrap();
        Account::ensure_default(&mut store, "RUB").unwrap();
        let other = Account::create(&mut store, "Savings", "USD", 0.0).unwrap();
        let mut book = Book::open(&dir, 7, None).unwrap();
        book.account = other.id.clone();
        money_core::account_preferences::AccountPreferences::set_hidden(
            book.store.as_mut().unwrap(),
            &other.id,
            true,
        )
        .unwrap();
        book.reload().unwrap();
        assert_eq!(
            book.current_account().unwrap().id,
            money_core::store::DEFAULT_ACCOUNT_ID
        );
        assert_eq!(book.accounts().len(), 2);
        assert_eq!(book.visible_accounts().count(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_named_first_account_is_preserved_after_reopening() {
        let dir = scratch("named-account");
        let mut book = Book::open(&dir, 7, None).unwrap();
        book.create_first_account("Cash", "EUR").unwrap();
        let mut book = Book::open(&dir, 8, None).unwrap();
        book.ensure_default_account("USD").unwrap();
        assert_eq!(book.current_account().unwrap().name, "Cash");
        assert_eq!(book.currency(), "EUR");
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
            hidden: false,
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

    #[test]
    fn a_record_keeps_its_tags_through_an_edit_and_loses_them_when_deleted() {
        let dir = scratch("tags");
        let mut store = Store::open(&dir, 7).unwrap();
        Account::ensure_default(&mut store, "RUB").unwrap();
        let food = Category::create(&mut store, "Food").unwrap();
        let a = Tag::create(&mut store, TagKind::Expense, "B-trip", &Default::default()).unwrap();
        let b = Tag::create(&mut store, TagKind::Expense, "A-work", &Default::default()).unwrap();
        drop(store);
        let mut book = Book::open(&dir, 7, None).unwrap();
        let tags = book.tags(Mode::Expense);
        assert_eq!(tags[0].name.as_ref(), "A-work");
        let store = book.store.as_mut().unwrap();
        let draft = EntryDraft {
            category_id: food.id.clone(),
            amount: 5.0,
            comment: None,
            date: NaiveDate::from_ymd_opt(2026, 11, 7).unwrap(),
            tags: vec![a.id.clone(), b.id.clone()],
        };
        let made = Expense::create(store, &food.id, 5.0, None, draft.date, &book.account).unwrap();
        Tag::set_for_record(store, TagKind::Expense, &made.id, &draft.tags).unwrap();
        book.reload().unwrap();
        let entry = &book.entries(Mode::Expense)[0];
        // In the order of the names.
        assert_eq!(entry.tags, vec![b.id.clone(), a.id.clone()]);
        assert!(Filter::Tag(a.id.clone()).admits(entry));
        assert!(book.entries(Mode::Income).is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn hidden_merged_and_removed_categories_and_tags_are_shown_as_they_say() {
        let dir = scratch("category-states");
        let mut store = Store::open(&dir, 7).unwrap();
        Account::ensure_default(&mut store, "RUB").unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();
        let account = money_core::store::DEFAULT_ACCOUNT_ID;
        let taxi = Category::create(&mut store, "Taxi").unwrap();
        let transport = Category::create(&mut store, "Transport").unwrap();
        let cafe = Category::create(&mut store, "Cafe").unwrap();
        let ride = Expense::create(&mut store, &taxi.id, 10.0, None, day, &account).unwrap();
        let lunch = Expense::create(&mut store, &cafe.id, 20.0, None, day, &account).unwrap();
        let trip = Tag::create(&mut store, TagKind::Expense, "Trip", &Default::default()).unwrap();
        let travel =
            Tag::create(&mut store, TagKind::Expense, "Travel", &Default::default()).unwrap();
        Tag::set_for_record(
            &mut store,
            TagKind::Expense,
            &ride.id,
            std::slice::from_ref(&trip.id),
        )
        .unwrap();
        drop(store);
        let mut book = Book::open(&dir, 7, None).unwrap();
        let store = book.store.as_mut().unwrap();
        let style = CategoryStyle::default();
        // What the interface's commands write.
        Category::create_with_id(
            store,
            &taxi.id,
            "Taxi",
            &style,
            &CategoryState::Merged(transport.id.clone()),
        )
        .unwrap();
        Category::create_with_id(store, &cafe.id, "Cafe", &style, &CategoryState::Removed).unwrap();
        Category::create_with_id(
            store,
            &transport.id,
            "Transport",
            &style,
            &CategoryState::Hidden,
        )
        .unwrap();
        Tag::create_with_id(
            store,
            TagKind::Expense,
            &trip.id,
            "Trip",
            &style,
            &CategoryState::Merged(travel.id.clone()),
        )
        .unwrap();
        book.reload().unwrap();

        let categories = book.categories(Mode::Expense);
        assert_eq!(categories.len(), 1);
        assert!(categories[0].hidden);
        let entries = book.entries(Mode::Expense);
        let entry = |id: &[u8]| entries.iter().find(|e| e.id == id).unwrap().clone();
        // The ride is shown in Transport, with Travel on it.
        assert_eq!(entry(&ride.id).category_id, transport.id);
        assert_eq!(entry(&ride.id).tags, vec![travel.id.clone()]);
        assert_eq!(
            book.category_name(Mode::Expense, &entry(&ride.id).category_id),
            "Transport"
        );
        // The lunch keeps pointing at Cafe, shown without a name.
        assert_eq!(entry(&lunch.id).category_id, cafe.id);
        assert_eq!(
            book.category_name(Mode::Expense, &cafe.id),
            UNNAMED_CATEGORY
        );
        assert!(book.category_style(Mode::Expense, &cafe.id).is_empty());
        // The records themselves were not rewritten.
        let stored = Expense::read_all(book.store.as_ref().unwrap()).unwrap();
        assert!(
            stored
                .iter()
                .any(|e| e.id == ride.id && e.category_id == taxi.id)
        );
        assert_eq!(book.tags(Mode::Expense).len(), 1);

        // A merge needs another live one to go into.
        assert!(
            book.check_merge(
                &transport.id,
                &CategoryState::Merged(cafe.id.clone()),
                book.category_ids(Mode::Expense)
            )
            .is_err()
        );
        assert!(
            book.check_merge(
                &transport.id,
                &CategoryState::Merged(transport.id.clone()),
                book.category_ids(Mode::Expense)
            )
            .is_err()
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
