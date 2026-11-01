use gpui_adaptive::prim::{column, row};
use gpui_adaptive::{
    ActiveScale as _, Banner, Command, Destination, Destinations, Env, Menu, Screen, Segments,
};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, SharedString, Window, div};
use money_core::format::format_money;
use money_core::store::hex_encode;

use super::{Section, Workspace};
use crate::book::Mode;
use crate::shell::{
    CONTEXT, EditRecord, ExportCsv, ImportCsv, NewRecord, NewTransfer, OpenSettings, SelectNext,
    SelectPrevious, ShowCategories, ShowCharts, ShowTransactions, SyncNow, ToggleMode,
};
use crate::ui::Lucide;

impl Workspace {
    /// The account every section is about, and the way to another one: the
    /// page's switcher, in its bar.
    fn account_menu(&self, cx: &mut Context<Self>) -> Menu {
        let book = self.book.read(cx);
        let label = match book.current_account() {
            Some(account) => {
                let balance = format_money(book.summary(&account.id).balance, &account.currency);
                format!("{} · {balance}", account.name)
            }
            None => "No account".to_string(),
        };
        let current = book.current_account().cloned();
        let accounts: Vec<Command> = book
            .visible_accounts()
            .map(|account| {
                let balance = format_money(book.summary(&account.id).balance, &account.currency);
                let chosen = current.as_ref().is_some_and(|c| c.id == account.id);
                let (handle, id) = (self.book.clone(), account.id.clone());
                Command::new(
                    SharedString::from(format!("account-{}", hex_encode(&account.id))),
                    format!("{} · {balance}", account.name),
                )
                .checked(chosen)
                .on_trigger(move |_, cx| handle.update(cx, |book, cx| book.select_account(&id, cx)))
            })
            .collect();
        let (add, edit, manage) = (self.book.clone(), self.book.clone(), self.book.clone());
        Menu::new(Command::new("account", label).icon(Lucide::Wallet))
            .choice()
            .title("Accounts")
            .commands(accounts)
            .separator()
            .command(
                Command::new("new-transfer", "New transfer…")
                    .icon(Lucide::ArrowLeftRight)
                    .action(NewTransfer),
            )
            .command(
                Command::new("new-account", "New account…")
                    .icon(Lucide::Plus)
                    .on_trigger(move |window, cx| {
                        crate::accounts::open_account_dialog(&add, None, window, cx)
                    }),
            )
            .command(
                Command::new("edit-account", "Edit account…")
                    .icon(Lucide::Pencil)
                    .disabled(current.is_none())
                    .on_trigger(move |window, cx| {
                        crate::accounts::open_account_dialog(&edit, current.clone(), window, cx)
                    }),
            )
            .command(
                Command::new("manage-accounts", "Accounts…")
                    .icon(Lucide::Wallet)
                    .on_trigger(move |window, cx| {
                        crate::accounts::open_accounts(&manage, window, cx)
                    }),
            )
    }

    /// Expenses or income: one switch for all three sections.
    fn mode_switch(&self, cx: &mut Context<Self>) -> Segments {
        let workspace = cx.entity().downgrade();
        Mode::ALL
            .iter()
            .fold(
                Segments::new("mode", self.mode.noun()),
                |segments, mode| segments.segment(mode.noun(), mode.title()),
            )
            .on_change(move |key, _, cx| {
                if let Some(mode) = Mode::ALL.into_iter().find(|mode| mode.noun() == key.as_ref()) {
                    workspace.update(cx, |this, cx| this.set_mode(mode, cx)).ok();
                }
            })
    }

    fn new_label(&self) -> String {
        match self.screen {
            Section::Categories => "New category…".to_string(),
            Section::Transactions | Section::Charts => format!("New {}…", self.mode.noun()),
        }
    }

    /// "New" on the section that is up: the page's one primary command.
    fn new_command(&self) -> Command {
        Command::new("new", self.new_label())
            .icon(Lucide::Plus)
            .primary()
            .action(NewRecord)
    }

