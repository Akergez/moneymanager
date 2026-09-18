use chrono::NaiveDate;
use uuid::Uuid;

use crate::models::TopUp;
use crate::store::{
    DEFAULT_ACCOUNT_ID, TOP_UPS_IDX, Store, child_account_id, child_f64, child_opt_str, child_str,
    hex_decode, hex_encode, opt_str_value,
};
use rdx_rs::RdxValue;

const DATE_FMT: &str = "%Y-%m-%d";

impl TopUp {
    pub fn create(
        store: &mut Store,
        category_id: &[u8],
        amount: f64,
        comment: Option<&str>,
        date: NaiveDate,
        account_id: &[u8],
    ) -> Result<TopUp, String> {
        let id = Uuid::new_v4().as_bytes().to_vec();
        let account_id = if account_id.is_empty() {
            DEFAULT_ACCOUNT_ID.to_vec()
        } else {
            account_id.to_vec()
        };
        Self::create_with_id(store, &id, category_id, amount, comment, date, &account_id)?;
        Ok(TopUp {
            id,
            category_id: category_id.to_vec(),
            amount,
            comment: comment.map(str::to_string),
            date,
            account_id,
            transfer: None,
        })
    }

    /// Insert (or overwrite) a top-up with a caller-supplied id. Used by CSV
    /// import to preserve the original identifiers.
    pub fn create_with_id(
        store: &mut Store,
        id: &[u8],
        category_id: &[u8],
        amount: f64,
        comment: Option<&str>,
        date: NaiveDate,
        account_id: &[u8],
    ) -> Result<(), String> {
        store.upsert(
            TOP_UPS_IDX,
            &hex_encode(id),
            fields(category_id, amount, comment, date, account_id),
        )
    }

    pub fn read_all(store: &Store) -> Result<Vec<TopUp>, String> {
        Ok(store
            .records(TOP_UPS_IDX)
            .into_iter()
            .map(|rec| TopUp {
                id: hex_decode(&child_str(rec, 0)),
                category_id: hex_decode(&child_str(rec, 1)),
                amount: child_f64(rec, 2),
                comment: child_opt_str(rec, 3),
                date: parse_date(&child_str(rec, 4)),
                account_id: child_account_id(rec, 5),
                transfer: None,
            })
            .collect())
    }

    // CRUD completeness: editing isn't wired into the TUI yet.
    #[allow(dead_code)]
    pub fn update(
        store: &mut Store,
        top_up_id: &[u8],
        new_amount: f64,
        new_comment: Option<&str>,
        new_date: NaiveDate,
    ) -> Result<usize, String> {
        let key = hex_encode(top_up_id);
        let (category_id, account_id) = store
            .records(TOP_UPS_IDX)
            .into_iter()
            .find(|rec| child_str(rec, 0) == key)
            .map(|rec| (hex_decode(&child_str(rec, 1)), child_account_id(rec, 5)))
            .unwrap_or((Vec::new(), DEFAULT_ACCOUNT_ID.to_vec()));
        store.upsert(
            TOP_UPS_IDX,
            &key,
            fields(&category_id, new_amount, new_comment, new_date, &account_id),
        )?;
        Ok(1)
    }

    pub fn delete(store: &mut Store, top_up_id: &[u8]) -> Result<usize, String> {
        store.delete(TOP_UPS_IDX, &hex_encode(top_up_id))?;
        Ok(1)
    }
}

fn fields(
    category_id: &[u8],
    amount: f64,
    comment: Option<&str>,
    date: NaiveDate,
    account_id: &[u8],
) -> Vec<RdxValue> {
    vec![
        RdxValue::Str(hex_encode(category_id)),
        RdxValue::Float(amount),
        opt_str_value(comment),
        RdxValue::Str(date.format(DATE_FMT).to_string()),
        RdxValue::Str(hex_encode(account_id)),
    ]
}

fn parse_date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, DATE_FMT)
        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
}
