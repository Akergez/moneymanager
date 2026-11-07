//! The open ledger on screen: navigation, the account, and one of four
//! screens — and over a screen, the page of one category's or one tag's
//! records, which back leaves.
//!
//! [`Workspace`] owns what the three screens share and none of them should
//! decide alone — which screen is up and whether it is about expenses or
//! income — and hands the mode down when it changes. Everything about the
//! ledger itself it leaves to [`Book`].
//!
//! Every command here is one method, reached from a key binding, a button
//! and a menu alike, so the three cannot come to mean different things.

mod render;

use std::rc::Rc;

use gpui_adaptive::{NavigatorState, Notice, Route, Tone, WidthClass};
use gpui_kit::{AppContext, Context, Entity, FocusHandle, Subscription, Window};

use crate::book::{Book, Filter, Mode, SyncState};
use crate::categories::{CategoriesView, Kind};
use crate::charts::ChartsView;
use crate::settings::Settings;
use crate::transactions::TransactionsView;
use crate::ui::Lucide;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Transactions,
    Categories,
    Charts,
    Tags,
}

impl Section {
    pub const ALL: [Section; 4] = [
        Section::Transactions,
        Section::Categories,
        Section::Charts,
        Section::Tags,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Section::Transactions => "Transactions",
            Section::Categories => "Categories",
            Section::Charts => "Charts",
            Section::Tags => "Tags",
        }
    }

    pub fn icon(self) -> Lucide {
        match self {
            Section::Transactions => Lucide::List,
            Section::Categories => Lucide::LayoutGrid,
            Section::Charts => Lucide::ChartColumn,
            Section::Tags => Lucide::Tags,
        }
    }

    /// What a UI scenario calls it.
    pub fn name(self) -> &'static str {
        match self {
            Section::Transactions => "transactions",
            Section::Categories => "categories",
            Section::Charts => "charts",
            Section::Tags => "tags",
        }
    }
}

/// A page of the workspace: a screen of its navigation, or the records of
/// one category or one tag, opened from the screen that lists them.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    Section(Section),
    Records(Mode, Filter),
}

impl Route for Place {
    /// The charts' bars scroll sideways; a swipe there is theirs.
    fn scrolls_sideways(&self) -> bool {
        matches!(self, Place::Section(Section::Charts))
    }
}

pub struct Workspace {
    book: Entity<Book>,
    screen: Section,
    mode: Mode,
    transactions: Entity<TransactionsView>,
    categories: Entity<CategoriesView>,
    tags: Entity<CategoriesView>,
    charts: Entity<ChartsView>,
    /// The screen up, and the page over it.
    navigator: Entity<NavigatorState<Place>>,
    /// The records of the page over the screen, while there is one.
    records: Option<Entity<TransactionsView>>,
    focus: FocusHandle,
    /// The width the last frame was drawn at, for a UI scenario to ask about.
    width: WidthClass,
    /// The sync state last reported, so each outcome is announced once.
    announced: SyncState,
    _book: Subscription,
}

