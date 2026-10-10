//! Synced account visibility and one shared startup-account register.
//!
//! Separate keys keep hiding, restoring and account-detail edits independent.
//! The main-account reference is one LWW record, never one flag per account.
//! Resolution is deterministic even when concurrent hides leave no visible
//! account: the default (or first by id) remains accessible as the main one.

use std::collections::HashSet;

use rdx_rs::RdxValue;

use crate::models::Account;
use crate::store::{
    ACCOUNT_PREFERENCES_IDX, DEFAULT_ACCOUNT_ID, Store, child_str, hex_decode, hex_encode,
};

const MAIN: &str = "main_account";

#[derive(Default)]
pub struct AccountPreferences {
    hidden: HashSet<Vec<u8>>,
    main: Option<Vec<u8>>,
}

impl AccountPreferences {
    pub fn read(store: &Store, accounts: &[Account]) -> Self {
        let mut preferences = Self::default();
        for record in store.records(ACCOUNT_PREFERENCES_IDX) {
            let key = child_str(record, 0);
            let value = child_str(record, 1);
            if key == MAIN {
                preferences.main = Some(hex_decode(&value));
            } else if let Some(id) = key.strip_prefix("hidden:")
                && value == "true"
            {
                preferences.hidden.insert(hex_decode(id));
            }
        }
        let visible = |account: &&Account| !preferences.hidden.contains(&account.id);
        let main = accounts
            .iter()
            .filter(visible)
            .find(|account| preferences.main.as_deref() == Some(account.id.as_slice()))
            .or_else(|| {
                accounts
                    .iter()
                    .filter(visible)
                    .find(|account| account.id == DEFAULT_ACCOUNT_ID)
            })
            .or_else(|| {
                accounts
                    .iter()
                    .filter(visible)
                    .min_by(|a, b| a.id.cmp(&b.id))
            })
            .or_else(|| {
                accounts
                    .iter()
                    .find(|account| account.id == DEFAULT_ACCOUNT_ID)
            })
            .or_else(|| accounts.iter().min_by(|a, b| a.id.cmp(&b.id)))
            .map(|account| account.id.clone());
        if let Some(main) = &main {
            preferences.hidden.remove(main);
        }
        preferences.main = main;
        preferences
    }

    pub fn is_hidden(&self, id: &[u8]) -> bool {
        self.hidden.contains(id)
    }

    pub fn main_id(&self) -> Option<&[u8]> {
        self.main.as_deref()
    }

    pub fn set_hidden(store: &mut Store, id: &[u8], hidden: bool) -> Result<(), String> {
        let accounts = Account::read_all(store)?;
        if !accounts.iter().any(|account| account.id == id) {
            return Err("Account no longer exists.".into());
        }
        let preferences = Self::read(store, &accounts);
        if hidden
            && accounts
                .iter()
                .filter(|account| !preferences.is_hidden(&account.id))
                .count()
                <= 1
            && !preferences.is_hidden(id)
        {
            return Err("Keep at least one account visible.".into());
        }
        store.upsert(
            ACCOUNT_PREFERENCES_IDX,
            &format!("hidden:{}", hex_encode(id)),
            vec![RdxValue::Str(hidden.to_string())],
        )
    }

