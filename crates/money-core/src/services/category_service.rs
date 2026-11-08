use uuid::Uuid;

use crate::models::{Category, CategoryState, CategoryStyle};
use crate::store::{
    CATEGORIES_IDX, Store, category_fields, child_category_state, child_category_style, child_str,
    hex_decode, hex_encode,
};

impl Category {
    pub fn create(store: &mut Store, name: &str) -> Result<Category, String> {
        Self::create_styled(store, name, &CategoryStyle::default())
    }

    /// A new category with its colour and icon.
    pub fn create_styled(
        store: &mut Store,
        name: &str,
        style: &CategoryStyle,
    ) -> Result<Category, String> {
        let id = Uuid::new_v4().as_bytes().to_vec();
        Self::create_with_id(store, &id, name, style, &CategoryState::Visible)?;
        Ok(Category {
            id,
            name: name.to_string(),
            style: style.clone(),
            state: CategoryState::Visible,
        })
    }

    /// Insert (or overwrite) a category with a caller-supplied id. The record
    /// is written whole: its style and state go with it.
    pub fn create_with_id(
        store: &mut Store,
        id: &[u8],
        name: &str,
        style: &CategoryStyle,
        state: &CategoryState,
    ) -> Result<(), String> {
        store.upsert(
            CATEGORIES_IDX,
            &hex_encode(id),
            category_fields(name, style, state),
        )
    }

    pub fn read_all(store: &Store) -> Result<Vec<Category>, String> {
        Ok(store
            .records(CATEGORIES_IDX)
            .into_iter()
            .map(|rec| Category {
                id: hex_decode(&child_str(rec, 0)),
                name: child_str(rec, 1),
                style: child_category_style(rec, 2),
                state: child_category_state(rec, 4),
            })
            .collect())
    }

    #[allow(dead_code)]
    pub fn delete(store: &mut Store, category_id: &[u8]) -> Result<usize, String> {
        store.delete(CATEGORIES_IDX, &hex_encode(category_id))?;
        Ok(1)
    }
}