impl Workspace {
    fn csv_file(&mut self, export: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.book.read(cx).is_busy() {
            Notice::error("The ledger is busy. Try again when it finishes.").push(window, cx);
            return;
        }
        let book = self.book.clone();
        #[cfg(not(target_os = "android"))]
        let save = export.then(|| {
            cx.prompt_for_new_path(
                &dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from(".")),
                Some("transactions.csv"),
            )
        });
        let open = (!export).then(|| {
            cx.prompt_for_paths(gpui_kit::PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some("Import CSV".into()),
            })
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = async {
                let path = if export {
                    #[cfg(target_os = "android")]
                    {
                        None
                    }
                    #[cfg(not(target_os = "android"))]
                    {
                        let Some(save) = save else {
                            return Ok(None);
                        };
                        save.await
                            .map_err(|e| e.to_string())?
                            .map_err(|e| e.to_string())?
                    }
                } else if let Some(open) = open {
                    open.await
                        .map_err(|e| e.to_string())?
                        .map_err(|e| e.to_string())?
                        .and_then(|paths| paths.into_iter().next())
                } else {
                    None
                };
                // Android saves directly to a SAF URI, without a filesystem path.
                if path.is_none() && !(export && cfg!(target_os = "android")) {
                    return Ok(None);
                }
                let task = book.update(cx, |book, cx| book.csv_file(path, export, cx))?;
                task.await
            }
            .await;
            this.update_in(cx, |_, window, cx| match result {
                Ok(Some(report)) => Notice::new(if export {
                    "CSV exported".to_string()
                } else if report.skipped == 0 {
                    format!("Imported {} transactions", report.added)
                } else {
                    format!(
                        "Imported {} transactions. {} were already in the ledger.",
                        report.added, report.skipped
                    )
                })
                .tone(Tone::Success)
                .push(window, cx),
                Ok(None) => {}
                Err(error) => Notice::error(error).push(window, cx),
            })
            .ok();
        })
        .detach();
    }

    pub fn new(book: Entity<Book>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mode = Mode::default();
        let transactions = cx.new(|cx| TransactionsView::new(book.clone(), mode, None, cx));
        let workspace = cx.weak_entity();
        let open_records: crate::categories::OpenRecords = Rc::new(move |mode, filter, _, cx| {
            workspace
                .update(cx, |this, cx| this.open_records(mode, filter, cx))
                .ok();
        });
        let categories = cx.new(|cx| {
            CategoriesView::new(book.clone(), mode, Kind::Category, open_records.clone(), cx)
        });
        let tags =
            cx.new(|cx| CategoriesView::new(book.clone(), mode, Kind::Tag, open_records, cx));
        let charts = cx.new(|cx| ChartsView::new(book.clone(), mode, cx));
        let navigator = cx.new(|_| NavigatorState::new(Place::Section(Section::Transactions)));
        cx.observe(&navigator, |_, _, cx| cx.notify()).detach();
        // Selecting a row is what makes Edit available, and Edit is drawn
        // here.
        cx.observe(&transactions, |_, _, cx| cx.notify()).detach();
        let announced = book.read(cx).sync_state().clone();
        let watch = cx.observe_in(&book, window, |this, _, window, cx| {
            this.announce_sync(window, cx);
            this.leave_missing_page(cx);
            cx.notify();
        });
        Workspace {
            book,
            screen: Section::Transactions,
            mode,
            transactions,
            categories,
            tags,
            charts,
            navigator,
            records: None,
            focus: cx.focus_handle(),
            width: WidthClass::Expanded,
            announced,
            _book: watch,
        }
    }

    pub fn book(&self) -> &Entity<Book> {
        &self.book
    }

    pub fn screen(&self) -> Section {
        self.screen
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn width(&self) -> WidthClass {
        self.width
    }

    pub fn charts(&self) -> &Entity<ChartsView> {
        &self.charts
    }

    /// Puts the keyboard in the workspace, which is what makes its shortcuts
    /// work before anything in it has been clicked.
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
    }

    /// The page on top: a screen, or the records over one.
    pub fn place(&self, cx: &gpui_kit::App) -> Place {
        self.navigator.read(cx).top().clone()
    }

    /// The records of the page over the screen, while one is up.
    pub fn records_page(&self, cx: &gpui_kit::App) -> Option<&Entity<TransactionsView>> {
        match self.place(cx) {
            Place::Records(..) => self.records.as_ref(),
            Place::Section(_) => None,
        }
    }

    /// Goes to a screen; a page open over the last one is closed.
    pub fn show(&mut self, screen: Section, cx: &mut Context<Self>) {
        self.screen = screen;
        self.navigator.update(cx, |navigator, cx| {
            navigator.replace_root(Place::Section(screen), cx)
        });
        cx.notify();
    }

    /// Opens the page of the records `filter` admits, over the screen that
    /// is up.
    pub fn open_records(&mut self, mode: Mode, filter: Filter, cx: &mut Context<Self>) {
        let book = self.book.clone();
        let view = {
            let filter = filter.clone();
            cx.new(|cx| TransactionsView::new(book, mode, Some(filter), cx))
        };
        cx.observe(&view, |_, _, cx| cx.notify()).detach();
        self.records = Some(view);
        let root = Place::Section(self.screen);
        self.navigator.update(cx, |navigator, cx| {
            navigator.set_stack(vec![root, Place::Records(mode, filter)], cx)
        });
    }

    /// Back, from the page of records to its screen.
    fn go_back(&mut self, cx: &mut Context<Self>) {
        self.navigator.update(cx, |navigator, cx| {
            navigator.back(cx);
        });
    }

    /// A page of a category or a tag that is no longer there (a tag
    /// deleted, here or on another device) has nothing to show: back.
    fn leave_missing_page(&mut self, cx: &mut Context<Self>) {
        let Place::Records(mode, filter) = self.place(cx) else {
            return;
        };
        let book = self.book.read(cx);
        let there = match &filter {
            Filter::Category(id) => book.category(mode, id).is_some(),
            Filter::Tag(id) => book.tag(mode, id).is_some(),
        };
        if !there {
            self.go_back(cx);
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
        self.tags.update(cx, |view, cx| view.set_mode(mode, cx));
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

    /// What "new" means on the page that is up: a category on the
    /// categories screen, a tag on the tags screen, a record in the category
    /// or with the tag on the page of one, and a record everywhere else.
    fn new_record(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let book = &self.book;
        match (self.place(cx), self.screen) {
            (Place::Records(mode, filter), _) => {
                crate::transactions::open_new_entry_in(book, mode, &filter, window, cx)
            }
            (_, Section::Categories) => {
                crate::categories::open_category_dialog(book, self.mode, None, window, cx)
            }
            (_, Section::Tags) => {
                crate::categories::open_tag_dialog(book, self.mode, None, window, cx)
            }
            (_, Section::Transactions | Section::Charts) => {
                crate::transactions::open_entry_dialog(book, self.mode, None, window, cx)
            }
        }
    }

    /// The list of records on the page that is up, if it has one.
    fn active_records(&self, cx: &gpui_kit::App) -> Option<Entity<TransactionsView>> {
        match self.place(cx) {
            Place::Records(..) => self.records.clone(),
            Place::Section(Section::Transactions) => Some(self.transactions.clone()),
            Place::Section(_) => None,
        }
    }

    /// Whether there is something for Edit to open: a selected row of a
    /// list of records. The other screens open what is pressed, and have no
    /// selection.
    fn can_edit(&self, cx: &gpui_kit::App) -> bool {
        self.active_records(cx)
            .is_some_and(|records| records.read(cx).has_selection(cx))
    }

    fn edit_record(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(records) = self.active_records(cx).filter(|_| self.can_edit(cx)) {
            records.update(cx, |view, cx| view.open_selected(window, cx));
        }
    }

    /// Moves the list's selection a row up or down: the keyboard's way
    /// through the records, Enter opening the one it is on.
    fn step_selection(&mut self, forward: bool, cx: &mut Context<Self>) {
        if let Some(records) = self.active_records(cx) {
            records.update(cx, |view, cx| view.step_selection(forward, cx));
        }
    }

    fn new_transfer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::accounts::open_transfer_dialog(&self.book, window, cx);
    }

    fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        crate::settings_dialog::open(Some(self.book.clone()), window, cx);
    }

    /// Syncs with the remote. With none set up, the place to set one up is
    /// what opens instead: the command still leads somewhere.
    fn sync_now(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (remote, currency) = {
            let settings = Settings::global(cx);
            (settings.remote(), settings.default_currency())
        };
        match remote {
            Ok(Some(_)) => self.book.update(cx, |book, cx| book.sync(currency, cx)),
            Ok(None) => self.open_settings(window, cx),
            Err(error) => Notice::error(error).push(window, cx),
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
            SyncState::Done { pulled, pushed } => {
                Notice::new(format!("Synced: {pulled} received, {pushed} sent"))
                    .tone(Tone::Success)
                    .push(window, cx)
            }
            SyncState::Failed(message) => Notice::error(message).push(window, cx),
        }
    }
}
