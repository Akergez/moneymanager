use uuid::Uuid;

use crate::models::TopUpCategory;
use crate::store::{Store, TOP_UP_CATEGORIES_IDX, child_str, hex_decode, hex_encode};
use rdx_rs::RdxValue;

impl TopUpCategory {
    pub fn create(store: &mut Store, name: &str) -> Result<TopUpCategory, String> {
        let id = Uuid::new_v4().as_bytes().to_vec();
        Self::create_with_id(store, &id, name)?;
        Ok(TopUpCategory {
            id,
            name: name.to_string(),
        })
    }

    /// Insert (or overwrite) a top-up category with a caller-supplied id. Used by
    /// CSV import to preserve the original identifiers.
    pub fn create_with_id(store: &mut Store, id: &[u8], name: &str) -> Result<(), String> {
        store.upsert(
            TOP_UP_CATEGORIES_IDX,
            &hex_encode(id),
            vec![RdxValue::Str(name.to_string())],
        )
    }

    pub fn read_all(store: &Store) -> Result<Vec<TopUpCategory>, String> {
        Ok(store
            .records(TOP_UP_CATEGORIES_IDX)
            .into_iter()
            .map(|rec| TopUpCategory {
                id: hex_decode(&child_str(rec, 0)),
                name: child_str(rec, 1),
            })
            .collect())
    }

    pub fn update(store: &mut Store, category_id: &[u8], new_name: &str) -> Result<usize, String> {
        store.upsert(
            TOP_UP_CATEGORIES_IDX,
            &hex_encode(category_id),
            vec![RdxValue::Str(new_name.to_string())],
        )?;
        Ok(1)
    }

    pub fn delete(store: &mut Store, category_id: &[u8]) -> Result<usize, String> {
        store.delete(TOP_UP_CATEGORIES_IDX, &hex_encode(category_id))?;
        Ok(1)
    }
}
