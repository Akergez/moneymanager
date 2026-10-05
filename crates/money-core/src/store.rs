//! RDX-backed persistence: replaces the former SQLite/diesel layer.
//!
//! The dataset is a CRDT document — a positional [`Tuple`] of six [`Eulerian`]
//! collections (categories, expenses, top-up categories, top-ups, accounts,
//! transfers, in the order of the `*_IDX` constants below). Each record is a
//! `Tuple` whose first child is its hex id (the Eulerian map key); the rest are
//! fields. Per-record stamps give last-write-wins semantics and tombstones mark
//! deletions.
//!
//! Collections are merged positionally with zero-padding, so documents written
//! with fewer collections (legacy, pre-accounts) merge losslessly into this
//! six-collection shape; and a record missing a trailing field reads as empty.
//!
//! **Chunking.** A *chunk is the delta of one sync* — not the whole document
//! (no dedup, re-sends everything) and not one record (object explosion). Local
//! writes accumulate in a small mutable **staging** delta (`staging.rdx`, a
//! 6-collection doc holding only the records changed since the last seal),
//! persisted on every write so a crash loses nothing. On sync the staged delta
//! is *sealed* into a single immutable, content-addressed chunk in a
//! [`FileChunkStore`] (one file per chunk, named by its hash), then exchanged
//! with the remote. Because `merge` is commutative/idempotent, merging the
//! committed chunks plus staging reconstructs the full document. Net effect:
//! O(syncs-with-changes) chunks, content-addressed and idempotent, with
//! incremental, hash-verified transfer.
//!
//! [`Tuple`]: rdx_rs::RdxValue::Tuple
//! [`Eulerian`]: rdx_rs::RdxValue::Eulerian

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rdx_rs::{RdxElement, RdxValue, Stamp};
use rdx_sync::{ChunkStore, SyncReport};
use rdx_sync_fs::FileChunkStore;
use rdx_sync_s3::{S3ChunkStore, S3Config};

use crate::models::CategoryStyle;
use crate::remote::RemoteConfig;

pub const CATEGORIES_IDX: usize = 0;
pub const EXPENSES_IDX: usize = 1;
pub const TOP_UP_CATEGORIES_IDX: usize = 2;
pub const TOP_UPS_IDX: usize = 3;
pub const ACCOUNTS_IDX: usize = 4;
pub const TRANSFERS_IDX: usize = 5;
const COLLECTION_COUNT: usize = 6;

/// Fixed id of the implicit default account. Existing records with no
/// `account_id` belong to it; the id is deterministic so two devices creating
/// it at the same time merge to a single record (LWW).
pub const DEFAULT_ACCOUNT_ID: [u8; 16] = [0; 16];

/// One unit of Lamport time. The low 6 bits of a stamp's `time` are the
/// revision (bit 0 = tombstone), so a full "tick" is `1 << 6`.
const TIME_STEP: u64 = 1 << 6;

/// The persistent dataset plus the local clock/source used to stamp writes.
pub struct Store {
    /// Persistent, content-addressed store of sealed delta chunks.
    chunks: FileChunkStore,
    /// Where the uncommitted staging delta is persisted.
    staging_path: PathBuf,
    /// Uncommitted writes since the last seal: a 6-collection delta document.
    staged: RdxElement,
    /// In-memory full view = `merge(get_completed(chunks), staged)`.
    doc: RdxElement,
    /// This installation's stamp source id (stable, from the config file).
    source: u64,
    /// Monotonic Lamport time; strictly increasing so later local writes win.
    clock: u64,
}

impl Store {
    /// Open the data directory at `dir` (creating it if absent): load the sealed
    /// chunks and any staged-but-unsynced writes, and merge them into the view.
    pub fn open(dir: &Path, source: u64) -> Result<Self, String> {
        let chunks = FileChunkStore::open(dir).map_err(|e| format!("open chunk dir: {e}"))?;
        let staging_path = dir.join("staging.rdx");
        let staged = read_doc(&staging_path)?.unwrap_or_else(empty_doc);
        let committed = chunks
            .get_completed()
            .map_err(|e| format!("merge chunks: {e}"))?
            .unwrap_or_else(empty_doc);
        let doc = rdx_rs::merge(&committed, &staged);
        let clock = max_time(&doc);
        Ok(Self {
            chunks,
            staging_path,
            staged,
            doc,
            source,
            clock,
        })
    }

