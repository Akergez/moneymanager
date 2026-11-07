//! The first-run question: a ledger on this device only, or one that syncs.
//!
//! One page with two tabs. The first is the short way — start without sync,
//! sign in on a Tresse server (the build's default, or another one), or
//! paste a config string. The second describes a storage field by field:
//! a Tresse repository on S3 or on an HTTP server.
//!
//! A new ledger gets its default account. A connected ledger first reads the
//! remote history, then creates the account only if one was not received.
//! The storage is written to `.tresse/remotes.toml`; strings use the same
//! `tresse://` format as the CLI and other clients.

mod connect_form;
mod render;

use gpui_adaptive::{FieldEvent, Purpose, TextFieldState};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, SharedString, Subscription, Window,
};
use money_core::remote::RemoteConfig;

use crate::book::{Book, SyncState};
use crate::settings::{self, Settings};

pub use connect_form::{RemoteFields, describe as describe_remote};

pub enum OnboardingEvent {
    /// A native ledger is open.
    Ready(Entity<Book>),
}

/// The two pages of the first-run question. `Server` is the short way:
/// a ledger without sync, or one through a Tresse server (signed in in the
/// browser, or by a config string). `Custom` is every field of a storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Server,
    Custom,
}

impl Tab {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Tab::Server => "server",
            Tab::Custom => "custom",
        }
    }

    pub(crate) fn from_key(key: &str) -> Self {
        if key == "custom" {
            Tab::Custom
        } else {
            Tab::Server
        }
    }
}

/// Where the storage being connected to is described.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Described {
    ConfigString,
    Fields,
}

/// The ledger being filled by the first sync.
struct Connecting {
    book: Entity<Book>,
    _watch: Subscription,
}

pub struct Onboarding {
    tab: Tab,
    /// The server the sign-in page opens on: the build's default, or any
    /// other one typed over it.
    server_url: Entity<TextFieldState>,
    config_string: Entity<TextFieldState>,
    remote: RemoteFields,
    /// A remote the server page just returned: shown in the fields at the
    /// next render, which has a window and `set_value` wants one.
    pending_fill: Option<RemoteConfig>,
    connecting: Option<Connecting>,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<OnboardingEvent> for Onboarding {}

/// The name of the account a ledger started without sync begins with; it
/// can be renamed, and its currency changed, like any other account.
pub const FIRST_ACCOUNT: &str = "Основной";

/// Makes a new, empty ledger with its default account.
pub fn create_ledger(name: &str, currency: &str, cx: &mut App) -> Result<Entity<Book>, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Account name is required.".into());
    }
    let currency = currency.trim().to_uppercase();
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err("Currency is a three-letter code, like USD.".to_string());
    }
    let source = settings::new_source();
    let mut book = Book::open(&crate::paths::ledger_dir(), source, None)?;
    book.create_first_account(name, &currency)?;
    Settings::update(cx, |settings| {
        settings.set_default_currency(&currency);
    });
    Ok(cx.new(|_| book))
}

