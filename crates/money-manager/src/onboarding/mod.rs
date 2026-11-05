//! The first-run question: a new ledger, or one that already syncs somewhere.
//!
//! A new ledger gets its default account. A connected ledger first reads the
//! remote history, then creates the account only if one was not received.
//! Both S3 and HTTP settings are written to `.tresse/remotes.toml`; strings
//! use the same `tresse://` format as the CLI and other clients.

mod connect_form;
mod render;

use gpui_adaptive::{FieldEvent, Purpose, TextFieldState};
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

/// The two ways of saying where the storage is, and the server page that is
/// the first one offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConnectBy {
    Server,
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
    currency: Entity<TextFieldState>,
    account_name: Entity<TextFieldState>,
    /// The server the sign-in page opens on; the build's default, or any
    /// other one typed over it.
    server_url: Entity<TextFieldState>,
    config_string: Entity<TextFieldState>,
    remote: RemoteFields,
    /// A remote the server page just returned: shown in the fields at the
    /// next render, which has a window and `set_value` wants one.
    pending_fill: Option<RemoteConfig>,
    connecting: Option<Connecting>,
    error: Option<SharedString>,
    /// Back, past the first step: to the first step.
    back: Option<gpui_android_glue::back::Guard>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<OnboardingEvent> for Onboarding {}

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
        let default_currency = Settings::global(cx).default_currency();
        let mut field = |purpose: Purpose, value: &str, placeholder: &'static str| {
            let value = value.to_string();
            cx.new(|cx| {
                let mut state = TextFieldState::new(purpose, window, cx);
                state.set_placeholder(placeholder, window, cx);
                state.set_value(value, window, cx);
                state
            })
        };
        let account_name = field(Purpose::Message, "Основной", "Account name");
        let currency = field(Purpose::Plain, &default_currency, "USD");
        let server_url = field(
            Purpose::Address,
            crate::sync_server::default_server_url().unwrap_or_default(),
            "https://sync.example.com",
        );
        let config_string = field(Purpose::Address, "", "tresse://…");
        let remote = RemoteFields::new(None, window, cx);

        // Enter in any field is the same as the step's button.
        let mut subscriptions: Vec<Subscription> = remote
            .fields()
            .into_iter()
            .chain([&account_name, &currency, &server_url, &config_string])
            .map(|field| {
                cx.subscribe(field, |this, _, event: &FieldEvent, cx| {
                    if matches!(event, FieldEvent::Submit) {
                        this.submit(cx);
                    }
                })
            })
            .collect();
        subscriptions.push(cx.observe_global::<crate::sync_server::SyncConnect>(
            |this, cx| this.server_connect_changed(cx),
        ));
        subscriptions.shrink_to_fit();

        Onboarding {
            step: Step::Choose,
            connect_by: ConnectBy::Server,
            currency,
            account_name,
            server_url,
            config_string,
            remote,
            pending_fill: None,
            connecting: None,
            error: None,
            back: None,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) fn step(&self) -> Step {
        self.step
    }

    pub(crate) fn is_connecting(&self) -> bool {
        self.connecting.is_some()
    }

    pub(crate) fn error(&self) -> Option<&SharedString> {
        self.error.as_ref()
    }

    fn go_to(&mut self, step: Step, cx: &mut Context<Self>) {
        if self.is_connecting() {
            return;
        }
        self.step = step;
        self.error = None;
        self.back = (step != Step::Choose).then(|| {
            let this = cx.weak_entity();
            gpui_android_glue::back::push(move |_, cx| {
                // Not while the first sync runs: there is no going back then.
                this.update(cx, |this, cx| {
                    let back = !this.is_connecting();
                    this.go_to(Step::Choose, cx);
                    back
                })
                .unwrap_or(false)
            })
        });
        cx.notify();
    }

    /// What Enter means on the current step.
    fn submit(&mut self, cx: &mut Context<Self>) {
        match self.step {
            Step::Choose => {}
            Step::Create => self.create(cx),
            Step::Connect => match self.connect_by {
                ConnectBy::Server => self.open_server_page(cx),
                ConnectBy::ConfigString | ConnectBy::Details => self.connect(cx),
            },
        }
    }

    /// Opens the server's sign-in page in the browser; its return is matched
    /// against the attempt in [`crate::sync_server::SyncConnect`] and arrives
    /// through `server_connect_changed`.
    fn open_server_page(&mut self, cx: &mut Context<Self>) {
        if crate::sync_server::SyncConnect::waiting(cx) {
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
        if self.step != Step::Connect || self.connect_by != ConnectBy::Server {
            return;
        }
        let Some(outcome) = crate::sync_server::SyncConnect::take_outcome(cx) else {
            // Only the waiting state changed.
            cx.notify();
            return;
        };
        match outcome {
            Ok(remote) => {
                self.pending_fill = Some(remote.clone());
                self.connect_by = ConnectBy::Details;
                self.connect_with(remote, cx);
            }
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
            }
        }
    }

    fn create(&mut self, cx: &mut Context<Self>) {
        let currency = self.currency.read(cx).value(cx).to_string();
        let name = self.account_name.read(cx).value(cx).to_string();
        match create_ledger(&name, &currency, cx) {
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
            // A string the server page ended with carries no key for a new
            // repository: one is generated here, as for any pasted string.
            ConnectBy::ConfigString => {
                let text = self.config_string.read(cx).value(cx);
                if text.trim().is_empty() {
                    return Err("Paste the config string first.".to_string());
                }
                crate::sync_server::import_share_string(&text, None)?
            }
            ConnectBy::Server => {
                return Err("Finish signing in in the browser, or pick another way.".to_string())
            }
            ConnectBy::Details => self.remote.read(cx)?,
        };
        remote.validate()?;
        Ok(remote)
    }

    fn connect(&mut self, cx: &mut Context<Self>) {
        let remote = match self.remote_from_form(cx) {
            Ok(remote) => remote,
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
                return;
            }
        };
        self.connect_with(remote, cx);
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
