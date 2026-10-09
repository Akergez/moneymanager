//! Tresse S3 and HTTP remote fields, shared by onboarding and Settings.
//!
//! Shared by the first-run form and the settings dialog, so the two cannot
//! come to disagree about what a remote is made of.

use gpui_kit::component::button::{Button, ButtonGroup};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Selectable, Sizable, Size, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Div, Entity, Subscription, Window};
use money_core::remote::{RemoteConfig, StorageType};

use crate::ui;

pub struct RemoteFields {
    backend: Entity<InputState>,
    url: Entity<InputState>,
    token: Entity<InputState>,
    endpoint: Entity<InputState>,
    region: Entity<InputState>,
    bucket: Entity<InputState>,
    access_key_id: Entity<InputState>,
    secret_access_key: Entity<InputState>,
    prefix: Entity<InputState>,
    encryption_key: Entity<InputState>,
    _backend_watch: Subscription,
}

impl RemoteFields {
    /// The inputs, holding `remote` when there is one to edit.
    pub fn new<T: 'static>(
        remote: Option<&RemoteConfig>,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Self {
        let mut input = |value: Option<&str>, placeholder: &'static str, masked: bool| {
            let value = value.unwrap_or_default().to_string();
            cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(value)
                    .placeholder(placeholder)
                    .masked(masked)
            })
        };
        let backend = input(
            Some(
                match remote.map(|r| r.storage_type).unwrap_or(StorageType::S3) {
                    StorageType::S3 => "s3",
                    StorageType::Http => "http",
                },
            ),
            "",
            false,
        );
        let url = input(
            remote.map(|r| r.url.as_str()),
            "https://sync.example.com",
            false,
        );
        let token = input(remote.map(|r| r.token.as_str()), "", true);
        let endpoint = input(
            remote.map(|r| r.s3_endpoint.as_str()),
            "https://s3.us-east-1.amazonaws.com",
            false,
        );
        let region = input(remote.map(|r| r.s3_region.as_str()), "us-east-1", false);
        let bucket = input(remote.map(|r| r.s3_bucket.as_str()), "my-bucket", false);
        let access_key_id = input(remote.map(|r| r.s3_access_key_id.as_str()), "", false);
        let secret_access_key = input(remote.map(|r| r.s3_secret_access_key.as_str()), "", true);
        let prefix = input(remote.map(|r| r.repo_id.as_str()), "money-manager", false);
        let encryption_key = input(
            remote.map(|r| r.encryption_key.as_str()),
            "Required: 32-byte base64 key",
            true,
        );
        let backend_watch = cx.observe(&backend, |_, _, cx| cx.notify());
        RemoteFields {
            backend,
            _backend_watch: backend_watch,
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

    pub fn inputs(&self) -> [&Entity<InputState>; 10] {
        [
            &self.backend,
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

    /// What the fields currently say, trimmed. The secret is not trimmed
    /// away at its ends by anything but this: stores hand it out without
    /// spaces, and one pasted with a trailing newline is the usual mistake.
    pub fn read(&self, cx: &App) -> Result<RemoteConfig, String> {
        let text = |input: &Entity<InputState>| input.read(cx).value().trim().to_string();
        let encryption_key = text(&self.encryption_key);
        Ok(RemoteConfig {
            storage_type: match text(&self.backend).as_str() {
                "s3" => StorageType::S3,
                "http" => StorageType::Http,
                _ => return Err("Backend must be s3 or http.".into()),
            },
            url: text(&self.url),
            token: text(&self.token),
            s3_endpoint: text(&self.endpoint),
            s3_region: text(&self.region),
            s3_bucket: text(&self.bucket),
            s3_access_key_id: text(&self.access_key_id),
            s3_secret_access_key: text(&self.secret_access_key),
            repo_id: text(&self.prefix),
            encryption_key,
        })
    }

    /// Replaces what the fields hold, as pasting a config string does.
    pub fn fill(&self, remote: &RemoteConfig, window: &mut Window, cx: &mut App) {
        let values = [
            match remote.storage_type {
                StorageType::S3 => "s3",
                StorageType::Http => "http",
            }
            .to_string(),
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
        for (input, value) in self.inputs().into_iter().zip(values) {
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }
    }

    pub fn render(&self, size: Size, cx: &App) -> Div {
        let is_http = self.backend.read(cx).value() == "http";
        let backend = self.backend.clone();
        let field = |label: &'static str, input: &Entity<InputState>| {
            ui::field(label, Input::new(input).with_size(size))
        };
        v_flex()
            .gap_3()
            .child(ui::field(
                "Storage type",
                ButtonGroup::new("remote-backend")
                    .outline()
                    .child(Button::new("s3").label("S3").selected(!is_http))
                    .child(Button::new("http").label("HTTP").selected(is_http))
                    .on_click(move |picked, window, cx| {
                        let value = if picked.first() == Some(&1) {
                            "http"
                        } else {
                            "s3"
                        };
                        backend.update(cx, |input, cx| input.set_value(value, window, cx));
                    }),
            ))
            .when(is_http, |column| {
                column
                    .child(field("Server URL", &self.url))
                    .child(field("Token", &self.token))
            })
            .when(!is_http, |column| {
                column
                    .child(field("Endpoint", &self.endpoint))
                    .child(field("Region", &self.region))
                    .child(field("Bucket", &self.bucket))
                    .child(field("Access key ID", &self.access_key_id))
                    .child(field("Secret access key", &self.secret_access_key))
            })
            .child(field("Repository ID", &self.prefix))
            .child(field("Encryption key", &self.encryption_key))
    }
}
