//! The seven fields of a sync remote, as inputs.
//!
//! Shared by the first-run form and the settings dialog, so the two cannot
//! come to disagree about what a remote is made of.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Sizable, Size, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Div, Entity, Window};
use money_core::remote::RemoteConfig;

use crate::ui;

pub struct RemoteFields {
    endpoint: Entity<InputState>,
    region: Entity<InputState>,
    bucket: Entity<InputState>,
    access_key_id: Entity<InputState>,
    secret_access_key: Entity<InputState>,
    prefix: Entity<InputState>,
    encryption_key: Entity<InputState>,
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
        RemoteFields {
            endpoint: input(
                remote.map(|r| r.endpoint.as_str()),
                "https://s3.us-east-1.amazonaws.com",
                false,
            ),
            region: input(remote.map(|r| r.region.as_str()), "us-east-1", false),
            bucket: input(remote.map(|r| r.bucket.as_str()), "my-bucket", false),
            access_key_id: input(remote.map(|r| r.access_key_id.as_str()), "", false),
            secret_access_key: input(remote.map(|r| r.secret_access_key.as_str()), "", true),
            prefix: input(remote.map(|r| r.prefix.as_str()), "money-manager", false),
            encryption_key: input(
                remote.and_then(|r| r.encryption_key.as_deref()),
                "Leave empty to store unencrypted",
                true,
            ),
        }
    }

    pub fn inputs(&self) -> [&Entity<InputState>; 7] {
        [
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
    pub fn read(&self, cx: &App) -> RemoteConfig {
        let text = |input: &Entity<InputState>| input.read(cx).value().trim().to_string();
        let encryption_key = text(&self.encryption_key);
        RemoteConfig {
            endpoint: text(&self.endpoint),
            region: text(&self.region),
            bucket: text(&self.bucket),
            access_key_id: text(&self.access_key_id),
            secret_access_key: text(&self.secret_access_key),
            prefix: text(&self.prefix),
            encryption_key: (!encryption_key.is_empty()).then_some(encryption_key),
        }
    }

    /// Replaces what the fields hold, as pasting a config string does.
    pub fn fill(&self, remote: &RemoteConfig, window: &mut Window, cx: &mut App) {
        let values = [
            remote.endpoint.clone(),
            remote.region.clone(),
            remote.bucket.clone(),
            remote.access_key_id.clone(),
            remote.secret_access_key.clone(),
            remote.prefix.clone(),
            remote.encryption_key.clone().unwrap_or_default(),
        ];
        for (input, value) in self.inputs().into_iter().zip(values) {
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }
    }

    pub fn render(&self, size: Size) -> Div {
        let field = |label: &'static str, input: &Entity<InputState>| {
            ui::field(label, Input::new(input).with_size(size))
        };
        v_flex()
            .gap_3()
            .child(field("Endpoint", &self.endpoint))
            .child(field("Region", &self.region))
            .child(field("Bucket", &self.bucket))
            .child(field("Access key ID", &self.access_key_id))
            .child(field("Secret access key", &self.secret_access_key))
            .child(field("Prefix", &self.prefix))
            .child(field("Encryption key", &self.encryption_key))
    }
}
