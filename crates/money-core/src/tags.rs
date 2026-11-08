//! Tags: labels of a person's own making, any number of them on a record.
//!
//! A tag is drawn like a category — a name, a colour and an icon — but where
//! a record is in exactly one category, it may carry several tags or none.
//! Expenses and income each have tags of their own, as they have categories
//! of their own; a transfer has none.
//!
//! Four collections hold them, after the seven that were there before (see
//! [`crate::store`]): the tags of each kind, and the links of each kind. An
//! earlier version merges a document positionally and keeps collections it
//! does not know, so the tags travel through it untouched; and it never
//! writes them, so nothing it does can take one away.
//!
//! A link is a record of its own, keyed `<record hex>:<tag hex>`, rather than
//! a field of the expense: an earlier version rewrites an expense whole, five
//! fields, and would drop a field it does not know of; and two devices
//! tagging the same record at once both keep their tag. Untagging is the
//! link's tombstone. A link whose record or tag is gone is never shown.

use std::collections::HashMap;

use rdx_rs::RdxValue;
use uuid::Uuid;

use crate::models::{CategoryState, CategoryStyle};
use crate::store::{
    EXPENSE_TAG_LINKS_IDX, EXPENSE_TAGS_IDX, Store, TOP_UP_TAG_LINKS_IDX, TOP_UP_TAGS_IDX,
    category_fields, child_category_state, child_category_style, child_str, hex_decode, hex_encode,
};

/// Which records a tag is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TagKind {
    Expense,
    TopUp,
}

impl TagKind {
    fn tags(self) -> usize {
        match self {
            TagKind::Expense => EXPENSE_TAGS_IDX,
            TagKind::TopUp => TOP_UP_TAGS_IDX,
        }
    }

    fn links(self) -> usize {
        match self {
            TagKind::Expense => EXPENSE_TAG_LINKS_IDX,
            TagKind::TopUp => TOP_UP_TAG_LINKS_IDX,
        }
    }
}

/// A tag. Its record is laid out as a category's is: the name, then the
/// optional colour and icon, then the state. A removed tag is not shown on
/// the records it is on; a merged one is shown as the tag it went into.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tag {
    pub id: Vec<u8>,
    pub name: String,
    pub style: CategoryStyle,
    pub state: CategoryState,
}

/// What a link record says: that it is on.
const LINKED: &str = "true";

fn link_key(record_id: &[u8], tag_id: &[u8]) -> String {
    format!("{}:{}", hex_encode(record_id), hex_encode(tag_id))
}

impl Tag {
    /// A new tag with its colour and icon.
    pub fn create(
        store: &mut Store,
        kind: TagKind,
        name: &str,
        style: &CategoryStyle,
    ) -> Result<Tag, String> {
        let id = Uuid::new_v4().as_bytes().to_vec();
        Self::create_with_id(store, kind, &id, name, style, &CategoryState::Visible)?;
        Ok(Tag {
            id,
            name: name.to_string(),
            style: style.clone(),
            state: CategoryState::Visible,
        })
    }

    /// Inserts or rewrites a tag, whole: its style and state go with it.
    pub fn create_with_id(
        store: &mut Store,
        kind: TagKind,
        id: &[u8],
        name: &str,
        style: &CategoryStyle,
        state: &CategoryState,
    ) -> Result<(), String> {
        store.upsert(
            kind.tags(),
            &hex_encode(id),
            category_fields(name, style, state),
        )
    }

    pub fn read_all(store: &Store, kind: TagKind) -> Vec<Tag> {
        store
            .records(kind.tags())
            .into_iter()
            .map(|rec| Tag {
                id: hex_decode(&child_str(rec, 0)),
                name: child_str(rec, 1),
                style: child_category_style(rec, 2),
                state: child_category_state(rec, 4),
            })
            .collect()
    }