    /// What the page offers besides "new". Each is the same action as its
    /// key, so a button, a menu item and a shortcut cannot disagree.
    fn commands(&self, cx: &mut Context<Self>) -> Vec<Command> {
        let busy = self.book.read(cx).is_busy();
        let mut commands = vec![
            Command::new("sync", "Sync")
                .icon(Lucide::RefreshCw)
                .disabled(busy)
                .action(SyncNow),
            Command::new("settings", "Settings…")
                .icon(Lucide::Settings)
                .action(OpenSettings),
        ];
        if self.screen == Section::Transactions {
            commands.push(
                Command::new("edit", "Open record")
                    .icon(Lucide::Pencil)
                    .overflow()
                    .disabled(!self.can_edit(cx))
                    .action(EditRecord),
            );
        }
        commands.extend([
            Command::new("import-csv", "Import CSV…")
                .overflow()
                .disabled(busy)
                .action(ImportCsv),
            Command::new("export-csv", "Export CSV…")
                .overflow()
                .disabled(busy)
                .action(ExportCsv),
        ]);
        commands
    }

    /// The page of the section that is up, with the mode over its content.
    fn render_page(&self, env: Env, cx: &mut Context<Self>) -> AnyElement {
        let scale = cx.scale();
        let (detail, view, scrolls): (String, AnyElement, bool) = match self.screen {
            Section::Transactions => (
                self.transactions.read(cx).detail(cx),
                self.transactions.clone().into_any_element(),
                false,
            ),
            Section::Categories => (
                self.categories.read(cx).detail(cx),
                self.categories.clone().into_any_element(),
                true,
            ),
            Section::Charts => (
                self.charts.read(cx).detail(cx),
                self.charts.clone().into_any_element(),
                true,
            ),
        };
        let mode = row()
            .flex_none()
            .px(scale.space_m)
            .py(scale.space_s)
            .child(self.mode_switch(cx));
        let content = column()
            .w_full()
            .when(!scrolls, |content| content.size_full())
            .child(mode)
            .child(
                div()
                    .w_full()
                    .when(!scrolls, |area| area.flex_1().min_h_0())
                    .child(view),
            );
        let syncing = self.book.read(cx).is_syncing();
        let page = Screen::new(self.screen.name(), self.screen.title())
            .subtitle(detail)
            .root(true)
            .switcher(self.account_menu(cx))
            .banner(syncing.then(|| {
                Banner::new("syncing", "Syncing with the sync storage…").icon(Lucide::RefreshCw)
            }))
            .commands(self.commands(cx))
            .scrolls(scrolls)
            .content(content);
        // With room for a rail or a sidebar, "new" heads it; on a compact
        // window it is the page's own, its FAB or its button.
        if env.is_compact() {
            page.command(self.new_command()).into_any_element()
        } else {
            page.into_any_element()
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let env = Env::of(window, cx);
        self.width = env.width();
        let workspace = cx.entity().downgrade();
        let sections = Section::ALL
            .into_iter()
            .fold(
                Destinations::new("navigation", self.screen.name()),
                |sections, section| {
                    sections.destination(Destination::new(
                        section.name(),
                        section.title(),
                        section.icon(),
                    ))
                },
            )
            .primary(self.new_command())
            .on_select(move |key, _, cx| {
                if let Some(section) = Section::ALL.into_iter().find(|s| s.name() == key.as_ref()) {
                    workspace.update(cx, |this, cx| this.show(section, cx)).ok();
                }
            })
            .content(self.render_page(env, cx));

        div()
            .id("workspace")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .on_action(cx.listener(|this, _: &ShowTransactions, _, cx| {
                this.show(Section::Transactions, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowCategories, _, cx| {
                this.show(Section::Categories, cx)
            }))
            .on_action(cx.listener(|this, _: &ShowCharts, _, cx| this.show(Section::Charts, cx)))
            .on_action(cx.listener(|this, _: &ToggleMode, _, cx| this.toggle_mode(cx)))
            .on_action(cx.listener(|this, _: &NewRecord, window, cx| this.new_record(window, cx)))
            .on_action(cx.listener(|this, _: &EditRecord, window, cx| this.edit_record(window, cx)))
            .on_action(cx.listener(|this, _: &SelectPrevious, _, cx| this.step_selection(false, cx)))
            .on_action(cx.listener(|this, _: &SelectNext, _, cx| this.step_selection(true, cx)))
            .on_action(
                cx.listener(|this, _: &NewTransfer, window, cx| this.new_transfer(window, cx)),
            )
            .on_action(cx.listener(|this, _: &SyncNow, window, cx| this.sync_now(window, cx)))
            .on_action(
                cx.listener(|this, _: &ImportCsv, window, cx| this.csv_file(false, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &ExportCsv, window, cx| this.csv_file(true, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &OpenSettings, window, cx| this.open_settings(window, cx)),
            )
            .child(sections)
    }
}
