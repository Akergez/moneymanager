use std::path::Path;

use rdx_rs::Stamp;
use rdx_sync::{ChunkClass, ObjectStore, RemoteKv, SyncError};
use rdx_sync_fs::FileObjectStore;
use rdx_sync_s3::{S3ChunkStore, S3Config};
use tresse_lib::VersionLabel;
use tresse_lib::commit::{MergedMeta, merge_local_meta};
use tresse_lib::entry::Entry;
use tresse_lib::meta_bundle::{self, MetaBundleError};

use crate::config::{RemoteConfig, StorageType};
use crate::storage::{ServerTresseClient, parse_version_label};

pub const DEFAULT_CHUNK_RAW_SIZE: u64 = 64 * 1024 * 1024;
pub const DEFAULT_MAX_OBJECT_SIZE: usize = 96 * 1024 * 1024;

pub fn open_local_store(root: &Path) -> Result<FileObjectStore, SyncError> {
    FileObjectStore::open(root.join(".tresse"))
}

pub fn load_merged_meta(store: &FileObjectStore) -> Result<MergedMeta, MetaBundleError> {
    Ok(merge_local_meta(load_meta_entries(store)?)?)
}

/// Записи меты до слияния — так их читает домен показа (`tresse-history`).
pub fn load_meta_entries(store: &FileObjectStore) -> Result<Vec<Entry>, MetaBundleError> {
    let objects = meta_bundle::load_meta_objects(store)?;
    Ok(meta_bundle::assemble_meta(&objects).entries)
}

/// Метка версии из аргумента → штамп из канонического списка.
///
/// Метка — не адрес: собрать `source` обратно нельзя, сверяются метки.
pub fn resolve_version(meta: &MergedMeta, spec: &str) -> Result<Stamp, String> {
    let label = parse_version_label(spec)?;
    tresse_lib::versioning::find_version_by_label(&meta.versions, label).ok_or_else(|| {
        let available = meta
            .versions
            .iter()
            .map(|stamp| VersionLabel::from_source(stamp.source).format())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "version {} not found (available: {available})",
            label.format()
        )
    })
}

pub enum ConfiguredRemote {
    Http(ServerTresseClient),
    S3(S3ChunkStore),
}

impl ConfiguredRemote {
    pub fn open(config: &RemoteConfig) -> Result<Self, SyncError> {
        match config.storage_type {
            StorageType::Http => Ok(Self::Http(ServerTresseClient::new(
                config.url.clone(),
                config.token.clone(),
                config.repo_id.clone(),
                &config.encryption_key,
            )?)),
            StorageType::S3 => Ok(Self::S3(S3ChunkStore::with_encryption_key(
                S3Config {
                    endpoint: config.s3_endpoint.clone(),
                    region: config.s3_region.clone(),
                    bucket: config.s3_bucket.clone(),
                    access_key_id: config.s3_access_key_id.clone(),
                    secret_access_key: config.s3_secret_access_key.clone(),
                    prefix: config.repo_id.clone(),
                },
                &config.encryption_key,
            )?)),
        }
    }
}

impl RemoteKv for ConfiguredRemote {
    fn list(&self, prefix: &str) -> Result<Vec<String>, SyncError> {
        match self {
            Self::Http(remote) => remote.list(prefix),
            Self::S3(remote) => remote.list(prefix),
        }
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, SyncError> {
        match self {
            Self::Http(remote) => remote.get(key),
            Self::S3(remote) => remote.get(key),
        }
    }

    fn put(&mut self, key: &str, value: &[u8]) -> Result<(), SyncError> {
        match self {
            Self::Http(remote) => remote.put(key, value),
            Self::S3(remote) => remote.put(key, value),
        }
    }

    fn delete(&mut self, key: &str) -> Result<(), SyncError> {
        match self {
            Self::Http(remote) => remote.delete(key),
            Self::S3(remote) => remote.delete(key),
        }
    }
}

pub struct EmptyRemote;

impl ObjectStore for EmptyRemote {
    fn list_objects(&self, _class: ChunkClass) -> Result<Vec<String>, SyncError> {
        Ok(Vec::new())
    }

    fn get_bytes(&self, _class: ChunkClass, id: &String) -> Result<Vec<u8>, SyncError> {
        Err(SyncError::NotFound(id.clone()))
    }

    fn put_bytes(&mut self, class: ChunkClass, bytes: &[u8]) -> Result<String, SyncError> {
        let _ = (class, bytes);
        Err(SyncError::Other("empty remote is read-only".to_string()))
    }
}
