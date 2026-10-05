use uuid::Uuid;

use crate::models::{CategoryStyle, TopUpCategory};
use crate::store::{
    Store, TOP_UP_CATEGORIES_IDX, category_fields, child_category_style, child_str, hex_decode,
    hex_encode,
};

impl TopUpCategory {
    pub fn create(store: &mut Store, name: &str) -> Result<TopUpCategory, String> {
        Self::create_styled(store, name, &CategoryStyle::default())
    }

    /// A new top-up category with its colour and icon.
    pub fn create_styled(
        store: &mut Store,
        name: &str,
        style: &CategoryStyle,
    ) -> Result<TopUpCategory, String> {
        let id = Uuid::new_v4().as_bytes().to_vec();
        Self::create_with_id(store, &id, name, style)?;
        Ok(TopUpCategory {
            id,
            name: name.to_string(),
            style: style.clone(),
        })
    }

    /// Insert (or overwrite) a top-up category with a caller-supplied id. The
    /// record is written whole: a style left out here is a style removed.
    pub fn create_with_id(
        store: &mut Store,
        id: &[u8],
        name: &str,
        style: &CategoryStyle,
    ) -> Result<(), String> {
        store.upsert(
            TOP_UP_CATEGORIES_IDX,
            &hex_encode(id),
            category_fields(name, style),
        )
    }

    pub fn read_all(store: &Store) -> Result<Vec<TopUpCategory>, String> {
        Ok(store
            .records(TOP_UP_CATEGORIES_IDX)
            .into_iter()
            .map(|rec| TopUpCategory {
                id: hex_decode(&child_str(rec, 0)),
                name: child_str(rec, 1),
                style: child_category_style(rec, 2),
            })
            .collect())
    }

    #[allow(dead_code)]
    pub fn delete(store: &mut Store, category_id: &[u8]) -> Result<usize, String> {
        store.delete(TOP_UP_CATEGORIES_IDX, &hex_encode(category_id))?;
        Ok(1)
    }
}
