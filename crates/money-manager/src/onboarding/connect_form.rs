//! Tresse S3 and HTTP remote fields, shared by onboarding and Settings.
//!
//! Shared by the first-run form and the settings form, so the two cannot
//! come to disagree about what a remote is made of.

use gpui_adaptive::{Fields, Labelled, Purpose, Segments, TextField, TextFieldState};
use gpui_kit::{App, AppContext, Context, Entity, Window};
use money_core::remote::{RemoteConfig, StorageType};

pub struct RemoteFields {
    backend: StorageType,
    url: Entity<TextFieldState>,
    token: Entity<TextFieldState>,
    endpoint: Entity<TextFieldState>,
    region: Entity<TextFieldState>,
    bucket: Entity<TextFieldState>,
    access_key_id: Entity<TextFieldState>,
    secret_access_key: Entity<TextFieldState>,
    prefix: Entity<TextFieldState>,
    encryption_key: Entity<TextFieldState>,
}

impl RemoteFields {
    /// The fields, holding `remote` when there is one to edit.
    pub fn new<T: 'static>(
        remote: Option<&RemoteConfig>,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Self {
        let mut field = |purpose: Purpose, value: Option<&str>, placeholder: &'static str| {
            let value = value.unwrap_or_default().to_string();
            cx.new(|cx| {
                let mut state = TextFieldState::new(purpose, window, cx);
                state.set_placeholder(placeholder, window, cx);
                state.set_value(value, window, cx);
                state
            })
        };
        let url = field(
            Purpose::Address,
            remote.map(|r| r.url.as_str()),
            "https://sync.example.com",
        );
        let token = field(Purpose::Password, remote.map(|r| r.token.as_str()), "");
        let endpoint = field(
            Purpose::Address,
            remote.map(|r| r.s3_endpoint.as_str()),
            "https://s3.us-east-1.amazonaws.com",
        );
        let region = field(
            Purpose::Plain,
            remote.map(|r| r.s3_region.as_str()),
            "us-east-1",
        );
        let bucket = field(
            Purpose::Plain,
            remote.map(|r| r.s3_bucket.as_str()),
            "my-bucket",
        );
        let access_key_id = field(
            Purpose::Login,
            remote.map(|r| r.s3_access_key_id.as_str()),
            "",
        );
        let secret_access_key = field(
            Purpose::Password,
            remote.map(|r| r.s3_secret_access_key.as_str()),
            "",
        );
        let prefix = field(
            Purpose::Plain,
            remote.map(|r| r.repo_id.as_str()),
            "money-manager",
        );
        let encryption_key = field(
            Purpose::Password,
            remote.map(|r| r.encryption_key.as_str()),
            "Required: 32-byte base64 key",
        );
        RemoteFields {
            backend: remote.map_or(StorageType::S3, |r| r.storage_type),
            url,
            token,
            endpoint,
            region,
            bucket,
            access_key_id,
            secret_access_key,
            prefix,
            encryption_key,
        }
    }

    /// Every field, in the order they are laid out.
    pub fn fields(&self) -> [&Entity<TextFieldState>; 9] {
        [
            &self.url,
            &self.token,
            &self.endpoint,
            &self.region,
            &self.bucket,
            &self.access_key_id,
            &self.secret_access_key,
            &self.prefix,
            &self.encryption_key,
        ]
    }

    pub fn set_backend(&mut self, backend: StorageType) {
        self.backend = backend;
    }

    /// What the fields currently say, trimmed. The secret is not trimmed
    /// away at its ends by anything but this: stores hand it out without
    /// spaces, and one pasted with a trailing newline is the usual mistake.
    pub fn read(&self, cx: &App) -> Result<RemoteConfig, String> {
        let text = |field: &Entity<TextFieldState>| field.read(cx).value(cx).trim().to_string();
        Ok(RemoteConfig {
            storage_type: self.backend,
            url: text(&self.url),
            token: text(&self.token),
            s3_endpoint: text(&self.endpoint),
            s3_region: text(&self.region),
            s3_bucket: text(&self.bucket),
            s3_access_key_id: text(&self.access_key_id),
            s3_secret_access_key: text(&self.secret_access_key),
            repo_id: text(&self.prefix),
            encryption_key: text(&self.encryption_key),
        })
    }

    /// Replaces what the fields hold, as pasting a config string does.
    pub fn fill(&mut self, remote: &RemoteConfig, window: &mut Window, cx: &mut App) {
        self.backend = remote.storage_type;
        let values = [
            remote.url.clone(),
            remote.token.clone(),
            remote.s3_endpoint.clone(),
            remote.s3_region.clone(),
            remote.s3_bucket.clone(),
            remote.s3_access_key_id.clone(),
            remote.s3_secret_access_key.clone(),
            remote.repo_id.clone(),
            remote.encryption_key.clone(),
        ];
        for (field, value) in self.fields().into_iter().zip(values) {
            field.update(cx, |field, cx| field.set_value(value, window, cx));
        }
    }

    /// Whether every field is empty, which is how "no storage" is said.
    pub fn is_blank(remote: &RemoteConfig) -> bool {
        remote.url.is_empty()
            && remote.token.is_empty()
            && remote.repo_id.is_empty()
            && remote.encryption_key.is_empty()
            && remote.s3_endpoint.is_empty()
            && remote.s3_region.is_empty()
            && remote.s3_bucket.is_empty()
            && remote.s3_access_key_id.is_empty()
            && remote.s3_secret_access_key.is_empty()
    }

    /// Whether the key field holds anything: a key is only generated into an
    /// empty one.
    pub fn has_key(&self, cx: &App) -> bool {
        !self.encryption_key.read(cx).value(cx).trim().is_empty()
    }

    /// A new key, for a storage that has none yet. Replacing the key of a
    /// storage that already holds an encrypted ledger would lock it out, so
    /// an existing key is never replaced.
    pub fn generate_key(&mut self, window: &mut Window, cx: &mut App) -> Result<(), String> {
        if self.has_key(cx) {
            return Ok(());
        }
        let key = money_core::remote::generate_encryption_key()?;
        self.encryption_key
            .update(cx, |field, cx| field.set_value(key, window, cx));
        Ok(())
    }

    /// The fields of the storage type chosen, under the choice of it.
    /// `on_backend` is told when another is chosen.
    pub fn render(
        &self,
        on_backend: impl Fn(StorageType, &mut Window, &mut App) + 'static,
    ) -> Fields {
        let is_http = self.backend == StorageType::Http;
        let backend = Segments::new("remote-backend", if is_http { "http" } else { "s3" })
            .segment("s3", "S3")
            .segment("http", "HTTP")
            .on_change(move |key, window, cx| {
                let backend = if key.as_ref() == "http" {
                    StorageType::Http
                } else {
                    StorageType::S3
                };
                on_backend(backend, window, cx)
            });
        let fields = Fields::new().child(Labelled::new("Storage type", backend));
        let fields = if is_http {
            fields
                .child(TextField::new(&self.url).label("Server URL"))
                .child(TextField::new(&self.token).label("Token"))
        } else {
            fields
                .child(TextField::new(&self.endpoint).label("Endpoint"))
                .child(TextField::new(&self.region).label("Region"))
                .child(TextField::new(&self.bucket).label("Bucket"))
                .child(TextField::new(&self.access_key_id).label("Access key ID"))
                .child(TextField::new(&self.secret_access_key).label("Secret access key"))
        };
        fields
            .child(TextField::new(&self.prefix).label("Repository ID"))
            .child(TextField::new(&self.encryption_key).label("Encryption key"))
    }
}

