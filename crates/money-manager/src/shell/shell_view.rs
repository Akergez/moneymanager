use gpui_kit::component::{ActiveTheme, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{Context, Entity, Pixels, SharedString, Subscription, Window, div, img};

use super::chrome::{bottom_bar_room, system_bars, title_frame};
use crate::book::Book;
use crate::onboarding::{self, Onboarding, OnboardingEvent};
use crate::settings::Settings;
use crate::ui::{self, Lucide};
use crate::workspace::Workspace;

/// What the window is showing.
pub enum Stage {
    /// Nothing has been decided yet: the window has only just opened.
    Starting,
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
    /// The room last left for the system's bar at the bottom; see
    /// [`bottom_bar_room`].
    bar_room: f32,
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
            bar_room: 0.0,
        }
    }

    pub fn stage(&self) -> &Stage {
        &self.stage
    }

    /// Decides what this window is for. Runs once, right after it opens.
    pub(super) fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (source, last_account, currency) = {
            let settings = Settings::global(cx);
            (
                settings.source(),
                settings.last_account(),
                settings.default_currency(),
            )
        };

        if let Some(source) = source {
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
            match onboarding::create_ledger(&currency, cx) {
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

    fn show_workspace(&mut self, book: Entity<Book>, window: &mut Window, cx: &mut Context<Self>) {
        let workspace = cx.new(|cx| Workspace::new(book, window, cx));
        workspace.update(cx, |workspace, cx| workspace.focus(window, cx));
        self._onboarding = None;
        self.stage = Stage::Workspace(workspace);
        cx.notify();
    }

    /// The bar a stage with nothing of its own to put in it gets: the name of
    /// the application, and the window's buttons.
    fn plain_bar(cx: &mut Context<Self>) -> impl IntoElement {
        title_frame(
            h_flex()
                .gap_2()
                .child(img(ui::app_icon()).flex_none().size_5())
                .child(div().text_sm().font_semibold().child("Money Manager")),
            cx,
        )
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (bars_top, inset_bottom) = system_bars();
        self.bar_room = bottom_bar_room(
            inset_bottom,
            f32::from(window.viewport_size().height),
            self.bar_room,
        );
        let bars_bottom = self.bar_room;
        let body = match &self.stage {
            Stage::Starting => v_flex()
                .size_full()
                .child(Self::plain_bar(cx))
                .into_any_element(),
            Stage::Welcome(onboarding) => v_flex()
                .size_full()
                .child(Self::plain_bar(cx))
                .child(div().flex_1().min_h_0().child(onboarding.clone()))
                .into_any_element(),
            Stage::Workspace(workspace) => workspace.clone().into_any_element(),
            Stage::Failed(error) => v_flex()
                .size_full()
                .child(Self::plain_bar(cx))
                .child(div().flex_1().min_h_0().child(ui::empty_state(
                    Lucide::TriangleAlert,
                    "Couldn’t open the ledger",
                    format!(
                        "Nothing was changed. The ledger is in {}. ({error})",
                        crate::paths::ledger_dir().display()
                    ),
                    None,
                    cx,
                )))
                .into_any_element(),
        };

        let theme = cx.theme();
        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            // Under the status bar, in the title bar's colour so that the two
            // read as one. The insets are the platform's own measurements.
            .child(
                div()
                    .flex_none()
                    .h(Pixels::from(bars_top))
                    .w_full()
                    .bg(theme.title_bar),
            )
            .child(div().flex_1().min_h_0().w_full().child(body))
            .pb(Pixels::from(bars_bottom))
    }
}
