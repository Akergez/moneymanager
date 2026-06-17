//! `money_manager.toml` — local instance config plus optional S3 sync remote.
//!
//! ```toml
//! # Stable per-installation stamp source for CRDT writes (auto-generated).
//! source = 12345678901234567
//!
//! [remote]
//! endpoint = "https://s3.us-east-1.amazonaws.com"
//! region = "us-east-1"
//! bucket = "my-bucket"
//! access_key_id = "AKID..."
//! secret_access_key = "..."
//! prefix = "money-manager"           # object-key prefix (repo id)
//! encryption_key = "base64-32-bytes" # optional E2E key; omit for plaintext
//! ```

use std::path::Path;

use base64::Engine;
use serde::{Deserialize, Serialize};

/// Version-tagged prefix for the single-string config form. Shared verbatim with
/// the Obsidian plugin (`obsidian-plugin/src/config-share.ts`), so a `tresse1:`
/// string can be moved between the plugin and this CLI in either direction.
const SHARE_PREFIX: &str = "tresse1:";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Per-installation stamp source. Generated on first run if missing.
    #[serde(default)]
    pub source: Option<u64>,
    pub remote: Option<RemoteConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Parse a shareable string produced by [`to_share_string`] or by the
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
}

/// A ready-to-edit `money_manager.toml` skeleton, with every S3 field present
/// and commented. Printed by `config --template`; `source` is intentionally
/// omitted because it is generated on first run.
pub const CONFIG_TEMPLATE: &str = "\
# money_manager.toml — local instance configuration.
#
# `source` is a per-installation CRDT stamp id, generated automatically on first
# run, so you normally do not set it here.

# Optional S3 sync remote. Point it at any S3-compatible bucket (AWS S3, MinIO,
# Backblaze B2, Yandex Object Storage, …). Delete this whole section to keep the
# data local-only. Tip: you can also import these values from a single string
# with `money_manager config --import \"tresse1:...\"`.
[remote]
endpoint = \"https://s3.us-east-1.amazonaws.com\"
region = \"us-east-1\"
bucket = \"my-bucket\"
access_key_id = \"AKID...\"
secret_access_key = \"...\"
# Object-key prefix; acts as a repo id within the bucket.
prefix = \"money-manager\"
# Optional end-to-end encryption key (base64, 32 bytes). Generate one with
# `money_manager keygen`. Omit this line to store chunks unencrypted.
encryption_key = \"base64-32-bytes\"
";

impl Config {
    /// Load config from `path`, or return defaults if the file is absent.
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        toml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let text =
            toml::to_string_pretty(self).map_err(|e| format!("serialize config: {e}"))?;
        std::fs::write(path, text).map_err(|e| format!("write {}: {e}", path.display()))
    }

    /// Return the configured source, generating and persisting one if absent.
    pub fn ensure_source(&mut self, path: &Path) -> Result<u64, String> {
        if let Some(s) = self.source {
            return Ok(s);
        }
        let bytes = uuid::Uuid::new_v4().into_bytes();
        // Mask into the low 60 bits: the RDX stamp model reserves the upper 4
        // bits of `source`, and — crucially — TOML integers are i64, so a raw
        // u64 above i64::MAX fails to serialize. `| 1` keeps it nonzero.
        let source = (u64::from_le_bytes(bytes[..8].try_into().unwrap()) & 0x0FFF_FFFF_FFFF_FFFF) | 1;
        self.source = Some(source);
        self.save(path)?;
        Ok(source)
    }
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
        assert_eq!(back.endpoint, c.endpoint);
        assert_eq!(back.region, c.region);
        assert_eq!(back.bucket, c.bucket);
        assert_eq!(back.access_key_id, c.access_key_id);
        assert_eq!(back.secret_access_key, c.secret_access_key);
        assert_eq!(back.prefix, c.prefix);
        assert_eq!(back.encryption_key, c.encryption_key);
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
    fn template_is_valid_parseable_toml() {
        let cfg: Config = toml::from_str(CONFIG_TEMPLATE).expect("template must be valid TOML");
        let remote = cfg.remote.expect("template must include a [remote] section");
        assert_eq!(remote.endpoint, "https://s3.us-east-1.amazonaws.com");
        assert_eq!(remote.bucket, "my-bucket");
        // `source` is intentionally left out of the template.
        assert!(cfg.source.is_none());
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
}
