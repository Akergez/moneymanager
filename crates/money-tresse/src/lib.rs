//! Embedded filesystem client for Tresse.
//!
//! The filesystem adapters are adapted from tresse-cli at
//! 809dff9a0a5b0d162a2efbca602b5cceb4073f9c. The upstream CLI does not yet
//! publish a library target. Keep its configuration, layout, locking, index
//! and operation journal compatible; diff, CAS and sync algorithms stay upstream.

mod cas;
pub mod config;
mod config_share;
mod file_index;
mod fs_writer;
mod operation;
mod restore_sink;
mod storage;
mod worktree;
mod commands {
    pub mod commit;
    pub mod init;
    pub mod restore;
    pub mod sync;
}

pub use config::{RemoteConfig, StorageType, TresseConfig};
use std::path::Path;

pub fn init(root: &Path) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    commands::init::run(root).map_err(|e| e.to_string())
}

/// Settle interrupted work while holding the same lock used by the CLI.
pub fn recover(root: &Path) -> Result<(), String> {
    commands::restore::run(root, None).map_err(|e| e.to_string())
}

/// Edit the worktree and commit under one repository lock. The callback must
/// read the file afresh: another client may have changed it since the last view.
pub fn edit(root: &Path, write: impl FnOnce() -> Result<(), String>) -> Result<(), String> {
    commands::commit::run_with(root, || write().map_err(Into::into)).map_err(|e| e.to_string())
}

pub fn commit(root: &Path) -> Result<(), String> {
    commands::commit::run(root).map_err(|e| e.to_string())
}

/// Commit local edits, exchange Tresse objects, then apply incoming versions.
/// All remote parameters are re-read from `.tresse/remotes.toml`.
pub fn sync(root: &Path) -> Result<(usize, usize), String> {
    commit(root)?;
    let report = commands::sync::run(root).map_err(|e| e.to_string())?;
    recover(root)?;
    Ok(report)
}

/// Transport injection for offline tests; the key still comes from the file.
pub fn sync_with(
    root: &Path,
    remote: &mut impl rdx_sync::RemoteKv,
) -> Result<(usize, usize), String> {
    let config = TresseConfig::load(root).map_err(|e| e.to_string())?;
    let origin = config.origin().ok_or("remote is not configured")?;
    let key =
        tresse_lib::crypto::decode_key_b64(&origin.encryption_key).map_err(|e| e.to_string())?;
    hydrate(root, remote, &key)?;
    commit(root)?;
    let report = commands::sync::exchange(root, remote, &key).map_err(|e| e.to_string())?;
    hydrate(root, remote, &key)?;
    recover(root)?;
    Ok(report)
}

fn hydrate(root: &Path, remote: &impl rdx_sync::RemoteKv, key: &[u8; 32]) -> Result<(), String> {
    let mut local = cas::open_local_store(root).map_err(|e| e.to_string())?;
    let meta = cas::load_merged_meta(&local).map_err(|e| e.to_string())?;
    let missing =
        tresse_lib::partial_sync::content_fetch_plan(&meta, &local).map_err(|e| e.to_string())?;
    tresse_lib::partial_sync::pull_remote_content(&mut local, remote, key, &missing)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests;