    /// True if there is nothing stored at all — no sealed chunks and no staged
    /// writes (a fresh install, eligible for legacy migration).
    pub fn is_empty(&self) -> Result<bool, String> {
        let no_chunks = self
            .chunks
            .is_empty()
            .map_err(|e| format!("list chunks: {e}"))?;
        Ok(no_chunks && !has_records(&self.staged))
    }

    /// Seal the staged writes into a single immutable delta chunk. A no-op when
    /// nothing is staged. This is the local "commit" boundary; [`Store::sync`]
    /// calls it before exchanging chunks.
    pub fn seal(&mut self) -> Result<(), String> {
        if has_records(&self.staged) {
            self.chunks
                .put(&self.staged)
                .map_err(|e| format!("store chunk: {e}"))?;
            self.staged = empty_doc();
            // Best-effort: the data now lives in an immutable chunk.
            let _ = std::fs::remove_file(&self.staging_path);
        }
        Ok(())
    }

    /// One-time migration from the legacy single-blob format: decode the blob at
    /// `path` and seal it as one baseline chunk (preserving every stamp and id).
    /// Returns the number of records migrated.
    pub fn import_legacy_blob(&mut self, path: &Path) -> Result<usize, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let (legacy, _) = rdx_rs::decode(&bytes).map_err(|e| format!("decode blob: {e:?}"))?;
        let n = count_records(&legacy);
        self.chunks
            .put(&legacy)
            .map_err(|e| format!("store chunk: {e}"))?;
        let committed = self
            .chunks
            .get_completed()
            .map_err(|e| format!("merge chunks: {e}"))?
            .unwrap_or_else(empty_doc);
        self.doc = rdx_rs::merge(&committed, &self.staged);
        self.clock = self.clock.max(max_time(&self.doc));
        Ok(n)
    }

    /// Next strictly-increasing stamp for a local write.
    fn next_stamp(&mut self) -> Stamp {
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
            << 6;
        let time = wall.max(self.clock + TIME_STEP);
        self.clock = time;
        Stamp::new(self.source, time)
    }

    /// Live (non-tombstoned) record tuples of collection `idx`.
    pub fn records(&self, idx: usize) -> Vec<&RdxElement> {
        self.doc
            .value
            .children()
            .and_then(|cols| cols.get(idx))
            .and_then(|collection| collection.value.children())
            .map(|children| children.iter().filter(|rec| !rec.is_tombstone()).collect())
            .unwrap_or_default()
    }

    /// Insert or overwrite the record keyed by `key` in collection `idx`.
    ///
    /// `fields` are the record's values *after* the key; each one (and the
    /// record itself) is stamped with a fresh local stamp, then merged into the
    /// document so a repeat key updates in place via CRDT merge.
    pub fn upsert(&mut self, idx: usize, key: &str, fields: Vec<RdxValue>) -> Result<(), String> {
        let stamp = self.next_stamp();
        let mut children = Vec::with_capacity(fields.len() + 1);
        children.push(RdxElement::with_stamp(
            RdxValue::Str(key.to_string()),
            stamp,
        ));
        for v in fields {
            children.push(RdxElement::with_stamp(v, stamp));
        }
        let record = RdxElement::with_stamp(RdxValue::Tuple(children), stamp);
        self.merge_record(idx, record)
    }

    /// Tombstone the record keyed by `key` in collection `idx`.
    pub fn delete(&mut self, idx: usize, key: &str) -> Result<(), String> {
        let mut stamp = self.next_stamp();
        stamp.time |= 1; // mark tombstone (odd time)
        let record = RdxElement::with_stamp(
            RdxValue::Tuple(vec![RdxElement::with_stamp(
                RdxValue::Str(key.to_string()),
                stamp,
            )]),
            stamp,
        );
        self.merge_record(idx, record)
    }

    /// Fold `record` into the staged delta and the in-memory view, then persist
    /// staging. The write becomes part of the next sealed chunk, not its own.
    fn merge_record(&mut self, idx: usize, record: RdxElement) -> Result<(), String> {
        let patch = patch_doc(idx, record);
        self.staged = rdx_rs::merge(&self.staged, &patch);
        self.doc = rdx_rs::merge(&self.doc, &patch);
        write_doc(&self.staging_path, &self.staged)
    }

    /// Synchronize with the configured S3 remote: seal pending writes into a
    /// delta chunk, exchange the missing chunks both ways, then rebuild the view.
    pub fn sync(&mut self, remote: &RemoteConfig) -> Result<SyncReport, String> {
        let config = S3Config {
            endpoint: remote.endpoint.clone(),
            region: remote.region.clone(),
            bucket: remote.bucket.clone(),
            access_key_id: remote.access_key_id.clone(),
            secret_access_key: remote.secret_access_key.clone(),
            prefix: remote.prefix.clone(),
        };
        let mut s3 = match &remote.encryption_key {
            Some(key) => S3ChunkStore::with_encryption_key(config, key),
            None => S3ChunkStore::new(config),
        }
        .map_err(|e| format!("open S3 store: {e}"))?;
        self.sync_with(&mut s3)
    }

    /// Synchronize with any chunk store: seal pending writes into a delta
    /// chunk, exchange the missing chunks both ways, then rebuild the view.
    /// [`Store::sync`] is this against S3; it is separate so the exchange can
    /// be exercised without a network.
    pub fn sync_with<R: ChunkStore>(&mut self, remote: &mut R) -> Result<SyncReport, String> {
        self.seal()?;

        // The local store persists every chunk we hold, so this transfers only
        // chunks one side is missing; each fetched chunk is hash-verified.
        let report = rdx_sync::sync(&mut self.chunks, remote).map_err(|e| format!("sync: {e}"))?;

        self.doc = self
            .chunks
            .get_completed()
            .map_err(|e| format!("merge chunks: {e}"))?
            .unwrap_or_else(empty_doc);
        self.clock = self.clock.max(max_time(&self.doc));
        Ok(report)
    }
}

