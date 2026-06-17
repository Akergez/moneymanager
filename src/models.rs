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
}
