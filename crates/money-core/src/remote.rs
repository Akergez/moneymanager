//! Tresse remote configuration; persisted only in `.tresse/remotes.toml`.
use base64::Engine;
pub use money_tresse::{RemoteConfig, StorageType};
pub const DEFAULT_CURRENCY: &str = "RUB";
/// A fresh base64 encryption key, as `encryption_key` wants it.
pub fn generate_encryption_key() -> Result<String, String> {
    let key = rdx_sync::crypto::generate_key().map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(key))
}

/// A fresh native RDX writer source. It is embedded in document stamps, not
/// kept as a Tresse setting.
///
/// Masked into the low 60 bits: the RDX stamp model reserves the upper 4 bits
/// of `source`. `| 1` keeps it nonzero. The terminal version generated its
/// source the same way, so the two are interchangeable.
pub fn generate_source() -> u64 {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    let mut low = [0u8; 8];
    low.copy_from_slice(&bytes[..8]);
    (u64::from_le_bytes(low) & 0x0FFF_FFFF_FFFF_FFFF) | 1
}