/// An empty document: a 6-tuple of empty Eulerian collections.
fn empty_doc() -> RdxElement {
    let cols = (0..COLLECTION_COUNT)
        .map(|_| RdxElement::new(RdxValue::Eulerian(Vec::new())))
        .collect();
    RdxElement::new(RdxValue::Tuple(cols))
}

/// A document containing only `record` in collection `idx`, ready to merge.
fn patch_doc(idx: usize, record: RdxElement) -> RdxElement {
    let cols = (0..COLLECTION_COUNT)
        .map(|i| {
            let entries = if i == idx {
                vec![record.clone()]
            } else {
                Vec::new()
            };
            RdxElement::new(RdxValue::Eulerian(entries))
        })
        .collect();
    RdxElement::new(RdxValue::Tuple(cols))
}

/// Largest stamp time anywhere in the tree (seeds the local clock so writes stay
/// strictly increasing after loading a blob).
fn max_time(el: &RdxElement) -> u64 {
    let here = el.stamp.map(|s| s.time).unwrap_or(0);
    let children = el
        .value
        .children()
        .map(|cs| cs.iter().map(max_time).max().unwrap_or(0))
        .unwrap_or(0);
    here.max(children)
}

/// True if any collection of `doc` holds at least one record.
fn has_records(doc: &RdxElement) -> bool {
    doc.value
        .children()
        .map(|cols| {
            cols.iter()
                .any(|c| c.value.children().map(|r| !r.is_empty()).unwrap_or(false))
        })
        .unwrap_or(false)
}

/// Total number of records across all collections of `doc`.
fn count_records(doc: &RdxElement) -> usize {
    doc.value
        .children()
        .map(|cols| {
            cols.iter()
                .map(|c| c.value.children().map(|r| r.len()).unwrap_or(0))
                .sum()
        })
        .unwrap_or(0)
}

/// Read and decode an RDX document from `path`, or `None` if it doesn't exist.
fn read_doc(path: &Path) -> Result<Option<RdxElement>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let (el, _) =
        rdx_rs::decode(&bytes).map_err(|e| format!("decode {}: {e:?}", path.display()))?;
    Ok(Some(el))
}

/// Encode `doc` and write it to `path` atomically (temp file then rename), so a
/// crash mid-write can never leave a half-written staging file.
fn write_doc(path: &Path, doc: &RdxElement) -> Result<(), String> {
    let bytes = rdx_rs::encode(doc);
    let tmp = path.with_extension("rdx.tmp");
    std::fs::write(&tmp, &bytes).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("rename {}: {e}", path.display()))
}

// ---------------------------------------------------------------------------
// Field <-> RDX value helpers, shared by the per-record services.
// ---------------------------------------------------------------------------

pub fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

pub fn hex_decode(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i + 1 < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16);
        let lo = (bytes[i + 1] as char).to_digit(16);
        match (hi, lo) {
            (Some(h), Some(l)) => out.push(((h << 4) | l) as u8),
            _ => break,
        }
        i += 2;
    }
    out
}

/// The string value of a record child (key or text field), or "" if not a string.
pub fn child_str(rec: &RdxElement, pos: usize) -> String {
    rec.value
        .children()
        .and_then(|c| c.get(pos))
        .and_then(|e| e.as_str())
        .unwrap_or("")
        .to_string()
}

