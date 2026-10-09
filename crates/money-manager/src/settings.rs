//! What this installation remembers that is not the ledger.
//!
//! `settings.json` holds display preferences, the currency of a new ledger
//! and the last selected account. Tresse settings are read from repository
//! files and never cached here. Old `source` and `remote` keys are read only
//! until migration completes. Nothing in this file is synced.
//!
//! The file is read once, at startup, and kept in memory as a GPUI global;
//! every change is written straight back. A key this version does not know is
//! left alone, so a later version's settings survive an older one's writes.

use std::fs;
use std::path::{Path, PathBuf};

use gpui_kit::{App, Global};
use money_core::models::CategoryStyle;
use money_core::remote::{self, DEFAULT_CURRENCY, RemoteConfig};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Where category colours and icons were kept before the ledger had fields
/// for them. Only read, to move them there; see
/// [`crate::book::Book::adopt_legacy_styles`].
const LEGACY_CATEGORY_STYLES: &str = "category_styles";

pub struct Settings {
    file: PathBuf,
    values: Map<String, Value>,
}

impl Global for Settings {}

impl Settings {
    /// Reads `file`. A missing file is the ordinary first run; one that does
    /// not parse is reported and treated the same way rather than refused,
    /// since the ledger itself is elsewhere and intact.
    pub fn load(file: PathBuf) -> Self {
        let values = match fs::read_to_string(&file) {
            Ok(raw) => serde_json::from_str::<Map<String, Value>>(&raw).unwrap_or_else(|error| {
                tracing::warn!(%error, "could not parse the settings");
                Map::new()
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Map::new(),
            Err(error) => {
                tracing::warn!(%error, "could not read the settings");
                Map::new()
            }
        };
        Self { file, values }
    }

    pub fn install(cx: &mut App) {
        cx.set_global(Self::load(crate::paths::settings_file()));
    }

    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    /// Changes the settings and writes them back.
    pub fn update<R>(cx: &mut App, change: impl FnOnce(&mut Self) -> R) -> R {
        let settings = cx.global_mut::<Self>();
        let result = change(settings);
        if let Err(error) = settings.save() {
            tracing::warn!(%error, "could not save the settings");
        }
        result
    }

    fn save(&self) -> std::io::Result<()> {
        write_atomically(&self.file, &Value::Object(self.values.clone()).to_string())
    }

    fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.values
            .get(key)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }

    fn set<T: Serialize>(&mut self, key: &str, value: Option<T>) {
        match value.and_then(|value| serde_json::to_value(value).ok()) {
            Some(value) => self.values.insert(key.to_string(), value),
            None => self.values.remove(key),
        };
    }

    /// Legacy stamp source, read only while converting the previous format.
    pub fn source(&self) -> Option<u64> {
        self.get("source")
    }

    /// Read Tresse's file, rather than keeping a second copy in this global.
    pub fn remote(&self) -> Result<Option<RemoteConfig>, String> {
        money_core::tresse::config::read_remote(&crate::paths::ledger_dir())
    }

    /// Drop old transport settings only after the native baseline is durable.
    pub fn finish_migration(cx: &mut App) -> Result<(), String> {
        let settings = cx.global_mut::<Self>();
        settings.clear_legacy_transport()
    }

    fn clear_legacy_transport(&mut self) -> Result<(), String> {
        let settings = self;
        let old = settings.values.clone();
        settings.values.remove("source");
        settings.values.remove("remote");
        if let Err(error) = settings.save() {
            settings.values = old;
            return Err(format!("could not reset old sync settings: {error}"));
        }
        Ok(())
    }

    pub fn has_legacy_config(&self) -> bool {
        self.values.contains_key("source") || self.values.contains_key("remote")
    }

    /// Currency (ISO-4217) of a brand-new default account.
    pub fn default_currency(&self) -> String {
        self.get("default_currency")
            .unwrap_or_else(|| DEFAULT_CURRENCY.to_string())
    }

    pub fn set_default_currency(&mut self, currency: &str) {
        self.set("default_currency", Some(currency));
    }

    /// Hex id of the account last selected, restored on the next launch.
    pub fn last_account(&self) -> Option<String> {
        self.get("last_account")
    }

    pub fn set_last_account(&mut self, hex: &str) {
        self.set("last_account", Some(hex));
    }

    /// `system`, `light` or `dark`.
    pub fn theme(&self) -> Option<String> {
        self.get("theme")
    }

    pub fn set_theme(&mut self, theme: &str) {
        self.set("theme", Some(theme));
    }

    /// Where the interface's colours come from, as the stored form of a
    /// [`crate::appearance::ColorChoice`]: `system` or `#rrggbb`.
    pub fn color(&self) -> Option<String> {
        self.get("color")
    }

    pub fn set_color(&mut self, color: &str) {
        self.set("color", Some(color));
    }

    /// What the system said its colour was the last time it was asked, so
    /// that the next launch starts in it rather than changing to it.
    pub fn system_color(&self) -> Option<String> {
        self.get("system_color")
    }

    pub fn set_system_color(&mut self, color: Option<String>) {
        self.set("system_color", color);
    }

    /// The name of the theme worn while the interface is light or dark, for
    /// someone who chose one over the colour scheme. See [`crate::themes`].
    pub fn theme_name(&self, dark: bool) -> Option<String> {
        self.get(if dark { "dark_theme" } else { "light_theme" })
    }

    pub fn set_theme_name(&mut self, dark: bool, name: Option<&str>) {
        self.set(if dark { "dark_theme" } else { "light_theme" }, name);
    }

    /// How large the interface is drawn, as the stored name of a
    /// [`crate::appearance::InterfaceSize`].
    pub fn interface_size(&self) -> Option<String> {
        self.get("interface_size")
    }

    pub fn set_interface_size(&mut self, size: &str) {
        self.set("interface_size", Some(size));
    }

    /// Whether the charts leave transfer legs out.
    pub fn hides_transfers(&self) -> bool {
        self.get("hide_transfers").unwrap_or(false)
    }

    pub fn set_hides_transfers(&mut self, hidden: bool) {
        self.set("hide_transfers", Some(hidden));
    }

    /// The styles an earlier version kept here, by the category's hex id.
    pub fn legacy_category_styles(&self) -> Vec<(String, CategoryStyle)> {
        let Some(styles) = self
            .values
            .get(LEGACY_CATEGORY_STYLES)
            .and_then(Value::as_object)
        else {
            return Vec::new();
        };
        let text = |style: &Value, key: &str| {
            style
                .get(key)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        };
        styles
            .iter()
            .map(|(hex, style)| {
                let style = CategoryStyle {
                    color: text(style, "color"),
                    icon: text(style, "icon"),
                };
                (hex.clone(), style)
            })
            .filter(|(_, style)| !style.is_empty())
            .collect()
    }

    /// Drops them, once the ledger has them.
    pub fn forget_legacy_category_styles(&mut self) {
        self.values.remove(LEGACY_CATEGORY_STYLES);
    }
}

/// A source for a ledger that is about to be created or connected.
pub fn new_source() -> u64 {
    remote::generate_source()
}

/// Temp file then rename, so a crash mid-write cannot leave half a file where
/// the credentials of the sync remote were.
fn write_atomically(file: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = file.with_extension("json.tmp");
    fs::write(&temporary, contents)?;
    fs::rename(&temporary, file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "money-manager-settings-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir.join("settings.json")
    }

    #[test]
    fn migration_drops_old_transport_settings_and_preserves_preferences() {
        let file = scratch("migration-reset");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, r#"{"source":42,"remote":{"secret_access_key":"old"},"theme":"dark","default_currency":"EUR","last_account":"aa"}"#).unwrap();
        let mut settings = Settings::load(file.clone());
        settings.clear_legacy_transport().unwrap();
        let reloaded = Settings::load(file);
        assert!(!reloaded.has_legacy_config());
        assert_eq!(reloaded.theme().as_deref(), Some("dark"));
        assert_eq!(reloaded.default_currency(), "EUR");
        assert_eq!(reloaded.last_account().as_deref(), Some("aa"));
    }

    #[test]
    fn a_failed_migration_reset_keeps_the_old_settings_for_retry() {
        let file = scratch("migration-reset-failed");
        let mut settings = Settings::load(file.clone());
        settings.values.insert("source".into(), Value::from(42));
        settings
            .values
            .insert("remote".into(), serde_json::json!({"key":"old"}));
        fs::create_dir_all(&file).unwrap();
        assert!(settings.clear_legacy_transport().is_err());
        assert_eq!(settings.source(), Some(42));
        assert_eq!(settings.values["remote"]["key"], "old");
    }

    #[test]
    fn a_first_run_has_no_ledger_and_the_default_currency() {
        let settings = Settings::load(scratch("first-run"));
        assert_eq!(settings.source(), None);
        assert!(!settings.has_legacy_config());
        assert_eq!(settings.default_currency(), "RUB");
        assert!(!settings.hides_transfers());
    }

    #[test]
    fn what_is_set_is_there_after_a_reload() {
        let file = scratch("reload");
        let mut settings = Settings::load(file.clone());
        settings.set_default_currency("EUR");
        settings.set_last_account("00ff");
        settings.save().unwrap();

        let reloaded = Settings::load(file);
        assert_eq!(reloaded.source(), None);
        assert_eq!(reloaded.default_currency(), "EUR");
        assert_eq!(reloaded.last_account().as_deref(), Some("00ff"));
        assert!(!reloaded.has_legacy_config());
    }

    #[test]
    fn styles_an_earlier_version_kept_here_are_read_and_then_dropped() {
        let file = scratch("legacy-styles");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(
            &file,
            r##"{"source":7,"category_styles":{
                "ab":{"color":"#e8853b","icon":"utensils"},
                "cd":{"icon":"car"},
                "ef":{}}}"##,
        )
        .unwrap();

