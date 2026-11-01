use gpui_adaptive::{AppFrame, Command, Empty, Notice, Tone};
use gpui_kit::prelude::*;
use gpui_kit::{Context, Entity, SharedString, Subscription, Window};

use crate::book::Book;
use crate::onboarding::{self, Onboarding, OnboardingEvent};
use crate::settings::Settings;
use crate::ui::Lucide;
use crate::workspace::Workspace;

/// What the window is showing.
pub enum Stage {
    /// Nothing has been decided yet: the window has only just opened.
    Starting,
    Upgrade,
    Converting,
    /// No ledger is set up here, so ask which there should be.
    Welcome(Entity<Onboarding>),
    Workspace(Entity<Workspace>),
    /// There is a ledger, and it would not open.
    Failed(SharedString),
}

pub struct Shell {
    stage: Stage,
    /// The system switching between light and dark, which a "System" theme
    /// has to follow while the window is open.
    _appearance: Subscription,
    /// The window coming back to the front, which is when a phone's colours
    /// may have changed: nothing announces a new wallpaper.
    _activation: Subscription,
    _onboarding: Option<Subscription>,
}

impl Shell {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Only now is there a window to ask what the desktop looks like.
        crate::appearance::apply(Some(window), cx);
        let appearance = window
            .observe_window_appearance(|window, cx| crate::appearance::apply(Some(window), cx));
        let activation = cx.observe_window_activation(window, |_, window, cx| {
            if window.is_window_active() {
                gpui_adaptive_colors::refresh(cx);
            }
        });
        Shell {
            stage: Stage::Starting,
            _appearance: appearance,
            _activation: activation,
            _onboarding: None,
        }
    }

    pub fn stage(&self) -> &Stage {
        &self.stage
    }

    /// Decides what this window is for. Runs once, right after it opens.
    pub(super) fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (last_account, currency) = {
            let settings = Settings::global(cx);
            (settings.last_account(), settings.default_currency())
        };

        let root = crate::paths::ledger_dir();
        let legacy = money_core::store::has_legacy_data(&crate::paths::legacy_ledger_dir());
        match legacy {
            Err(error) => {
                self.stage = Stage::Failed(error.into());
                cx.notify();
                return;
            }
            Ok(has_data)
                if has_data
                    || (root.join(money_core::store::LEDGER_FILE).exists()
                        && Settings::global(cx).has_legacy_config()) =>
            {
                self.stage = Stage::Upgrade;
                cx.notify();
                return;
            }
            _ => {}
        }
        if Settings::global(cx).has_legacy_config()
            && let Err(error) = Settings::finish_migration(cx)
        {
            self.stage = Stage::Failed(error.into());
            cx.notify();
            return;
        }
        if root.join(money_core::store::LEDGER_FILE).is_file() {
            let source = crate::settings::new_source();
            // A ledger was set up here before. It always has its default
            // account, as the terminal version made sure of on every launch.
            let opened = Book::open(&crate::paths::ledger_dir(), source, last_account).and_then(
                |mut book| {
                    book.ensure_default_account(&currency)?;
                    Ok(book)
                },
            );
            match opened {
                Ok(book) => {
                    let book = cx.new(|_| book);
                    // What an earlier version kept beside the ledger now
                    // belongs in it.
                    book.update(cx, |book, cx| book.adopt_legacy_styles(cx));
                    self.show_workspace(book, window, cx);
                }
                Err(error) => {
                    tracing::error!(%error, "could not open the ledger");
                    self.stage = Stage::Failed(error.into());
                    cx.notify();
                }
            }
            return;
        }

        // A layout-only mode, so the screens can be reviewed, and scripted,
        // without anybody answering the first-run question.
        if crate::demo_requested() {
            match onboarding::create_ledger("Основной", &currency, cx) {
                Ok(book) => {
                    let today = crate::today();
                    if let Err(error) = book.update(cx, |book, cx| book.seed_demo(today, cx)) {
                        tracing::error!(%error, "could not write the sample records");
                    }
                    self.show_workspace(book, window, cx);
                }
                Err(error) => {
                    self.stage = Stage::Failed(error.into());
                    cx.notify();
                }
            }
            return;
        }

        let onboarding = cx.new(|cx| Onboarding::new(window, cx));
        self._onboarding = Some(cx.subscribe_in(
            &onboarding,
            window,
            |shell, _, event: &OnboardingEvent, window, cx| {
                let OnboardingEvent::Ready(book) = event;
                shell.show_workspace(book.clone(), window, cx);
            },
        ));
        self.stage = Stage::Welcome(onboarding);
        cx.notify();
    }

    fn convert(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stage = Stage::Converting;
        cx.notify();
        let source = Settings::global(cx)
            .source()
            .unwrap_or_else(crate::settings::new_source);
        let last_account = Settings::global(cx).last_account();
        let currency = Settings::global(cx).default_currency();
        cx.spawn_in(window, async move |shell, cx| {
            let result = cx
                .background_spawn(async move {
                    let old = crate::paths::legacy_ledger_dir();
                    let root = crate::paths::ledger_dir();
                    if money_core::store::has_legacy_data(&old)? {
                        money_core::store::migrate_legacy(&old, &root, source)?;
                    }
                    money_core::tresse::config::write_remote(&root, None)?;
                    let mut book = Book::open(&root, crate::settings::new_source(), last_account)?;
                    book.ensure_default_account(&currency)?;
                    if old.exists() {
                        let base = crate::paths::legacy_backup_dir();
                        let mut backup = base.clone();
                        let mut suffix = 1;
                        while backup.exists() {
                            backup = base.with_extension(format!("pre-tresse.{suffix}"));
                            suffix += 1;
                        }
                        std::fs::rename(&old, &backup)
                            .map_err(|e| format!("could not keep the legacy backup: {e}"))?;
                    }
                    Ok::<_, String>(book)
                })
                .await;
            shell
                .update_in(cx, |shell, window, cx| {
                    let result = result.and_then(|book| {
                        Settings::finish_migration(cx)?;
                        Ok(book)
                    });
                    match result {
                        Ok(book) => {
                            let book = cx.new(|_| book);
                            book.update(cx, |book, cx| book.adopt_legacy_styles(cx));
                            shell.show_workspace(book, window, cx);
                            Notice::new("Data converted to Tresse. Sync settings were reset.")
                                .tone(Tone::Success)
                                .push(window, cx);
                            crate::settings_dialog::open(window, cx);
                        }
                        Err(error) => {
                            shell.stage = Stage::Failed(error.into());
                            cx.notify();
                        }
                    }
                })
                .ok();
        })
        .detach();
    }

    fn show_workspace(&mut self, book: Entity<Book>, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = cx.new(|cx| Workspace::new(book, window, cx));
        workspace.update(cx, |workspace, cx| workspace.focus(window, cx));
        self._onboarding = None;
        self.stage = Stage::Workspace(workspace);
        cx.notify();
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let frame = AppFrame::new("Money Manager");
        match &self.stage {
            Stage::Starting => frame,
            Stage::Upgrade => frame.child(
                Empty::new(Lucide::RefreshCw, "Convert your ledger to Tresse")
                    .body(
                        "Your data will be converted to the new format. Sync settings will be reset. \
                         Set up Tresse sync in Settings afterwards. The original data will be kept as a backup.",
                    )
                    .action(Command::new("convert-ledger", "Convert and reset sync").primary().on_trigger({
                        let shell = cx.entity().downgrade();
                        move |window, cx| {
                            shell.update(cx, |shell, cx| shell.convert(window, cx)).ok();
                        }
                    })),
            ),
            Stage::Converting => frame.child(
                Empty::new(Lucide::RefreshCw, "Converting your ledger")
                    .body("Saving your data in Tresse and resetting sync settings…"),
            ),
            Stage::Welcome(onboarding) => frame.child(onboarding.clone()),
            // The sections keep clear of the system's bars and the keyboard
            // themselves.
            Stage::Workspace(workspace) => frame.edge_to_edge(true).child(workspace.clone()),
            Stage::Failed(error) => frame.child(
                Empty::new(Lucide::TriangleAlert, "Couldn’t open the ledger").body(format!(
                    "The ledger and original backup are kept in {}. ({error})",
                    crate::paths::ledger_dir().display()
                )),
            ),
        }
    }
}