/// The account id stored at `pos`, or [`DEFAULT_ACCOUNT_ID`] for legacy records
/// written before accounts existed (field absent → "").
pub fn child_account_id(rec: &RdxElement, pos: usize) -> Vec<u8> {
    let raw = child_str(rec, pos);
    if raw.is_empty() {
        DEFAULT_ACCOUNT_ID.to_vec()
    } else {
        hex_decode(&raw)
    }
}

/// The float value of a record child, or 0.0 if absent/wrong type.
pub fn child_f64(rec: &RdxElement, pos: usize) -> f64 {
    rec.value
        .children()
        .and_then(|c| c.get(pos))
        .and_then(|e| e.as_float())
        .unwrap_or(0.0)
}

/// An optional-string field: encode `None` as a `Term("null")`, `Some` as `Str`.
pub fn opt_str_value(v: Option<&str>) -> RdxValue {
    match v {
        Some(s) => RdxValue::Str(s.to_string()),
        None => RdxValue::Term("null".to_string()),
    }
}

/// Decode an optional-string field stored by [`opt_str_value`].
pub fn child_opt_str(rec: &RdxElement, pos: usize) -> Option<String> {
    let child = rec.value.children().and_then(|c| c.get(pos))?;
    child.as_str().map(|s| s.to_string())
}

/// The fields of a category record after its key: the name, then the colour
/// and the icon. An unstyled category is written as the name alone, exactly
/// as before the two existed; a colour that is absent beside an icon that is
/// not holds its place with a `null`.
pub fn category_fields(name: &str, style: &CategoryStyle) -> Vec<RdxValue> {
    let mut fields = vec![RdxValue::Str(name.to_string())];
    if !style.is_empty() {
        fields.push(opt_str_value(style.color.as_deref()));
        fields.push(opt_str_value(style.icon.as_deref()));
    }
    fields
}