    /// The tags on each record of `kind` as the interface shows them: a
    /// merged tag as the one it went into, once, and a removed one not at
    /// all.
    pub fn shown_links(store: &Store, kind: TagKind) -> HashMap<Vec<u8>, Vec<Vec<u8>>> {
        let redirects = crate::models::Redirects::new(
            Self::read_all(store, kind)
                .into_iter()
                .map(|tag| (tag.id, tag.state)),
        );
        let mut links = Self::links(store, kind);
        for tags in links.values_mut() {
            let mut shown: Vec<Vec<u8>> = Vec::new();
            for tag in tags.iter() {
                let to = redirects.resolve(tag);
                if redirects.is_live(&to) && !shown.contains(&to) {
                    shown.push(to);
                }
            }
            *tags = shown;
        }
        links.retain(|_, tags| !tags.is_empty());
        links
    }

    /// Deletes a tag and takes it off every record it was on.
    pub fn delete(store: &mut Store, kind: TagKind, id: &[u8]) -> Result<(), String> {
        let suffix = format!(":{}", hex_encode(id));
        let links: Vec<String> = store
            .records(kind.links())
            .into_iter()
            .map(|rec| child_str(rec, 0))
            .filter(|key| key.ends_with(&suffix))
            .collect();
        for key in links {
            store.delete(kind.links(), &key)?;
        }
        store.delete(kind.tags(), &hex_encode(id))
    }

    /// The tags on each record of `kind`, by record id, in no set order.
    /// Links to tags that are gone are left out.
    pub fn links(store: &Store, kind: TagKind) -> HashMap<Vec<u8>, Vec<Vec<u8>>> {
        let known: std::collections::HashSet<Vec<u8>> = Self::read_all(store, kind)
            .into_iter()
            .map(|tag| tag.id)
            .collect();
        let mut links: HashMap<Vec<u8>, Vec<Vec<u8>>> = HashMap::new();
        for rec in store.records(kind.links()) {
            if child_str(rec, 1) != LINKED {
                continue;
            }
            let key = child_str(rec, 0);
            let Some((record, tag)) = key.split_once(':') else {
                continue;
            };
            let tag = hex_decode(tag);
            if known.contains(&tag) {
                links.entry(hex_decode(record)).or_default().push(tag);
            }
        }
        links
    }

    /// Makes `tags` the tags on `record_id`: links that are wanted and not
    /// there are written, links there and not wanted are deleted, and the
    /// ones that stay are not touched, so another device's change to them
    /// is not outranked by a stamp that changes nothing.
    pub fn set_for_record(
        store: &mut Store,
        kind: TagKind,
        record_id: &[u8],
        tags: &[Vec<u8>],
    ) -> Result<(), String> {
        let prefix = format!("{}:", hex_encode(record_id));
        let present: Vec<String> = store
            .records(kind.links())
            .into_iter()
            .filter(|rec| child_str(rec, 1) == LINKED)
            .map(|rec| child_str(rec, 0))
            .filter(|key| key.starts_with(&prefix))
            .collect();
        let wanted: Vec<String> = tags.iter().map(|tag| link_key(record_id, tag)).collect();
        for key in &present {
            if !wanted.contains(key) {
                store.delete(kind.links(), key)?;
            }
        }
        for key in &wanted {
            if !present.contains(key) {
                store.upsert(kind.links(), key, vec![RdxValue::Str(LINKED.into())])?;
            }
        }
        Ok(())
    }

    /// The link records that put `tags` on `record_id`, for an import to
    /// write in one go with the record.
    pub(crate) fn link_records(
        kind: TagKind,
        record_id: &[u8],
        tags: &[Vec<u8>],
    ) -> Vec<(usize, String, Vec<RdxValue>)> {
        tags.iter()
            .map(|tag| {
                (
                    kind.links(),
                    link_key(record_id, tag),
                    vec![RdxValue::Str(LINKED.into())],
                )
            })
            .collect()
    }

    /// The record of a new tag, for an import.
    pub(crate) fn record(kind: TagKind, id: &[u8], name: &str) -> (usize, String, Vec<RdxValue>) {
        (
            kind.tags(),
            hex_encode(id),
            category_fields(name, &CategoryStyle::default(), &CategoryState::Visible),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Category, Expense};
    use chrono::NaiveDate;

    fn sorted(mut tags: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
        tags.sort();
        tags
    }

    #[test]
    fn a_record_carries_several_tags_and_loses_only_those_taken_off() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path(), 7).unwrap();
        let style = CategoryStyle {
            color: Some("#4c8df6".into()),
            icon: Some("plane".into()),
        };
        let trip = Tag::create(&mut store, TagKind::Expense, "Trip", &style).unwrap();
        let work = Tag::create(&mut store, TagKind::Expense, "Work", &Default::default()).unwrap();
        let food = Category::create(&mut store, "Food").unwrap();
        let day = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();
        let lunch = Expense::create(&mut store, &food.id, 12.0, None, day, &[]).unwrap();

