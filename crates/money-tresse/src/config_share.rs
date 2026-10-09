//! Shareable config string: `tresse://<backend>?<params>`.
//!
//! The same format the Obsidian plugin (`obsidian-plugin/src/config-share.ts`) and
//! the KMP library (`TresseConfigShare.kt`) speak, so a string exported by any of
//! the three can be pasted into the others. It says how to reach the repository
//! and nothing else: backend coordinates and the credentials that open them.
//! Ignore patterns and language strategies travel in the tracked `tresse.toml`.

use std::collections::HashMap;
use std::fmt;

use crate::config::{RemoteConfig, StorageType};

const SCHEME: &str = "tresse://";

#[derive(Debug, PartialEq, Eq)]
pub struct DecodeError(String);

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DecodeError {}

pub fn encode(remote: &RemoteConfig) -> String {
    let (backend, params): (&str, Vec<(&str, &str)>) = match remote.storage_type {
        StorageType::S3 => (
            "s3",
            vec![
                ("repo_id", &remote.repo_id),
                ("endpoint", &remote.s3_endpoint),
                ("region", &remote.s3_region),
                ("bucket", &remote.s3_bucket),
                ("access_key_id", &remote.s3_access_key_id),
                ("secret_access_key", &remote.s3_secret_access_key),
                ("encryption_key", &remote.encryption_key),
            ],
        ),
        StorageType::Http => (
            "http",
            vec![
                ("url", &remote.url),
                ("repo_id", &remote.repo_id),
                ("token", &remote.token),
                ("encryption_key", &remote.encryption_key),
            ],
        ),
    };
    // A blank field is not worth spelling out, and leaving it off keeps a
    // half-filled config from reading like a complete one.
    let query = params
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(key, value)| format!("{key}={}", escape(value)))
        .collect::<Vec<_>>()
        .join("&");
    if query.is_empty() {
        format!("{SCHEME}{backend}")
    } else {
        format!("{SCHEME}{backend}?{query}")
    }
}

pub fn decode(input: &str) -> Result<RemoteConfig, DecodeError> {
    // A BOM is what a string saved to a file by a Windows editor starts with.
    let trimmed = input.trim_start_matches('\u{feff}').trim();
    let body = trimmed.strip_prefix(SCHEME).ok_or_else(|| {
        DecodeError(format!(
            "not a Tresse config string (must start with {SCHEME})"
        ))
    })?;
    let (backend, query) = body.split_once('?').unwrap_or((body, ""));
    let storage_type = match backend {
        "s3" => StorageType::S3,
        "http" => StorageType::Http,
        other => {
            return Err(DecodeError(format!(
                "unknown backend '{other}' in config string (expected 's3' or 'http')"
            )));
        }
    };

    let mut params = HashMap::new();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        // A pair splits on its first `=`: a base64 key keeps its own padding.
        let (key, value) = pair
            .split_once('=')
            .filter(|(key, _)| !key.is_empty())
            .ok_or_else(|| DecodeError(format!("malformed parameter '{pair}' in config string")))?;
        params.insert(key, unescape(value)?);
    }

    let mut value = |key: &str| params.remove(key).unwrap_or_default();
    Ok(RemoteConfig {
        storage_type,
        url: value("url"),
        token: value("token"),
        repo_id: value("repo_id"),
        encryption_key: value("encryption_key"),
        s3_endpoint: value("endpoint"),
        s3_region: value("region"),
        s3_bucket: value("bucket"),
        s3_access_key_id: value("access_key_id"),
        s3_secret_access_key: value("secret_access_key"),
    })
}

/// Escaping kept as small as it can be: `:` `/` and `=` stay literal, so a server
/// URL and a base64 key read as themselves. Everything outside that set is
/// percent-encoded, which is what stops `&` and `#` from cutting the string in
/// half, and `+` from being read back as a space by form-encoding parsers.
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~:/=".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn unescape(value: &str) -> Result<String, DecodeError> {
    let bytes = value.as_bytes();
    // Escapes are gathered as bytes, not characters: one non-ASCII character is
    // several of them.
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = bytes
                .get(index + 1..index + 3)
                .ok_or_else(|| DecodeError("truncated percent escape in config string".into()))?;
            out.push((hex_digit(hex[0])? << 4) | hex_digit(hex[1])?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    // The plugin and KMP substitute U+FFFD here; a credential silently altered
    // is worse than a refused string.
    String::from_utf8(out)
        .map_err(|_| DecodeError("percent escapes in config string are not valid UTF-8".into()))
}

