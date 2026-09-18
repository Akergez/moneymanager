use uuid::Uuid;

use crate::models::Account;
use crate::store::{ACCOUNTS_IDX, DEFAULT_ACCOUNT_ID, Store, child_f64, child_str, hex_decode, hex_encode};
use rdx_rs::RdxValue;

impl Account {
    pub fn create(store: &mut Store, name: &str, currency: &str, opening_balance: f64) -> Result<Account, String> {
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

    // Deleting accounts is out of scope until there is a cascade policy for
    // their expenses/top-ups/transfers; kept for CRUD completeness.
    #[allow(dead_code)]
    pub fn delete(store: &mut Store, account_id: &[u8]) -> Result<usize, String> {
        store.delete(ACCOUNTS_IDX, &hex_encode(account_id))?;
        Ok(1)
    }
}