        let mut settings = Settings::load(file.clone());
        let mut styles = settings.legacy_category_styles();
        styles.sort_by(|a, b| a.0.cmp(&b.0));
        // The one with nothing chosen is not a style.
        assert_eq!(styles.len(), 2);
        assert_eq!(styles[0].0, "ab");
        assert_eq!(styles[0].1.color.as_deref(), Some("#e8853b"));
        assert_eq!(styles[0].1.icon.as_deref(), Some("utensils"));
        assert_eq!(styles[1].1.color, None);
        assert_eq!(styles[1].1.icon.as_deref(), Some("car"));

        settings.forget_legacy_category_styles();
        settings.save().unwrap();
        let reloaded = Settings::load(file);
        assert!(reloaded.legacy_category_styles().is_empty());
        assert_eq!(reloaded.source(), Some(7));
    }

    #[test]
    fn a_key_from_a_later_version_survives_a_write() {
        let file = scratch("future");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, r#"{"source":7,"future":{"kept":true}}"#).unwrap();

        let mut settings = Settings::load(file.clone());
        settings.set_theme("dark");
        settings.save().unwrap();

        let raw: Value = serde_json::from_str(&fs::read_to_string(file).unwrap()).unwrap();
        assert_eq!(raw["future"]["kept"], true);
        assert_eq!(raw["theme"], "dark");
        assert_eq!(raw["source"], 7);
    }
}