/// The style of a category record, whose colour is at `pos` and icon after
/// it. Fields that are absent, `null` or empty are no style.
pub fn child_category_style(rec: &RdxElement, pos: usize) -> CategoryStyle {
    let field = |pos| child_opt_str(rec, pos).filter(|value| !value.is_empty());
    CategoryStyle {
        color: field(pos),
        icon: field(pos + 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger;
    use crate::models::{Account, Category, Expense, TopUp, TopUpCategory, Transfer};
    use chrono::NaiveDate;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("mm_test_{tag}_{nanos}"))
    }

    #[test]
    fn a_category_style_is_optional_trailing_fields() {
        let dir = temp_dir("category_style");
        let mut store = Store::open(&dir, 7).unwrap();
        let style = |color: Option<&str>, icon: Option<&str>| CategoryStyle {
            color: color.map(str::to_string),
            icon: icon.map(str::to_string),
        };

        // A record as every earlier version wrote it: the key and the name.
        store
            .upsert(CATEGORIES_IDX, "aa", vec![RdxValue::Str("Old".into())])
            .unwrap();
        // An unstyled category is still written exactly that way.
        let plain = Category::create(&mut store, "Plain").unwrap();
        let styled =
            Category::create_styled(&mut store, "Food", &style(Some("#e8853b"), Some("zap")))
                .unwrap();
        // An icon without a colour: the colour's place is held, not skipped.
        let icon_only =
            TopUpCategory::create_styled(&mut store, "Salary", &style(None, Some("wallet")))
                .unwrap();

        let lengths: Vec<usize> = store
            .records(CATEGORIES_IDX)
            .iter()
            .map(|rec| rec.value.children().map_or(0, |children| children.len()))
            .collect();
        assert_eq!(lengths.iter().filter(|len| **len == 2).count(), 2);
        assert_eq!(lengths.iter().filter(|len| **len == 4).count(), 1);

        // Reopened, so this is what the staging file holds.
        let mut store = Store::open(&dir, 7).unwrap();
        let find = |store: &Store, id: &[u8]| {
            Category::read_all(store)
                .unwrap()
                .into_iter()
                .find(|c| c.id == id)
                .unwrap()
        };
        assert_eq!(find(&store, &[0xaa]).name, "Old");
        assert!(find(&store, &[0xaa]).style.is_empty());
        assert!(find(&store, &plain.id).style.is_empty());
        assert_eq!(
            find(&store, &styled.id).style,
            style(Some("#e8853b"), Some("zap"))
        );
        let salary = TopUpCategory::read_all(&store).unwrap();
        assert_eq!(salary[0].id, icon_only.id);
        assert_eq!(salary[0].style, style(None, Some("wallet")));

        // A client that knows only the name reads it where it always was.
        let rec = store
            .records(CATEGORIES_IDX)
            .into_iter()
            .find(|rec| child_str(rec, 0) == hex_encode(&styled.id))
            .unwrap();
        assert_eq!(child_str(rec, 1), "Food");

        // A rewrite replaces the record whole: the colour changes, the icon
        // goes, and a later rewrite without a style leaves none.
        Category::create_with_id(&mut store, &styled.id, "Food", &style(Some("#4c8df6"), None))
            .unwrap();
        assert_eq!(
            find(&store, &styled.id).style,
            style(Some("#4c8df6"), None)
        );
        Category::create_with_id(&mut store, &styled.id, "Groceries", &CategoryStyle::default())
            .unwrap();
        let renamed = find(&store, &styled.id);
        assert_eq!(renamed.name, "Groceries");
        assert!(renamed.style.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_read_roundtrip_and_persistence() {
        let dir = temp_dir("roundtrip");
        let mut store = Store::open(&dir, 7).unwrap();

        let cat = Category::create(&mut store, "Food").unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        Expense::create(
            &mut store,
            &cat.id,
            12.5,
            Some("lunch"),
            date,
            &DEFAULT_ACCOUNT_ID,
        )
        .unwrap();
        Expense::create(&mut store, &cat.id, 3.0, None, date, &DEFAULT_ACCOUNT_ID).unwrap();

        let cats = Category::read_all(&store).unwrap();
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].name, "Food");
        assert_eq!(cats[0].id, cat.id);

        // Reopen the data dir: staged writes were persisted and reload identically.
        let store2 = Store::open(&dir, 7).unwrap();
        let mut exps = Expense::read_all(&store2).unwrap();
        exps.sort_by(|a, b| a.amount.partial_cmp(&b.amount).unwrap());
        assert_eq!(exps.len(), 2);
        assert_eq!(exps[0].amount, 3.0);
        assert_eq!(exps[0].comment, None);
        assert_eq!(exps[1].amount, 12.5);
        assert_eq!(exps[1].comment, Some("lunch".to_string()));
        assert_eq!(exps[1].category_id, cat.id);
        assert_eq!(exps[1].date, date);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn writes_stage_until_sealed_into_one_chunk() {
        let dir = temp_dir("chunks");
        let mut store = Store::open(&dir, 7).unwrap();
        assert!(store.is_empty().unwrap());

        // Writes accumulate in staging — no chunks yet.
        Category::create(&mut store, "A").unwrap();
        Category::create(&mut store, "B").unwrap();
        Category::create(&mut store, "C").unwrap();
        assert_eq!(store.chunks.list().unwrap().len(), 0);
        assert!(store.staging_path.exists());

        // Sealing folds the whole batch into exactly one delta chunk.
        store.seal().unwrap();
        assert_eq!(store.chunks.list().unwrap().len(), 1);
        assert!(!store.staging_path.exists());
        // Sealing again with nothing staged is a no-op.
        store.seal().unwrap();
        assert_eq!(store.chunks.list().unwrap().len(), 1);

        // The sealed data reloads, and a further write stages anew.
        let mut store2 = Store::open(&dir, 7).unwrap();
        assert_eq!(Category::read_all(&store2).unwrap().len(), 3);
        Category::create(&mut store2, "D").unwrap();
        store2.seal().unwrap();
        assert_eq!(store2.chunks.list().unwrap().len(), 2); // one chunk per sealed batch

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn delete_tombstones_record() {
        let dir = temp_dir("delete");
        let mut store = Store::open(&dir, 1).unwrap();
        let a = Category::create(&mut store, "A").unwrap();
        let _b = Category::create(&mut store, "B").unwrap();
        assert_eq!(Category::read_all(&store).unwrap().len(), 2);

        Category::delete(&mut store, &a.id).unwrap();
        let remaining = Category::read_all(&store).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].name, "B");

        // The tombstone survives sealing + reopen.
        store.seal().unwrap();
        let store2 = Store::open(&dir, 1).unwrap();
        assert_eq!(Category::read_all(&store2).unwrap().len(), 1);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn divergent_docs_merge_like_sync() {
        // Two installations (distinct sources) edit independently; merging their
        // documents yields the union — the property S3 sync depends on.
        let da = temp_dir("merge_a");
        let db = temp_dir("merge_b");
        let mut a = Store::open(&da, 0xA).unwrap();
        let mut b = Store::open(&db, 0xB).unwrap();

        Category::create(&mut a, "FromA").unwrap();
        Category::create(&mut b, "FromB").unwrap();

        let merged = rdx_rs::merge(&a.doc, &b.doc);
        let mut names: Vec<String> = merged.value.children().unwrap()[CATEGORIES_IDX]
            .value
            .children()
            .unwrap()
            .iter()
            .filter(|r| !r.is_tombstone())
            .map(|r| child_str(r, 1))
            .collect();
        names.sort();
        assert_eq!(names, vec!["FromA".to_string(), "FromB".to_string()]);

        std::fs::remove_dir_all(&da).ok();
        std::fs::remove_dir_all(&db).ok();
    }

    /// A record tuple `[key, fields...]` with every element stamped `stamp`.
    fn legacy_record(key: &str, fields: Vec<RdxValue>, stamp: Stamp) -> RdxElement {
        let mut children = vec![RdxElement::with_stamp(
            RdxValue::Str(key.to_string()),
            stamp,
        )];
        children.extend(fields.into_iter().map(|v| RdxElement::with_stamp(v, stamp)));
        RdxElement::with_stamp(RdxValue::Tuple(children), stamp)
    }

    #[test]
    fn legacy_four_collection_document_opens_with_default_account() {
        // A pre-accounts document: 4 collections, 4-field expense records.
        let stamp = Stamp::new(0x99, 5 * TIME_STEP);
        let cat = "11".repeat(16);
        let expense = legacy_record(
            &"22".repeat(16),
            vec![
                RdxValue::Str(cat.clone()),
                RdxValue::Float(42.0),
                RdxValue::Term("null".to_string()),
                RdxValue::Str("2024-01-02".to_string()),
            ],
            stamp,
        );
        let cols = (0..4)
            .map(|i| {
                RdxElement::new(RdxValue::Eulerian(if i == EXPENSES_IDX {
                    vec![expense.clone()]
                } else {
                    Vec::new()
                }))
            })
            .collect();
        let legacy = RdxElement::new(RdxValue::Tuple(cols));

        let dir = temp_dir("legacy");
        std::fs::create_dir_all(&dir).unwrap();
        let blob = dir.join("legacy.rdx");
        std::fs::write(&blob, rdx_rs::encode(&legacy)).unwrap();
        let mut store = Store::open(&dir.join("data"), 7).unwrap();
        assert_eq!(store.import_legacy_blob(&blob).unwrap(), 1);

        // New-shape writes merge with the old chunk, and old records read as
        // belonging to the default account.
        Account::ensure_default(&mut store, "RUB").unwrap();
        let exps = Expense::read_all(&store).unwrap();
        assert_eq!(exps.len(), 1);
        assert_eq!(exps[0].amount, 42.0);
        assert_eq!(exps[0].account_id, DEFAULT_ACCOUNT_ID.to_vec());
        assert_eq!(Account::read_all(&store).unwrap().len(), 1);

        // Survives a reopen as well.
        store.seal().unwrap();
        let reopened = Store::open(&dir.join("data"), 7).unwrap();
        assert_eq!(
            Expense::read_all(&reopened).unwrap()[0].account_id,
            DEFAULT_ACCOUNT_ID.to_vec()
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn update_keeps_account_id() {
        // Note: rdx merges two versions of a record field-by-field only when
        // they carry the same stamp; a newer write replaces the whole record.
        // So a pre-accounts client that *overwrites* a record (it can only do
        // so via CSV re-import) resets it to the default account. Current
        // writers therefore always rewrite every field, account_id included.
        let dir = temp_dir("update_account");
        let mut store = Store::open(&dir, 7).unwrap();
        let account = [7u8; 16];
        let date = NaiveDate::from_ymd_opt(2026, 6, 15).unwrap();
        let e = Expense::create(&mut store, &[1u8; 16], 10.0, None, date, &account).unwrap();
        let t = TopUp::create(&mut store, &[1u8; 16], 10.0, None, date, &account).unwrap();

        Expense::update(&mut store, &e.id, 99.0, Some("edited"), date).unwrap();
        TopUp::update(&mut store, &t.id, 98.0, None, date).unwrap();

        let exps = Expense::read_all(&store).unwrap();
        assert_eq!(
            (exps[0].amount, exps[0].account_id.clone()),
            (99.0, account.to_vec())
        );
        assert_eq!(exps[0].category_id, vec![1u8; 16]);
        let tops = TopUp::read_all(&store).unwrap();
        assert_eq!(
            (tops[0].amount, tops[0].account_id.clone()),
            (98.0, account.to_vec())
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ensure_default_is_idempotent_and_merges_to_one_account() {
        let da = temp_dir("default_a");
        let db = temp_dir("default_b");
        let mut a = Store::open(&da, 0xA).unwrap();
        let mut b = Store::open(&db, 0xB).unwrap();

        Account::ensure_default(&mut a, "RUB").unwrap();
        Account::ensure_default(&mut a, "RUB").unwrap();
        assert_eq!(Account::read_all(&a).unwrap().len(), 1);

        // Two devices create the default account independently.
        Account::ensure_default(&mut b, "USD").unwrap();
        let merged = rdx_rs::merge(&a.doc, &b.doc);
        let live: Vec<_> = merged.value.children().unwrap()[ACCOUNTS_IDX]
            .value
            .children()
            .unwrap()
            .iter()
            .filter(|r| !r.is_tombstone())
            .collect();
        assert_eq!(live.len(), 1);
        assert_eq!(child_str(live[0], 0), hex_encode(&DEFAULT_ACCOUNT_ID));

        std::fs::remove_dir_all(&da).ok();
        std::fs::remove_dir_all(&db).ok();
    }

    #[test]
    fn deleting_records_and_transfers() {
        let dir = temp_dir("delete_ledger");
        let mut store = Store::open(&dir, 1).unwrap();
        Account::ensure_default(&mut store, "RUB").unwrap();
        let usd = Account::create(&mut store, "Card USD", "USD", 0.0).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 9, 12).unwrap();
        let keep =
            Expense::create(&mut store, &[1u8; 16], 5.0, None, date, &DEFAULT_ACCOUNT_ID).unwrap();
        let gone =
            Expense::create(&mut store, &[1u8; 16], 7.0, None, date, &DEFAULT_ACCOUNT_ID).unwrap();
        let tr = Transfer::create(
            &mut store,
            &usd.id,
            &DEFAULT_ACCOUNT_ID,
            100.0,
            9235.0,
            None,
            date,
        )
        .unwrap();

        let balances = |s: &Store| {
            let accounts = Account::read_all(s).unwrap();
            let transfers = Transfer::read_all(s).unwrap();
            let sums = ledger::summaries(
                &accounts,
                &Expense::read_all(s).unwrap(),
                &TopUp::read_all(s).unwrap(),
                &transfers,
            );
            let rub = ledger::ledger_for(
                &accounts[0],
                &accounts,
                &Expense::read_all(s).unwrap(),
                &[],
                &transfers,
            );
            (
                sums[&DEFAULT_ACCOUNT_ID.to_vec()].balance,
                sums[&usd.id].balance,
                rub.top_ups.len(),
            )
        };
        assert_eq!(balances(&store), (9235.0 - 12.0, -100.0, 1));

        // Deleting an expense removes only that expense.
        Expense::delete(&mut store, &gone.id).unwrap();
        let left = Expense::read_all(&store).unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, keep.id);

        // Deleting the transfer removes both legs; balances return to where
        // they were without it.
        Transfer::delete(&mut store, &tr.id).unwrap();
        assert_eq!(balances(&store), (-5.0, 0.0, 0));

        // Tombstones survive sealing + reopen.
        store.seal().unwrap();
        let reopened = Store::open(&dir, 1).unwrap();
        assert_eq!(Expense::read_all(&reopened).unwrap().len(), 1);
        assert!(Transfer::read_all(&reopened).unwrap().is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn two_devices_converge_through_a_shared_remote() {
        let (da, db) = (temp_dir("sync_a"), temp_dir("sync_b"));
        let mut remote = rdx_sync::MemChunkStore::new();
        let mut a = Store::open(&da, 0xA).unwrap();
        let mut b = Store::open(&db, 0xB).unwrap();
        Account::ensure_default(&mut a, "RUB").unwrap();
        let food = Category::create(&mut a, "Food").unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 9, 12).unwrap();
        let lunch = Expense::create(
            &mut a,
            &food.id,
            12.5,
            Some("lunch"),
            date,
            &DEFAULT_ACCOUNT_ID,
        )
        .unwrap();

        // A pushes its one sealed chunk; B pulls it.
        let pushed = a.sync_with(&mut remote).unwrap();
        assert_eq!((pushed.pulled.len(), pushed.pushed.len()), (0, 1));
        let pulled = b.sync_with(&mut remote).unwrap();
        assert_eq!((pulled.pulled.len(), pulled.pushed.len()), (1, 0));
        assert_eq!(Expense::read_all(&b).unwrap().len(), 1);

        // Both edit: B deletes the lunch, A records a coffee.
        Expense::delete(&mut b, &lunch.id).unwrap();
        Expense::create(
            &mut a,
            &food.id,
            3.0,
            Some("coffee"),
            date,
            &DEFAULT_ACCOUNT_ID,
        )
        .unwrap();
        b.sync_with(&mut remote).unwrap();
        a.sync_with(&mut remote).unwrap();
        b.sync_with(&mut remote).unwrap();

        // The deletion and the new record both arrive everywhere.
        for store in [&a, &b] {
            let expenses = Expense::read_all(store).unwrap();
            assert_eq!(expenses.len(), 1);
            assert_eq!(expenses[0].comment.as_deref(), Some("coffee"));
        }
        // Syncing again with nothing new moves nothing and seals nothing.
        let idle = a.sync_with(&mut remote).unwrap();
        assert_eq!((idle.pulled.len(), idle.pushed.len()), (0, 0));

        // And what sync wrote is what a restart reads.
        drop(b);
        let reopened = Store::open(&db, 0xB).unwrap();
        assert_eq!(Expense::read_all(&reopened).unwrap().len(), 1);

        std::fs::remove_dir_all(&da).ok();
        std::fs::remove_dir_all(&db).ok();
    }

    #[test]
    fn a_device_joining_a_synced_ledger_keeps_its_default_account() {
        // The first device has made the default account its own: renamed,
        // in another currency, with an opening balance.
        let (da, db, dc) = (temp_dir("join_a"), temp_dir("join_b"), temp_dir("join_c"));
        let mut remote = rdx_sync::MemChunkStore::new();
        let mut a = Store::open(&da, 0xA).unwrap();
        Account::ensure_default(&mut a, "RUB").unwrap();
        Account::update(&mut a, &DEFAULT_ACCOUNT_ID, "Main card", "EUR", 250.0).unwrap();
        a.sync_with(&mut remote).unwrap();

        // A second device joins the right way: sync first, and only then
        // make sure a default account exists. It finds one, and leaves it.
        let mut b = Store::open(&db, 0xB).unwrap();
        b.sync_with(&mut remote).unwrap();
        Account::ensure_default(&mut b, "RUB").unwrap();
        b.sync_with(&mut remote).unwrap();
        a.sync_with(&mut remote).unwrap();
        for store in [&a, &b] {
            let accounts = Account::read_all(store).unwrap();
            assert_eq!(accounts.len(), 1);
            assert_eq!(
                (
                    accounts[0].name.as_str(),
                    accounts[0].currency.as_str(),
                    accounts[0].opening_balance
                ),
                ("Main card", "EUR", 250.0)
            );
        }

        // Why the order matters: a device that makes the default account
        // *before* its first sync stamps it later than the synced one, and
        // last-write-wins then throws the first device's account away. (The
        // categories only move this device's clock on, the way the minutes
        // between two real devices' first runs would.)
        let mut c = Store::open(&dc, 0xC).unwrap();
        for name in ["one", "two", "three"] {
            Category::create(&mut c, name).unwrap();
        }
        Account::ensure_default(&mut c, "RUB").unwrap();
        c.sync_with(&mut remote).unwrap();
        let clobbered = Account::read_all(&c).unwrap();
        assert_eq!(clobbered.len(), 1);
        assert_eq!(clobbered[0].name, "Основной");
        assert_eq!(clobbered[0].opening_balance, 0.0);

        for dir in [&da, &db, &dc] {
            std::fs::remove_dir_all(dir).ok();
        }
    }

    #[test]
    fn transfer_create_validates_accounts_and_amounts() {
        let dir = temp_dir("transfer_validation");
        let mut store = Store::open(&dir, 1).unwrap();
        let (a, b) = ([1u8; 16], [2u8; 16]);
        let date = NaiveDate::from_ymd_opt(2026, 9, 12).unwrap();
        assert!(Transfer::create(&mut store, &a, &a, 1.0, 1.0, None, date).is_err());
        assert!(Transfer::create(&mut store, &a, &b, 0.0, 1.0, None, date).is_err());
        assert!(Transfer::create(&mut store, &a, &b, 1.0, -1.0, None, date).is_err());
        assert!(Transfer::create(&mut store, &a, &b, f64::NAN, 1.0, None, date).is_err());
        assert!(Transfer::read_all(&store).unwrap().is_empty());
        assert!(Transfer::create(&mut store, &a, &b, 1.0, 2.0, Some("ok"), date).is_ok());
        assert_eq!(Transfer::read_all(&store).unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