impl Onboarding {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut field = |purpose: Purpose, value: &str, placeholder: &'static str| {
            let value = value.to_string();
            cx.new(|cx| {
                let mut state = TextFieldState::new(purpose, window, cx);
                state.set_placeholder(placeholder, window, cx);
                state.set_value(value, window, cx);
                state
            })
        };
        let server_url = field(
            Purpose::Address,
            crate::sync_server::default_server_url().unwrap_or_default(),
            "https://…",
        );
        let config_string = field(Purpose::Address, "", "tresse://…");
        let remote = RemoteFields::new(None, window, cx);

        // Enter in a field does what the button under it does.
        let on_enter = |cx: &mut Context<Self>,
                        field: &Entity<TextFieldState>,
                        run: fn(&mut Self, &mut Context<Self>)| {
            cx.subscribe(field, move |this, _, event: &FieldEvent, cx| {
                if matches!(event, FieldEvent::Submit) {
                    run(this, cx);
                }
            })
        };
        let mut subscriptions = vec![
            on_enter(cx, &server_url, Self::open_server_page),
            on_enter(cx, &config_string, |this, cx| {
                this.connect(Described::ConfigString, cx)
            }),
        ];
        for field in remote.fields() {
            subscriptions.push(on_enter(cx, field, |this, cx| {
                this.connect(Described::Fields, cx)
            }));
        }
        subscriptions.push(
            cx.observe_global::<crate::sync_server::SyncConnect>(|this, cx| {
                this.server_connect_changed(cx)
            }),
        );

        Onboarding {
            tab: Tab::Server,
            server_url,
            config_string,
            remote,
            pending_fill: None,
            connecting: None,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn tab(&self) -> Tab {
        self.tab
    }

    pub(crate) fn is_connecting(&self) -> bool {
        self.connecting.is_some()
    }

    #[cfg(test)]
    pub(crate) fn error(&self) -> Option<&SharedString> {
        self.error.as_ref()
    }

    fn show_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        if self.tab != tab {
            self.tab = tab;
            self.error = None;
            cx.notify();
        }
    }

    /// Opens the server's sign-in page in the browser; its return is matched
    /// against the attempt in [`crate::sync_server::SyncConnect`] and arrives
    /// through `server_connect_changed`.
    fn open_server_page(&mut self, cx: &mut Context<Self>) {
        if self.is_connecting() || crate::sync_server::SyncConnect::waiting(cx) {
            return;
        }
        let server = self.server_url.read(cx).value(cx).to_string();
        match crate::sync_server::SyncConnect::begin(cx, &server) {
            Ok(page) => {
                gpui_android_glue::open_url(&page, cx);
                self.error = None;
            }
            Err(error) => self.error = Some(error.into()),
        }
        cx.notify();
    }

    /// The server page's return (or its refusal): proceed to the first sync,
    /// or say why not.
    fn server_connect_changed(&mut self, cx: &mut Context<Self>) {
        let Some(outcome) = crate::sync_server::SyncConnect::take_outcome(cx) else {
            // Only the waiting state changed.
            cx.notify();
            return;
        };
        match outcome {
            Ok(remote) => {
                // What came back is in the full form too, to be looked at
                // or corrected if the first sync fails.
                self.pending_fill = Some(remote.clone());
                self.connect_with(remote, cx);
            }
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
            }
        }
    }

    /// A new ledger on this device only, with its first account.
    fn start_without_sync(&mut self, cx: &mut Context<Self>) {
        if self.is_connecting() {
            return;
        }
        crate::sync_server::SyncConnect::cancel(cx);
        let currency = Settings::global(cx).default_currency();
        match create_ledger(FIRST_ACCOUNT, &currency, cx) {
            Ok(book) => cx.emit(OnboardingEvent::Ready(book)),
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
            }
        }
    }

    fn generate_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.error = self.remote.generate_key(window, cx).err().map(Into::into);
        cx.notify();
    }

    /// The remote the config string or the fields describe, checked.
    fn remote_from(&self, described: Described, cx: &App) -> Result<RemoteConfig, String> {
        let remote = match described {
            // A string the server page ended with carries no key for a new
            // repository: one is generated here, as for any pasted string.
            Described::ConfigString => {
                let text = self.config_string.read(cx).value(cx);
                if text.trim().is_empty() {
                    return Err("Paste the config string first.".to_string());
                }
                crate::sync_server::import_share_string(&text, None)?
            }
            Described::Fields => self.remote.read(cx)?,
        };
        remote.validate()?;
        Ok(remote)
    }

    fn connect(&mut self, described: Described, cx: &mut Context<Self>) {
        if self.is_connecting() {
            return;
        }
        match self.remote_from(described, cx) {
            Ok(remote) => {
                crate::sync_server::SyncConnect::cancel(cx);
                self.connect_with(remote, cx);
            }
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
            }
        }
    }

    fn connect_with(&mut self, remote: RemoteConfig, cx: &mut Context<Self>) {
        if self.is_connecting() {
            return;
        }
        let source = settings::new_source();
        let book = match Book::open(&crate::paths::ledger_dir(), source, None) {
            Ok(book) => cx.new(|_| book),
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
                return;
            }
        };
        let currency = Settings::global(cx).default_currency();
        let watch = cx.observe(&book, |this, book, cx| this.first_sync_changed(book, cx));
        if let Err(error) =
            money_core::tresse::config::write_remote(&crate::paths::ledger_dir(), Some(&remote))
        {
            self.error = Some(error.into());
            cx.notify();
            return;
        }
        book.update(cx, |book, cx| book.sync(currency, cx));
        self.error = None;
        self.connecting = Some(Connecting {
            book,
            _watch: watch,
        });
        cx.notify();
    }
    fn first_sync_changed(&mut self, book: Entity<Book>, cx: &mut Context<Self>) {
        match book.read(cx).sync_state().clone() {
            SyncState::Idle | SyncState::Running => {}
            SyncState::Done { .. } => {
                let Some(connecting) = self.connecting.take() else {
                    return;
                };
                cx.emit(OnboardingEvent::Ready(connecting.book));
            }
            SyncState::Failed(message) => {
                // Tresse keeps fetched objects and its recovery journal on disk.
                self.connecting = None;
                self.error = Some(message);
                cx.notify();
            }
        }
    }
}
