//! The S3 remote a ledger syncs with, and the one-string form of it.
//!
//! The same seven values the terminal version kept under `[remote]` in
//! `money_manager.toml`. Here they are a value the application stores wherever
//! it keeps its settings; the field names are unchanged so that either form
//! reads the other.

use base64::Engine;
use serde::{Deserialize, Serialize};

/// Version-tagged prefix for the single-string config form. Shared verbatim with
/// the Obsidian plugin (`obsidian-plugin/src/config-share.ts`), so a `tresse1:`
/// string can be moved between the plugin and this application in either
/// direction.
pub const SHARE_PREFIX: &str = "tresse1:";

/// Currency of the default account when nothing else was chosen.
pub const DEFAULT_CURRENCY: &str = "RUB";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RemoteConfig {
    pub endpoint: String,
    #[serde(default)]
    pub region: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    #[serde(default)]
    pub prefix: String,
    /// Base64-encoded 32-byte key for end-to-end encryption. Omit for plaintext.
    #[serde(default)]
    pub encryption_key: Option<String>,
}

/// The full settings shape used by the Obsidian plugin's shareable config
/// string. Field names (camelCase) match `TresseSettings` in the plugin so the
/// JSON payload is byte-for-byte interchangeable. money-manager only populates
/// the S3 fields; HTTP-backend fields ride along empty for round-tripping.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ShareSettings {
    #[serde(rename = "storageType", default)]
    storage_type: String,
    #[serde(rename = "repoId", default)]
    repo_id: String,
    #[serde(rename = "encryptionKey", default)]
    encryption_key: String,
    #[serde(rename = "serverUrl", default)]
    server_url: String,
    #[serde(default)]
    token: String,
    #[serde(rename = "s3Endpoint", default)]
    s3_endpoint: String,
    #[serde(rename = "s3Region", default)]
    s3_region: String,
    #[serde(rename = "s3Bucket", default)]
    s3_bucket: String,
    #[serde(rename = "s3AccessKeyId", default)]
    s3_access_key_id: String,
    #[serde(rename = "s3SecretAccessKey", default)]
    s3_secret_access_key: String,
}

impl RemoteConfig {
    /// Encode this remote as a single shareable string: `tresse1:` followed by
    /// the base64url (unpadded) JSON of the settings. Carries the secret key and
    /// encryption key, so treat it as a credential bundle.
    pub fn to_share_string(&self) -> String {
        let s = ShareSettings {
            storage_type: "s3".to_string(),
            repo_id: self.prefix.clone(),
            encryption_key: self.encryption_key.clone().unwrap_or_default(),
            s3_endpoint: self.endpoint.clone(),
            s3_region: self.region.clone(),
            s3_bucket: self.bucket.clone(),
            s3_access_key_id: self.access_key_id.clone(),
            s3_secret_access_key: self.secret_access_key.clone(),
            ..Default::default()
        };
        let json = serde_json::to_string(&s).unwrap_or_default();
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes());
        format!("{SHARE_PREFIX}{b64}")
    }

    /// Parse a shareable string produced by [`Self::to_share_string`] or by the
    /// Obsidian plugin. Unknown/missing fields fall back to empty defaults.
    pub fn from_share_string(input: &str) -> Result<Self, String> {
        let body = input
            .trim()
            .strip_prefix(SHARE_PREFIX)
            .ok_or_else(|| format!("not a config string (missing '{SHARE_PREFIX}' prefix)"))?;
        let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(body)
            .map_err(|e| format!("config string is corrupt (invalid base64): {e}"))?;
        let s: ShareSettings = serde_json::from_slice(&json)
            .map_err(|e| format!("config string is corrupt (invalid JSON): {e}"))?;
        let encryption_key = (!s.encryption_key.is_empty()).then_some(s.encryption_key);
        Ok(RemoteConfig {
            endpoint: s.s3_endpoint,
            region: s.s3_region,
            bucket: s.s3_bucket,
            access_key_id: s.s3_access_key_id,
            secret_access_key: s.s3_secret_access_key,
            prefix: s.repo_id,
            encryption_key,
        })
    }

    /// What has to be there before a sync is worth attempting: somewhere to
    /// connect to, a bucket, and both halves of the credentials. An encryption
    /// key, when given, has to be one.
    pub fn validate(&self) -> Result<(), String> {
        let required = [
            ("Endpoint", &self.endpoint),
            ("Bucket", &self.bucket),
            ("Access key ID", &self.access_key_id),
            ("Secret access key", &self.secret_access_key),
        ];
        if let Some((name, _)) = required.iter().find(|(_, value)| value.trim().is_empty()) {
            return Err(format!("{name} is required."));
        }
        if !(self.endpoint.starts_with("https://") || self.endpoint.starts_with("http://")) {
            return Err("Endpoint must start with https:// or http://.".to_string());
        }
        if let Some(key) = &self.encryption_key {
            rdx_sync::crypto::decode_key_b64(key)
                .map_err(|_| "Encryption key must be 32 bytes in base64.".to_string())?;
        }
        Ok(())
    }
}