        Tag::set_for_record(
            &mut store,
            TagKind::Expense,
            &lunch.id,
            &[trip.id.clone(), work.id.clone()],
        )
        .unwrap();
        let links = Tag::links(&store, TagKind::Expense);
        assert_eq!(
            sorted(links[&lunch.id].clone()),
            sorted(vec![trip.id.clone(), work.id.clone()])
        );
        // Of the other kind, nothing.
        assert!(Tag::links(&store, TagKind::TopUp).is_empty());

        Tag::set_for_record(
            &mut store,
            TagKind::Expense,
            &lunch.id,
            std::slice::from_ref(&work.id),
        )
        .unwrap();
        assert_eq!(
            Tag::links(&store, TagKind::Expense)[&lunch.id],
            vec![work.id.clone()]
        );

        // Put back after being taken off: the tombstone is not inherited.
        Tag::set_for_record(
            &mut store,
            TagKind::Expense,
            &lunch.id,
            &[work.id.clone(), trip.id.clone()],
        )
        .unwrap();
        assert_eq!(Tag::links(&store, TagKind::Expense)[&lunch.id].len(), 2);

        // A deleted tag is off every record.
        Tag::delete(&mut store, TagKind::Expense, &trip.id).unwrap();
        assert_eq!(
            Tag::links(&store, TagKind::Expense)[&lunch.id],
            vec![work.id.clone()]
        );
        let tags = Tag::read_all(&store, TagKind::Expense);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "Work");

        // And it all comes back from the disk.
        let reopened = Store::open(dir.path(), 7).unwrap();
        assert_eq!(
            Tag::links(&reopened, TagKind::Expense)[&lunch.id],
            vec![work.id]
        );
    }

    #[test]
    fn a_merged_tag_shows_as_its_target_once_and_a_removed_one_not_at_all() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path(), 7).unwrap();
        let kind = TagKind::Expense;
        let style = CategoryStyle::default();
        let trip = Tag::create(&mut store, kind, "Trip", &style).unwrap();
        let travel = Tag::create(&mut store, kind, "Travel", &style).unwrap();
        let work = Tag::create(&mut store, kind, "Work", &style).unwrap();
        let (one, two) = (vec![1u8], vec![2u8]);
        Tag::set_for_record(
            &mut store,
            kind,
            &one,
            &[trip.id.clone(), travel.id.clone()],
        )
        .unwrap();
        Tag::set_for_record(&mut store, kind, &two, std::slice::from_ref(&work.id)).unwrap();

        let merged = CategoryState::Merged(travel.id.clone());
        Tag::create_with_id(&mut store, kind, &trip.id, "Trip", &style, &merged).unwrap();
        Tag::create_with_id(
            &mut store,
            kind,
            &work.id,
            "Work",
            &style,
            &CategoryState::Removed,
        )
        .unwrap();

        let shown = Tag::shown_links(&store, kind);
        assert_eq!(shown[&one], vec![travel.id.clone()]);
        assert!(!shown.contains_key(&two));
        // The links themselves are as they were.
        assert_eq!(Tag::links(&store, kind)[&one].len(), 2);
        assert_eq!(Tag::read_all(&store, kind).len(), 3);
    }

    #[test]
    fn a_tag_keeps_its_style() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path(), 7).unwrap();
        let style = CategoryStyle {
            color: Some("#e0569a".into()),
            icon: Some("gift".into()),
        };
        let tag = Tag::create(&mut store, TagKind::TopUp, "Bonus", &style).unwrap();
        assert_eq!(Tag::read_all(&store, TagKind::TopUp), vec![tag]);
        assert!(Tag::read_all(&store, TagKind::Expense).is_empty());
    }
}