fn hex_digit(byte: u8) -> Result<u8, DecodeError> {
    (byte as char)
        .to_digit(16)
        .map(|digit| digit as u8)
        .ok_or_else(|| DecodeError("invalid percent escape in config string".into()))
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};
    use crate::config::{RemoteConfig, StorageType};

    fn s3() -> RemoteConfig {
        RemoteConfig {
            storage_type: StorageType::S3,
            url: String::new(),
            token: String::new(),
            repo_id: "vault".into(),
            encryption_key: "a+b/c==".into(),
            s3_endpoint: "https://storage.example.com".into(),
            s3_region: "ru-central1".into(),
            s3_bucket: "notes".into(),
            s3_access_key_id: "access".into(),
            s3_secret_access_key: "se&cr#et".into(),
        }
    }

    fn http() -> RemoteConfig {
        RemoteConfig {
            storage_type: StorageType::Http,
            url: "https://sync.example.com:8443/api".into(),
            token: "to ken".into(),
            repo_id: "заметки".into(),
            encryption_key: "key=".into(),
            s3_endpoint: String::new(),
            s3_region: String::new(),
            s3_bucket: String::new(),
            s3_access_key_id: String::new(),
            s3_secret_access_key: String::new(),
        }
    }

    /// Byte-for-byte what `encodeConfig` in the plugin produces for the same
    /// settings: the three implementations must agree on the wire form.
    #[test]
    fn s3_string_matches_the_plugin_wire_form() {
        assert_eq!(
            encode(&s3()),
            "tresse://s3?repo_id=vault&endpoint=https://storage.example.com\
             &region=ru-central1&bucket=notes&access_key_id=access\
             &secret_access_key=se%26cr%23et&encryption_key=a%2Bb/c=="
        );
    }

    #[test]
    fn http_string_matches_the_plugin_wire_form() {
        assert_eq!(
            encode(&http()),
            "tresse://http?url=https://sync.example.com:8443/api\
             &repo_id=%D0%B7%D0%B0%D0%BC%D0%B5%D1%82%D0%BA%D0%B8\
             &token=to%20ken&encryption_key=key="
        );
    }

    #[test]
    fn a_remote_survives_the_round_trip() {
        assert_eq!(decode(&encode(&s3())).unwrap(), s3());
        assert_eq!(decode(&encode(&http())).unwrap(), http());
    }

    #[test]
    fn blank_fields_are_left_out_and_read_back_blank() {
        let mut remote = http();
        remote.token.clear();
        remote.encryption_key.clear();

        let encoded = encode(&remote);

        assert_eq!(
            encoded,
            "tresse://http?url=https://sync.example.com:8443/api\
             &repo_id=%D0%B7%D0%B0%D0%BC%D0%B5%D1%82%D0%BA%D0%B8"
        );
        assert_eq!(decode(&encoded).unwrap(), remote);
    }

    #[test]
    fn fields_of_the_other_backend_are_not_exported() {
        let mut remote = s3();
        remote.url = "https://stale.example.com".into();
        remote.token = "stale".into();

        assert_eq!(encode(&remote), encode(&s3()));
    }

    #[test]
    fn surrounding_whitespace_and_lowercase_escapes_are_accepted() {
        let remote = decode("\u{feff}  tresse://http?repo_id=a%2fb&unknown=1\r\n").unwrap();

        assert_eq!(remote.repo_id, "a/b");
        assert!(matches!(remote.storage_type, StorageType::Http));
    }

    #[test]
    fn a_backend_without_parameters_decodes_to_a_blank_remote() {
        let remote = decode("tresse://s3").unwrap();

        assert!(matches!(remote.storage_type, StorageType::S3));
        assert_eq!(remote.repo_id, "");
    }

    #[test]
    fn a_string_that_is_not_a_config_string_is_rejected() {
        for input in [
            "[ignore]\npatterns = []",
            "tresse1:eyJhIjoxfQ",
            "tresse://ftp?url=x",
            "tresse://s3?bucket",
            "tresse://s3?=value",
            "tresse://s3?bucket=%4",
            "tresse://s3?bucket=%zz",
            "tresse://s3?bucket=%FF",
        ] {
            assert!(decode(input).is_err(), "accepted {input:?}");
        }
    }
}
