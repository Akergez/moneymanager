//! The first-run question: a new ledger, or one that already syncs somewhere.
//!
//! A new ledger gets its default account. A connected ledger first reads the
//! remote history, then creates the account only if one was not received.
//! Both S3 and HTTP settings are written to `.tresse/remotes.toml`; strings
//! use the same `tresse://` format as the CLI and other clients.

mod connect_form;
mod render;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, SharedString, Subscription, Window,
};
use money_core::remote::RemoteConfig;

use crate::book::{Book, SyncState};
use crate::settings::{self, Settings};

pub use connect_form::RemoteFields;

pub enum OnboardingEvent {
    /// A native ledger is open.
    Ready(Entity<Book>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Step {
    Choose,
    Create,
    Connect,
}

/// The two ways of saying where the storage is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectBy {
    ConfigString,
    Details,
}

/// The ledger being filled by the first sync.
struct Connecting {
    book: Entity<Book>,
    _watch: Subscription,
}

pub struct Onboarding {
    step: Step,
    connect_by: ConnectBy,
    currency: Entity<InputState>,
    config_string: Entity<InputState>,
    remote: RemoteFields,
    connecting: Option<Connecting>,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<OnboardingEvent> for Onboarding {}

/// Makes a new, empty ledger with its default account.
pub fn create_ledger(currency: &str, cx: &mut App) -> Result<Entity<Book>, String> {
    let currency = currency.trim().to_uppercase();
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err("Currency is a three-letter code, like USD.".to_string());
    }
    let source = settings::new_source();
    let mut book = Book::open(&crate::paths::ledger_dir(), source, None)?;
    book.ensure_default_account(&currency)?;
    Settings::update(cx, |settings| {
        settings.set_default_currency(&currency);
    });
    Ok(cx.new(|_| book))
}

impl Onboarding {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let default_currency = Settings::global(cx).default_currency();
        let currency = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(default_currency)
                .placeholder("USD")
        });
        let config_string = cx.new(|cx| InputState::new(window, cx).placeholder("tresse://…"));
        let remote = RemoteFields::new(None, window, cx);

        // Enter in any field is the same as the step's button.
        let mut subscriptions: Vec<Subscription> = remote
            .inputs()
            .into_iter()
            .chain([&currency, &config_string])
            .map(|input| {
                cx.subscribe_in(input, window, |this, _, event: &InputEvent, _, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        this.submit(cx);
                    }
                })
            })
            .collect();
        subscriptions.shrink_to_fit();

        Onboarding {
            step: Step::Choose,
            connect_by: ConnectBy::ConfigString,
            currency,
            config_string,
            remote,
            connecting: None,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn step(&self) -> Step {
        self.step
    }

    pub(crate) fn is_connecting(&self) -> bool {
        self.connecting.is_some()
    }

    fn go_to(&mut self, step: Step, cx: &mut Context<Self>) {
        if self.is_connecting() {
            return;
        }
        self.step = step;
        self.error = None;
        cx.notify();
    }

    /// What Enter means on the current step.
    fn submit(&mut self, cx: &mut Context<Self>) {
        match self.step {
            Step::Choose => {}
            Step::Create => self.create(cx),
            Step::Connect => self.connect(cx),
        }
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let currency = self.currency.read(cx).value().to_string();
        match create_ledger(&currency, cx) {
            Ok(book) => cx.emit(OnboardingEvent::Ready(book)),
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
            }
        }
    }

    /// The remote the form currently describes, checked.
    fn remote_from_form(&self, cx: &App) -> Result<RemoteConfig, String> {
        let remote = match self.connect_by {
            ConnectBy::ConfigString => {
                let text = self.config_string.read(cx).value();
                if text.trim().is_empty() {
                    return Err("Paste the config string first.".to_string());
                }
                RemoteConfig::from_share_string(&text).map_err(|_| {
                    "That is not a config string. It starts with tresse://".to_string()
                })?
            }
            ConnectBy::Details => self.remote.read(cx)?,
        };
        remote.validate()?;
        Ok(remote)
    }

    fn connect(&mut self, cx: &mut Context<Self>) {
        if self.is_connecting() {
            return;
        }
        let remote = match self.remote_from_form(cx) {
            Ok(remote) => remote,
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
                return;
            }
        };
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
