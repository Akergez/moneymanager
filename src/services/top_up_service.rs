use chrono::NaiveDate;
use uuid::Uuid;

use crate::models::TopUp;
use crate::store::{
    Store, TOP_UPS_IDX, child_f64, child_opt_str, child_str, hex_decode, hex_encode, opt_str_value,
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
    ) -> Result<TopUp, String> {
        let id = Uuid::new_v4().as_bytes().to_vec();
        Self::create_with_id(store, &id, category_id, amount, comment, date)?;
        Ok(TopUp {
            id,
            category_id: category_id.to_vec(),
            amount,
            comment: comment.map(str::to_string),
            date,
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
    ) -> Result<(), String> {
        store.upsert(TOP_UPS_IDX, &hex_encode(id), fields(category_id, amount, comment, date))
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
            })
            .collect())
    }

    pub fn update(
        store: &mut Store,
        top_up_id: &[u8],
        new_amount: f64,
        new_comment: Option<&str>,
        new_date: NaiveDate,
    ) -> Result<usize, String> {
        let key = hex_encode(top_up_id);
        let category_id = store
            .records(TOP_UPS_IDX)
            .into_iter()
            .find(|rec| child_str(rec, 0) == key)
            .map(|rec| hex_decode(&child_str(rec, 1)))
            .unwrap_or_default();
        store.upsert(TOP_UPS_IDX, &key, fields(&category_id, new_amount, new_comment, new_date))?;
        Ok(1)
    }

    pub fn delete(store: &mut Store, top_up_id: &[u8]) -> Result<usize, String> {
        store.delete(TOP_UPS_IDX, &hex_encode(top_up_id))?;
        Ok(1)
    }
}

fn fields(category_id: &[u8], amount: f64, comment: Option<&str>, date: NaiveDate) -> Vec<RdxValue> {
    vec![
        RdxValue::Str(hex_encode(category_id)),
        RdxValue::Float(amount),
        opt_str_value(comment),
        RdxValue::Str(date.format(DATE_FMT).to_string()),
    ]
}

fn parse_date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, DATE_FMT)
        .unwrap_or_else(|_| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
}