/// A fresh base64 encryption key, as `encryption_key` wants it.
pub fn generate_encryption_key() -> Result<String, String> {
    let key = rdx_sync::crypto::generate_key().map_err(|e| e.to_string())?;
    Ok(base64::engine::general_purpose::STANDARD.encode(key))
}

/// A per-installation stamp source for CRDT writes.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RemoteConfig {
        RemoteConfig {
            endpoint: "https://storage.yandexcloud.net".to_string(),
            region: "ru-central1".to_string(),
            bucket: "my-bucket".to_string(),
            access_key_id: "AKID".to_string(),
            secret_access_key: "secret".to_string(),
            prefix: "money-manager".to_string(),
            encryption_key: Some("a2V5".to_string()),
        }
    }

    #[test]
    fn share_string_roundtrips() {
        let c = sample();
        let s = c.to_share_string();
        assert!(s.starts_with(SHARE_PREFIX));
        let back = RemoteConfig::from_share_string(&s).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn decodes_obsidian_plugin_string() {
        // Exactly the shape the Obsidian plugin's config-share.ts emits:
        // tresse1: + base64url(JSON(TresseSettings)), camelCase keys, s3 backend.
        let json = r#"{"storageType":"s3","repoId":"vault","encryptionKey":"k","serverUrl":"","token":"","s3Endpoint":"https://s3.example.com","s3Region":"us-east-1","s3Bucket":"b","s3AccessKeyId":"AK","s3SecretAccessKey":"sk"}"#;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes());
        let rc = RemoteConfig::from_share_string(&format!("{SHARE_PREFIX}{b64}")).unwrap();
        assert_eq!(rc.endpoint, "https://s3.example.com");
        assert_eq!(rc.bucket, "b");
        assert_eq!(rc.prefix, "vault");
        assert_eq!(rc.encryption_key.as_deref(), Some("k"));
    }

    #[test]
    fn rejects_non_prefixed_string() {
        assert!(RemoteConfig::from_share_string("hello").is_err());
    }

    #[test]
    fn empty_encryption_key_decodes_as_none() {
        let json = r#"{"storageType":"s3","s3Endpoint":"e","s3Bucket":"b","encryptionKey":""}"#;
        let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes());
        let rc = RemoteConfig::from_share_string(&format!("{SHARE_PREFIX}{b64}")).unwrap();
        assert_eq!(rc.encryption_key, None);
    }

    #[test]
    fn the_settings_form_keeps_the_terminal_versions_field_names() {
        // What `[remote]` in money_manager.toml held, as JSON.
        let stored = serde_json::to_value(sample()).unwrap();
        for field in [
            "endpoint",
            "region",
            "bucket",
            "access_key_id",
            "secret_access_key",
            "prefix",
            "encryption_key",
        ] {
            assert!(stored.get(field).is_some(), "{field} is missing");
        }
        let back: RemoteConfig = serde_json::from_value(stored).unwrap();
        assert_eq!(back, sample());
    }

    #[test]
    fn a_remote_is_checked_before_it_is_used() {
        let mut remote = sample();
        remote.encryption_key = None;
        assert_eq!(remote.validate(), Ok(()));

        let mut no_bucket = remote.clone();
        no_bucket.bucket = "  ".to_string();
        assert_eq!(no_bucket.validate(), Err("Bucket is required.".to_string()));

        let mut bare_host = remote.clone();
        bare_host.endpoint = "storage.example.com".to_string();
        assert!(bare_host.validate().is_err());

        let mut short_key = remote.clone();
        short_key.encryption_key = Some("a2V5".to_string());
        assert!(short_key.validate().is_err());

        remote.encryption_key = Some(generate_encryption_key().unwrap());
        assert_eq!(remote.validate(), Ok(()));
    }

    #[test]
    fn a_source_fits_the_stamp_and_is_never_zero() {
        for _ in 0..64 {
            let source = generate_source();
            assert_ne!(source, 0);
            assert_eq!(source >> 60, 0);
        }
    }
}
