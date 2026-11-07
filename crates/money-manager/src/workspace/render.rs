use gpui_adaptive::prim::{column, row};
use gpui_adaptive::{
    ActiveScale as _, Banner, Command, Destination, Destinations, Env, Menu, Navigator, Page,
    Screen, Segments,
};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, SharedString, Window, div};
use money_core::format::format_money;
use money_core::store::hex_encode;

use super::{Place, Section, Workspace};
use crate::book::{Filter, Mode};
use crate::shell::{
    CONTEXT, EditRecord, ExportCsv, GoBack, ImportCsv, NewRecord, NewTransfer, OpenSettings,
    SelectNext, SelectPrevious, ShowCategories, ShowCharts, ShowTags, ShowTransactions, SyncNow,
    ToggleMode,
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
            .fold(Segments::new("mode", self.mode.noun()), |segments, mode| {
                segments.segment(mode.noun(), mode.title())
            })
            .on_change(move |key, _, cx| {
                if let Some(mode) = Mode::ALL
                    .into_iter()
                    .find(|mode| mode.noun() == key.as_ref())
                {
                    workspace
                        .update(cx, |this, cx| this.set_mode(mode, cx))
                        .ok();
                }
            })
    }

    fn new_label(&self, cx: &gpui_kit::App) -> String {
        match (self.place(cx), self.screen) {
            (Place::Records(mode, _), _) => format!("New {}…", mode.noun()),
            (_, Section::Categories) => "New category…".to_string(),
            (_, Section::Tags) => "New tag…".to_string(),
            (_, Section::Transactions | Section::Charts) => format!("New {}…", self.mode.noun()),
        }
    }

    /// "New" on the page that is up: the page's one primary command.
    fn new_command(&self, cx: &gpui_kit::App) -> Command {
        Command::new("new", self.new_label(cx))
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

    /// The page of a place: a screen, or the records of a category or a
    /// tag.
    fn render_place(
        &self,
        place: &Place,
        page: Page,
        env: Env,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match place {
            Place::Section(section) => self.render_page(*section, env, cx),
            Place::Records(mode, filter) => self.render_records(*mode, filter, page, env, cx),
        }
    }

    /// The records of one category or one tag: its name over them, the way
    /// back, and the way to change it.
    fn render_records(
        &self,
        mode: Mode,
        filter: &Filter,
        page: Page,
        env: Env,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(view) = self
            .records
            .clone()
            .filter(|view| view.read(cx).filter() == Some(filter))
        else {
            return gpui_kit::Empty.into_any_element();
        };
        let book = self.book.read(cx);
        let (item, noun) = match filter {
            Filter::Category(id) => (book.category(mode, id), "category"),
            Filter::Tag(id) => (book.tag(mode, id), "tag"),
        };
        let title = item
            .as_ref()
            .map(|item| item.name.clone())
            .unwrap_or_else(|| "Deleted".into());
        let detail = view.read(cx).detail(cx);
        let subtitle = match detail.is_empty() {
            true => format!("{} {noun}", capitalised(mode.noun())),
            false => format!("{} {noun} · {detail}", capitalised(mode.noun())),
        };
        let handle = self.book.clone();
        let is_tag = matches!(filter, Filter::Tag(_));
        let edit = Command::new("edit-filter", format!("Edit {noun}…"))
            .icon(Lucide::Pencil)
            .disabled(item.is_none())
            .on_trigger(move |window, cx| match (&item, is_tag) {
                (Some(item), true) => crate::categories::open_tag_dialog(
                    &handle,
                    mode,
                    Some(item.clone()),
                    window,
                    cx,
                ),
                (Some(item), false) => crate::categories::open_category_dialog(
                    &handle,
                    mode,
                    Some(item.clone()),
                    window,
                    cx,
                ),
                (None, _) => {}
            });
        let mut commands = vec![
            edit,
            Command::new("edit", "Open record")
                .icon(Lucide::Pencil)
                .overflow()
                .disabled(!view.read(cx).has_selection(cx))
                .action(EditRecord),
        ];
        if env.is_compact() {
            commands.push(self.new_command(cx));
        }
        Screen::new("records", title)
            .subtitle(subtitle)
            .up(page.up())
            .commands(commands)
            .scrolls(false)
            .content(view)
            .into_any_element()
    }

    /// The page of a section, with the mode over its content.
    fn render_page(&self, section: Section, env: Env, cx: &mut Context<Self>) -> AnyElement {
        let scale = cx.scale();
        let (detail, view, scrolls): (String, AnyElement, bool) = match section {
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
            Section::Tags => (
                self.tags.read(cx).detail(cx),
                self.tags.clone().into_any_element(),
                true,
            ),
        };
        let mode = row()
            .flex_none()
            .px(scale.space_m)
            .pb(scale.space_s)
            // On a finger the header above ends with its own room.
            .when(!env.is_touch(), |mode| mode.pt(scale.space_s))
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
        let page = Screen::new(section.name(), section.title())
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
            page.command(self.new_command(cx)).into_any_element()
        } else {
            page.into_any_element()
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let env = Env::of(window, cx);
        self.width = env.width();
        let at_root = self.navigator.read(cx).is_at_root();
        let this = cx.entity();
        let pages = Navigator::new("pages", &self.navigator, move |place, page, window, cx| {
            let env = Env::of(window, cx);
            this.update(cx, |this, cx| this.render_place(place, page, env, cx))
        });
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
            .primary(self.new_command(cx))
            .bar_visible(at_root)
            .on_select(move |key, _, cx| {
                if let Some(section) = Section::ALL.into_iter().find(|s| s.name() == key.as_ref()) {
                    workspace.update(cx, |this, cx| this.show(section, cx)).ok();
                }
            })
            .content(pages);

        div()
            .id("workspace")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .on_action(
                cx.listener(|this, _: &ShowTransactions, _, cx| {
                    this.show(Section::Transactions, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &ShowCategories, _, cx| this.show(Section::Categories, cx)),
            )
            .on_action(cx.listener(|this, _: &ShowCharts, _, cx| this.show(Section::Charts, cx)))
            .on_action(cx.listener(|this, _: &ShowTags, _, cx| this.show(Section::Tags, cx)))
            .on_action(cx.listener(|this, _: &GoBack, _, cx| this.go_back(cx)))
            .on_action(cx.listener(|this, _: &ToggleMode, _, cx| this.toggle_mode(cx)))
            .on_action(cx.listener(|this, _: &NewRecord, window, cx| this.new_record(window, cx)))
            .on_action(cx.listener(|this, _: &EditRecord, window, cx| this.edit_record(window, cx)))
            .on_action(
                cx.listener(|this, _: &SelectPrevious, _, cx| this.step_selection(false, cx)),
            )
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

/// "Expense" from "expense".
fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
