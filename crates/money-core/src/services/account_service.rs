use uuid::Uuid;

use crate::models::{Account, Expense, TopUp, Transfer};
use crate::store::{
    ACCOUNTS_IDX, DEFAULT_ACCOUNT_ID, Store, child_f64, child_str, hex_decode, hex_encode,
};
use rdx_rs::RdxValue;

impl Account {
    pub fn create(
        store: &mut Store,
        name: &str,
        currency: &str,
        opening_balance: f64,
    ) -> Result<Account, String> {
        let id = Uuid::new_v4().as_bytes().to_vec();
        Self::create_with_id(store, &id, name, currency, opening_balance)?;
        Ok(Account {
            id,
            name: name.to_string(),
            currency: currency.to_string(),
            opening_balance,
        })
    }

    /// Insert (or overwrite) an account with a caller-supplied id.
    pub fn create_with_id(
        store: &mut Store,
        id: &[u8],
        name: &str,
        currency: &str,
        opening_balance: f64,
    ) -> Result<(), String> {
        store.upsert(
            ACCOUNTS_IDX,
            &hex_encode(id),
            vec![
                RdxValue::Str(name.to_string()),
                RdxValue::Str(currency.to_string()),
                RdxValue::Float(opening_balance),
            ],
        )
    }

    pub fn read_all(store: &Store) -> Result<Vec<Account>, String> {
        Ok(store
            .records(ACCOUNTS_IDX)
            .into_iter()
            .map(|rec| Account {
                id: hex_decode(&child_str(rec, 0)),
                name: child_str(rec, 1),
                currency: child_str(rec, 2),
                opening_balance: child_f64(rec, 3),
            })
            .collect())
    }

    /// Create the default account (fixed id, "Основной") if it does not exist
    /// yet. Idempotent, so concurrent first runs merge to a single record on
    /// sync.
    pub fn ensure_default(store: &mut Store, currency: &str) -> Result<(), String> {
        let key = hex_encode(&DEFAULT_ACCOUNT_ID);
        if store
            .records(ACCOUNTS_IDX)
            .iter()
            .any(|rec| child_str(rec, 0) == key)
        {
            return Ok(());
        }
        Self::create_with_id(store, &DEFAULT_ACCOUNT_ID, "Основной", currency, 0.0)
    }

    pub fn update(
        store: &mut Store,
        account_id: &[u8],
        new_name: &str,
        new_currency: &str,
        new_opening_balance: f64,
    ) -> Result<usize, String> {
        store.upsert(
            ACCOUNTS_IDX,
            &hex_encode(account_id),
            vec![
                RdxValue::Str(new_name.to_string()),
                RdxValue::Str(new_currency.to_string()),
                RdxValue::Float(new_opening_balance),
            ],
        )?;
        Ok(1)
    }

    /// Set the account's balance as it stands: the amount it holds now, in the
    /// same breath as any other account edit.
    ///
    /// The ledger keeps an opening balance and records, never a running total,
    /// so this moves the opening balance by the difference the records have
    /// not accounted for and rewrites nothing else. Every record stays as it
    /// is, and the balance that comes out is `balance` to the cent.
    pub fn set_balance(store: &mut Store, account_id: &[u8], balance: f64) -> Result<(), String> {
        if !balance.is_finite() {
            return Err("The balance is not a number.".to_string());
        }
        let accounts = Self::read_all(store)?;
        let Some(account) = accounts.iter().find(|a| a.id == account_id) else {
            return Err("Account no longer exists.".to_string());
        };
        let now = crate::ledger::summaries(
            &accounts,
            &Expense::read_all(store)?,
            &TopUp::read_all(store)?,
            &Transfer::read_all(store)?,
        )
        .get(account_id)
        .map_or(account.opening_balance, |summary| summary.balance);
        let opening = account.opening_balance + (balance - now);
        // Money is kept to the cent, as it is shown: a typed amount is what the
        // account has, not a figure that differs from it in the last decimals.
        let opening = (opening * 100.0).round() / 100.0;
        Self::create_with_id(store, account_id, &account.name, &account.currency, opening)
    }

    // Deleting accounts is out of scope until there is a cascade policy for
    // their expenses/top-ups/transfers; kept for CRUD completeness.
    #[allow(dead_code)]
    pub fn delete(store: &mut Store, account_id: &[u8]) -> Result<usize, String> {
        store.delete(ACCOUNTS_IDX, &hex_encode(account_id))?;
        Ok(1)
    }
}
