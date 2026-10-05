use chrono::NaiveDate;
use uuid::Uuid;

use crate::models::Transfer;
use crate::store::{
    Store, TRANSFERS_IDX, child_f64, child_opt_str, child_str, hex_decode, hex_encode,
    opt_str_value,
};
use rdx_rs::RdxValue;

const DATE_FMT: &str = "%Y-%m-%d";

impl Transfer {
    /// Check the transfer invariants: the accounts differ and both amounts are
    /// finite and positive.
    pub fn validate(
        from_account_id: &[u8],
        to_account_id: &[u8],
        amount_from: f64,
        amount_to: f64,
    ) -> Result<(), String> {
        if from_account_id == to_account_id {
            return Err("transfer must be between two different accounts".to_string());
        }
        if !(amount_from.is_finite() && amount_from > 0.0) {
            return Err("amount_from must be greater than 0".to_string());
        }
        if !(amount_to.is_finite() && amount_to > 0.0) {
            return Err("amount_to must be greater than 0".to_string());
        }
        Ok(())
    }

    /// Create a transfer between two accounts (see [`Transfer::validate`]).
    pub fn create(
        store: &mut Store,
        from_account_id: &[u8],
        to_account_id: &[u8],
        amount_from: f64,
        amount_to: f64,
        comment: Option<&str>,
        date: NaiveDate,
    ) -> Result<Transfer, String> {
        Self::validate(from_account_id, to_account_id, amount_from, amount_to)?;
        let id = Uuid::new_v4().as_bytes().to_vec();
        Self::create_with_id(
            store,
            &id,
            from_account_id,
            to_account_id,
            amount_from,
            amount_to,
            comment,
            date,
        )?;
        Ok(Transfer {
            id,
            from_account_id: from_account_id.to_vec(),
            to_account_id: to_account_id.to_vec(),
            amount_from,
            amount_to,
            comment: comment.map(str::to_string),
            date,
        })
    }

    /// Insert (or overwrite) a transfer with a caller-supplied id.
    #[allow(clippy::too_many_arguments)]
    pub fn create_with_id(
        store: &mut Store,
        id: &[u8],
        from_account_id: &[u8],
        to_account_id: &[u8],
        amount_from: f64,
        amount_to: f64,
        comment: Option<&str>,
        date: NaiveDate,
    ) -> Result<(), String> {
        store.upsert(
            TRANSFERS_IDX,
            &hex_encode(id),
            fields(
                from_account_id,
                to_account_id,
                amount_from,
                amount_to,
                comment,
                date,
            ),
        )
    }

    pub fn read_all(store: &Store) -> Result<Vec<Transfer>, String> {
        Ok(store
            .records(TRANSFERS_IDX)
            .into_iter()
            .map(|rec| Transfer {
                id: hex_decode(&child_str(rec, 0)),
                from_account_id: hex_decode(&child_str(rec, 1)),
                to_account_id: hex_decode(&child_str(rec, 2)),
                amount_from: child_f64(rec, 3),
                amount_to: child_f64(rec, 4),
                comment: child_opt_str(rec, 5),
                date: parse_date(&child_str(rec, 6)),
            })
            .collect())
    }

    // CRUD completeness: editing isn't wired into the TUI yet.
    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn update(
        store: &mut Store,
        transfer_id: &[u8],
        from_account_id: &[u8],
        to_account_id: &[u8],
        new_amount_from: f64,
        new_amount_to: f64,
        new_comment: Option<&str>,
        new_date: NaiveDate,
    ) -> Result<usize, String> {
        store.upsert(
            TRANSFERS_IDX,
            &hex_encode(transfer_id),
            fields(
                from_account_id,
                to_account_id,
                new_amount_from,
                new_amount_to,
                new_comment,
                new_date,
            ),
        )?;
        Ok(1)
    }

    /// Tombstone the transfer; both derived legs disappear with it.
    pub fn delete(store: &mut Store, transfer_id: &[u8]) -> Result<usize, String> {
        store.delete(TRANSFERS_IDX, &hex_encode(transfer_id))?;
        Ok(1)
    }
}

fn fields(
    from_account_id: &[u8],
    to_account_id: &[u8],
    amount_from: f64,
    amount_to: f64,
    comment: Option<&str>,
    date: NaiveDate,
) -> Vec<RdxValue> {
    vec![
        RdxValue::Str(hex_encode(from_account_id)),
        RdxValue::Str(hex_encode(to_account_id)),
        RdxValue::Float(amount_from),
        RdxValue::Float(amount_to),
        opt_str_value(comment),
        RdxValue::Str(date.format(DATE_FMT).to_string()),
    ]
}

fn parse_date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, DATE_FMT)
        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
}