/// Where a remote keeps the ledger, in a few words: the server's host, or
/// the bucket and its prefix.
pub fn describe(remote: &RemoteConfig) -> String {
    fn host(url: &str) -> &str {
        let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
        rest.split('/').next().unwrap_or(rest)
    }
    match remote.storage_type {
        StorageType::Http => host(&remote.url).to_string(),
        StorageType::S3 if remote.repo_id.is_empty() => {
            format!("{} on {}", remote.s3_bucket, host(&remote.s3_endpoint))
        }
        StorageType::S3 => format!(
            "{}/{} on {}",
            remote.s3_bucket,
            remote.repo_id,
            host(&remote.s3_endpoint)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_remote_is_described_by_where_it_is() {
        let server = RemoteConfig {
            storage_type: StorageType::Http,
            url: "https://sync.example.com/".into(),
            ..Default::default()
        };
        assert_eq!(describe(&server), "sync.example.com");
        let bucket = RemoteConfig {
            storage_type: StorageType::S3,
            s3_endpoint: "https://s3.eu-central-1.amazonaws.com".into(),
            s3_bucket: "money".into(),
            repo_id: "ledger".into(),
            ..Default::default()
        };
        assert_eq!(
            describe(&bucket),
            "money/ledger on s3.eu-central-1.amazonaws.com"
        );
    }
}