    pub fn set_main(store: &mut Store, id: &[u8]) -> Result<(), String> {
        if !Account::read_all(store)?
            .iter()
            .any(|account| account.id == id)
        {
            return Err("Account no longer exists.".into());
        }
        // Selecting a hidden account as main restores it in the same write.
        store.import_records(vec![
            (
                ACCOUNT_PREFERENCES_IDX,
                MAIN.into(),
                vec![RdxValue::Str(hex_encode(id))],
            ),
            (
                ACCOUNT_PREFERENCES_IDX,
                format!("hidden:{}", hex_encode(id)),
                vec![RdxValue::Str("false".into())],
            ),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Category, Expense, TopUp, TopUpCategory, Transfer};
    use crate::store::LEDGER_FILE;
    use chrono::NaiveDate;
    use rdx_sync::ObjectStore;

    fn setup() -> (tempfile::TempDir, Store, Vec<Account>) {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path(), 7).unwrap();
        Account::ensure_default(&mut store, "USD").unwrap();
        Account::create_with_id(&mut store, &[1; 16], "Savings", "EUR", 10.0).unwrap();
        Account::create_with_id(&mut store, &[2; 16], "Card", "USD", 20.0).unwrap();
        let accounts = Account::read_all(&store).unwrap();
        (dir, store, accounts)
    }

    fn document(dir: &std::path::Path) -> rdx_rs::RdxElement {
        rdx_rs::decode(&std::fs::read(dir.join(LEDGER_FILE)).unwrap())
            .unwrap()
            .0
    }

    fn merged_preferences(a: &std::path::Path, b: &std::path::Path) -> AccountPreferences {
        let left = document(a);
        let right = document(b);
        let merged = rdx_rs::merge(&left, &right);
        assert_eq!(merged, rdx_rs::merge(&right, &left));
        assert_eq!(merged, rdx_rs::merge(&merged, &left));
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LEDGER_FILE), rdx_rs::encode(&merged)).unwrap();
        let store = Store::open(dir.path(), 9).unwrap();
        AccountPreferences::read(&store, &Account::read_all(&store).unwrap())
    }

    #[test]
    fn hiding_and_restoring_preserve_operations_balances_and_versions() {
        let (dir, mut store, accounts) = setup();
        let id = accounts[0].id.clone();
        let other = accounts[1].id.clone();
        let date = NaiveDate::from_ymd_opt(2026, 10, 11).unwrap();
        let expense_category = Category::create(&mut store, "Food").unwrap();
        let income_category = TopUpCategory::create(&mut store, "Salary").unwrap();
        Expense::create(&mut store, &expense_category.id, 2.0, None, date, &id).unwrap();
        TopUp::create(&mut store, &income_category.id, 3.0, None, date, &id).unwrap();
        Transfer::create(&mut store, &id, &other, 4.0, 5.0, None, date).unwrap();
        Transfer::create(&mut store, &other, &id, 6.0, 7.0, None, date).unwrap();
        let balances = |store: &Store| {
            crate::ledger::summaries(
                &accounts,
                &Expense::read_all(store).unwrap(),
                &TopUp::read_all(store).unwrap(),
                &Transfer::read_all(store).unwrap(),
            )
        };
        let before = balances(&store);
        let mut csv = Vec::new();
        crate::csv::export(&store, &mut csv).unwrap();
        store.seal().unwrap();
        let local = rdx_sync_fs::FileObjectStore::open(dir.path().join(".tresse")).unwrap();
        let versions = local.list_objects(rdx_sync::ChunkClass::Meta).unwrap();
        let blobs = local.list_objects(rdx_sync::ChunkClass::Content).unwrap();
        AccountPreferences::set_hidden(&mut store, &id, true).unwrap();
        assert!(AccountPreferences::read(&store, &accounts).is_hidden(&id));
        assert_eq!(balances(&store)[&id].balance, before[&id].balance);
        assert_eq!(balances(&store)[&other].balance, before[&other].balance);
        let mut exported = Vec::new();
        crate::csv::export(&store, &mut exported).unwrap();
        assert_eq!(exported, csv);
        AccountPreferences::set_hidden(&mut store, &id, false).unwrap();
        Account::update(&mut store, &id, "Renamed", "USD", 0.0).unwrap();
        let reopened = Store::open(dir.path(), 8).unwrap();
        assert!(!AccountPreferences::read(&reopened, &accounts).is_hidden(&id));
        assert_eq!(Transfer::read_all(&reopened).unwrap().len(), 2);
        assert_eq!(
            local.list_objects(rdx_sync::ChunkClass::Meta).unwrap(),
            versions
        );
        assert_eq!(
            local.list_objects(rdx_sync::ChunkClass::Content).unwrap(),
            blobs
        );
    }

    #[test]
    fn concurrent_main_choices_resolve_to_one_account_and_preserve_independent_hides() {
        let (a, mut first, accounts) = setup();
        let b = tempfile::tempdir().unwrap();
        std::fs::write(
            b.path().join(LEDGER_FILE),
            rdx_rs::encode(&document(a.path())),
        )
        .unwrap();
        let mut second = Store::open(b.path(), 8).unwrap();
        AccountPreferences::set_main(&mut first, &[1; 16]).unwrap();
        AccountPreferences::set_main(&mut second, &[2; 16]).unwrap();
        AccountPreferences::set_hidden(&mut first, &DEFAULT_ACCOUNT_ID, true).unwrap();
        Account::update(&mut second, &DEFAULT_ACCOUNT_ID, "Renamed", "USD", 0.0).unwrap();
        let preferences = merged_preferences(a.path(), b.path());
        assert!(preferences.is_hidden(&DEFAULT_ACCOUNT_ID));
        let main = preferences.main_id().unwrap();
        assert!(main == [1; 16] || main == [2; 16]);
        assert_eq!(
            accounts
                .iter()
                .filter(|account| preferences.main_id() == Some(account.id.as_slice()))
                .count(),
            1
        );
        assert!(!preferences.is_hidden(main));
    }

    #[test]
    fn concurrent_hides_keep_one_accessible_account_and_a_later_restore_wins() {
        let dir = tempfile::tempdir().unwrap();
        let mut first = Store::open(dir.path(), 7).unwrap();
        Account::ensure_default(&mut first, "USD").unwrap();
        Account::create_with_id(&mut first, &[1; 16], "Savings", "USD", 0.0).unwrap();
        let other = tempfile::tempdir().unwrap();
        std::fs::write(
            other.path().join(LEDGER_FILE),
            rdx_rs::encode(&document(dir.path())),
        )
        .unwrap();
        let mut second = Store::open(other.path(), 8).unwrap();
        AccountPreferences::set_hidden(&mut first, &DEFAULT_ACCOUNT_ID, true).unwrap();
        AccountPreferences::set_hidden(&mut second, &[1; 16], true).unwrap();
        let preferences = merged_preferences(dir.path(), other.path());
        assert_eq!(preferences.main_id(), Some(DEFAULT_ACCOUNT_ID.as_slice()));
        assert!(!preferences.is_hidden(&DEFAULT_ACCOUNT_ID));
        assert!(preferences.is_hidden(&[1; 16]));
        let merged = rdx_rs::merge(&document(dir.path()), &document(other.path()));
        std::fs::write(dir.path().join(LEDGER_FILE), rdx_rs::encode(&merged)).unwrap();
        let mut first = Store::open(dir.path(), 7).unwrap();
        AccountPreferences::set_hidden(&mut first, &[1; 16], false).unwrap();
        let preferences = merged_preferences(dir.path(), other.path());
        assert!(!preferences.is_hidden(&[1; 16]));
        assert!(preferences.is_hidden(&DEFAULT_ACCOUNT_ID));
        assert_eq!(preferences.main_id(), Some([1; 16].as_slice()));
        assert!(AccountPreferences::set_hidden(&mut first, &[1; 16], true).is_err());
    }

    #[test]
    fn choosing_a_hidden_main_restores_it_and_old_documents_have_a_default() {
        let (_dir, mut store, accounts) = setup();
        assert_eq!(
            AccountPreferences::read(&store, &accounts).main_id(),
            Some(DEFAULT_ACCOUNT_ID.as_slice())
        );
        AccountPreferences::set_hidden(&mut store, &[1; 16], true).unwrap();
        AccountPreferences::set_main(&mut store, &[1; 16]).unwrap();
        let preferences = AccountPreferences::read(&store, &accounts);
        assert_eq!(preferences.main_id(), Some([1; 16].as_slice()));
        assert!(!preferences.is_hidden(&[1; 16]));
    }
}
