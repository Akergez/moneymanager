//! The open ledger on screen: navigation, the account, and one of three
//! screens.
//!
//! [`Workspace`] owns what the three screens share and none of them should
//! decide alone — which screen is up and whether it is about expenses or
//! income — and hands the mode down when it changes. Everything about the
//! ledger itself it leaves to [`Book`].
//!
//! Every command here is one method, reached from a key binding, a button
//! and a menu alike, so the three cannot come to mean different things.

mod render;

use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::Notification;
use gpui_kit::{AppContext, Context, Entity, FocusHandle, Subscription, Window};

use crate::book::{Book, Mode, SyncState};
use crate::categories::CategoriesView;
use crate::charts::ChartsView;
use crate::settings::Settings;
use crate::shell::Layout;
use crate::transactions::TransactionsView;
use crate::ui::Lucide;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Transactions,
    Categories,
    Charts,
}

impl Screen {
    pub const ALL: [Screen; 3] = [Screen::Transactions, Screen::Categories, Screen::Charts];

    pub fn title(self) -> &'static str {
        match self {
            Screen::Transactions => "Transactions",
            Screen::Categories => "Categories",
            Screen::Charts => "Charts",
        }
    }

    pub fn icon(self) -> Lucide {
        match self {
            Screen::Transactions => Lucide::List,
            Screen::Categories => Lucide::LayoutGrid,
            Screen::Charts => Lucide::ChartColumn,
        }
    }

    pub fn index(self) -> usize {
        Screen::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    /// What a UI scenario calls it.
    pub fn name(self) -> &'static str {
        match self {
            Screen::Transactions => "transactions",
            Screen::Categories => "categories",
            Screen::Charts => "charts",
        }
    }
}

pub struct Workspace {
    book: Entity<Book>,
    screen: Screen,
    mode: Mode,
    transactions: Entity<TransactionsView>,
    categories: Entity<CategoriesView>,
    charts: Entity<ChartsView>,
    focus: FocusHandle,
    /// The layout the last frame was drawn in, for a UI scenario to ask about.
    layout: Layout,
    /// The sync state last reported, so each outcome is announced once.
    announced: SyncState,
    _book: Subscription,
}

impl Workspace {
    pub fn new(book: Entity<Book>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mode = Mode::default();
        let transactions = cx.new(|cx| TransactionsView::new(book.clone(), mode, window, cx));
        let categories = cx.new(|cx| CategoriesView::new(book.clone(), mode, cx));
        let charts = cx.new(|cx| ChartsView::new(book.clone(), mode, cx));
        let announced = book.read(cx).sync_state().clone();
        let watch = cx.observe_in(&book, window, |this, _, window, cx| {
            this.announce_sync(window, cx);
            cx.notify();
        });
        Workspace {
            book,
            screen: Screen::Transactions,
            mode,
            transactions,
            categories,
            charts,
            focus: cx.focus_handle(),
            layout: Layout::Desktop,
            announced,
            _book: watch,
        }
    }

    pub fn book(&self) -> &Entity<Book> {
        &self.book
    }

    pub fn screen(&self) -> Screen {
        self.screen
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub fn charts(&self) -> &Entity<ChartsView> {
        &self.charts
    }

    /// Puts the keyboard in the workspace, which is what makes its shortcuts
    /// work before anything in it has been clicked.
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
    }

    pub fn show(&mut self, screen: Screen, cx: &mut Context<Self>) {
        if self.screen != screen {
            self.screen = screen;
            cx.notify();
        }
    }

    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.mode == mode {
            return;
        }
        self.mode = mode;
        self.transactions
            .update(cx, |view, cx| view.set_mode(mode, cx));
        self.categories
            .update(cx, |view, cx| view.set_mode(mode, cx));
        self.charts.update(cx, |view, cx| view.set_mode(mode, cx));
        cx.notify();
    }

    fn toggle_mode(&mut self, cx: &mut Context<Self>) {
        let other = match self.mode {
            Mode::Expense => Mode::Income,
            Mode::Income => Mode::Expense,
        };
        self.set_mode(other, cx);
    }

    /// What "new" means on the screen that is up: a category on the
    /// categories screen, a record everywhere else.
    fn new_record(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.screen {
            Screen::Categories => {
                crate::categories::open_category_dialog(&self.book, self.mode, None, window, cx)
            }
            Screen::Transactions | Screen::Charts => {
                crate::transactions::open_entry_dialog(&self.book, self.mode, None, window, cx)
            }
        }
    }

    fn new_transfer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::accounts::open_transfer_dialog(&self.book, window, cx);
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::settings_dialog::open(window, cx);
    }

    /// Syncs with the remote. With none set up, the place to set one up is
    /// what opens instead: the command still leads somewhere.
    fn sync_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (remote, currency) = {
            let settings = Settings::global(cx);
            (settings.remote(), settings.default_currency())
        };
        match remote {
            Some(remote) => self
                .book
                .update(cx, |book, cx| book.sync(remote, currency, cx)),
            None => self.open_settings(window, cx),
        }
    }

    /// Says how a sync ended, once. While it runs the button itself shows it.
    fn announce_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let state = self.book.read(cx).sync_state().clone();
        if state == self.announced {
            return;
        }
        self.announced = state.clone();
        match state {
            SyncState::Idle | SyncState::Running => {}
            SyncState::Done { pulled, pushed } => window.push_notification(
                Notification::success(format!("Synced: {pulled} received, {pushed} sent")),
                cx,
            ),
            SyncState::Failed(message) => {
                window.push_notification(Notification::error(message), cx)
            }
        }
    }
}
