use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::sidebar::{Sidebar, SidebarFooter, SidebarMenu, SidebarMenuItem};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme, Sizable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, MouseButton, Window, div, img};
use money_core::format::format_money;

use super::{Screen, Workspace};
use crate::book::Mode;
use crate::shell::{
    CONTEXT, Layout, NewRecord, NewTransfer, OpenSettings, ShowCategories, ShowCharts,
    ShowTransactions, SyncNow, ToggleMode, title_frame,
};
use crate::ui::{self, Lucide};

impl Workspace {
    /// The account every screen is about, and the way to another one. It
    /// sits at the top of the window in every layout.
    fn render_account_menu(&self, layout: Layout, cx: &mut Context<Self>) -> impl IntoElement {
        let book = self.book.read(cx);
        let label = match book.current_account() {
            Some(account) => {
                let balance = format_money(book.summary(&account.id).balance, &account.currency);
                format!("{} · {balance}", account.name)
            }
            None => "No account".to_string(),
        };
        let book = self.book.clone();
        let button = Button::new("account")
            .icon(Lucide::Wallet)
            .label(label)
            .dropdown_caret(true);
        let button = if layout.is_phone() {
            button
        } else {
            button.small()
        };
        button.dropdown_menu(move |mut menu, _, cx| {
            let (accounts, current) = {
                let book = book.read(cx);
                let accounts: Vec<_> = book
                    .accounts()
                    .iter()
                    .map(|account| {
                        let balance = book.summary(&account.id).balance;
                        (account.clone(), format_money(balance, &account.currency))
                    })
                    .collect();
                (accounts, book.current_account().cloned())
            };
            for (account, balance) in accounts {
                let selected = current.as_ref().is_some_and(|c| c.id == account.id);
                let book = book.clone();
                menu = menu.item(
                    PopupMenuItem::new(format!("{} · {balance}", account.name))
                        .checked(selected)
                        .on_click(move |_, _, cx| {
                            book.update(cx, |book, cx| book.select_account(&account.id, cx));
                        }),
                );
            }
            let transfer = book.clone();
            let add = book.clone();
            let edit = book.clone();
            menu.separator()
                .item(
                    PopupMenuItem::new("New transfer…")
                        .icon(Lucide::ArrowLeftRight)
                        .on_click(move |_, window, cx| {
                            crate::accounts::open_transfer_dialog(&transfer, window, cx)
                        }),
                )
                .item(
                    PopupMenuItem::new("New account…")
                        .icon(Lucide::Plus)
                        .on_click(move |_, window, cx| {
                            crate::accounts::open_account_dialog(&add, None, window, cx)
                        }),
                )
                .item(
                    PopupMenuItem::new("Edit account…")
                        .icon(Lucide::Pencil)
                        .disabled(current.is_none())
                        .on_click(move |_, window, cx| {
                            crate::accounts::open_account_dialog(&edit, current.clone(), window, cx)
                        }),
                )
        })
    }

    /// Expenses or income: one switch for all three screens.
    fn render_mode_switch(&self, layout: Layout, cx: &mut Context<Self>) -> TabBar {
        let tabs = TabBar::new("mode")
            .segmented()
            .selected_index(self.mode.index())
            .children(Mode::ALL.map(|mode| Tab::new().label(mode.title())))
            .on_click(cx.listener(|this, index: &usize, _, cx| {
                this.set_mode(Mode::from_index(*index), cx);
            }));
        if layout.is_phone() {
            tabs.w_full()
        } else {
            tabs.small()
        }
    }

    fn new_label(&self) -> String {
        match self.screen {
            Screen::Categories => "New category…".to_string(),
            Screen::Transactions | Screen::Charts => format!("New {}…", self.mode.noun()),
        }
    }

    /// The window's title bar, carrying the account and, where there is room,
    /// the mode and the screen's one command.
    fn render_top_bar(&self, layout: Layout, cx: &mut Context<Self>) -> AnyElement {
        let new = Button::new("new").icon(Lucide::Plus);
        let new = if layout.is_phone() {
            // The label is what the icon stands for, for assistive technology
            // and as the tooltip.
            new.tooltip(self.new_label())
        } else {
            new.small().label(self.new_label())
        }
        .on_click(cx.listener(|this, _, window, cx| this.new_record(window, cx)));

        // The bar under these is what drags the window, so everything that
        // takes a press keeps it to itself.
        let controls = h_flex()
            .id("top-bar-controls")
            .flex_1()
            .min_w_0()
            .gap_3()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .when(!layout.is_phone(), |bar| {
                bar.child(img(ui::app_icon()).flex_none().size_5())
            })
            .child(self.render_account_menu(layout, cx))
            .when(!layout.is_phone(), |bar| {
                bar.child(self.render_mode_switch(layout, cx))
            })
            .child(div().flex_1())
            .child(new)
            .when(layout.is_phone(), |bar| {
                bar.child(self.render_more_menu(cx))
            });
        title_frame(controls.pr_2(), cx)
    }

    /// The sidebar of the two wider layouts; collapsed to icons on a tablet.
    fn render_sidebar(&self, layout: Layout, cx: &mut Context<Self>) -> impl IntoElement {
        let collapsed = layout == Layout::Tablet;
        let syncing = self.book.read(cx).is_syncing();
        let quiet = |id: &'static str, icon: Lucide, label: &'static str| {
            let button = Button::new(id).icon(icon).ghost().small();
            if collapsed {
                button.tooltip(label)
            } else {
                button.label(label)
            }
        };
        Sidebar::new("navigation")
            .collapsible(true)
            .collapsed(collapsed)
            .child(SidebarMenu::new().children(Screen::ALL.map(|screen| {
                SidebarMenuItem::new(screen.title())
                    .icon(screen.icon())
                    .active(self.screen == screen)
                    .on_click(cx.listener(move |this, _, _, cx| this.show(screen, cx)))
            })))
            .footer(
                SidebarFooter::new().child(
                    // At their own width, from the leading edge: the same
                    // spine the destinations above them stand on.
                    v_flex()
                        .w_full()
                        .items_start()
                        .gap_1()
                        .child(
                            quiet("sync", Lucide::RefreshCw, "Sync")
                                .loading(syncing)
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.sync_now(window, cx)),
                                ),
                        )
                        .child(quiet("settings", Lucide::Settings, "Settings…").on_click(
                            cx.listener(|this, _, window, cx| this.open_settings(window, cx)),
                        )),
                ),
            )
    }

    /// The navigation of the phone layout: the three screens along the
    /// bottom, by name — a row of bare icons would leave two of the three to
    /// be guessed at.
    fn render_bottom_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .flex_none()
            .w_full()
            .justify_center()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().sidebar)
            .child(
                TabBar::new("navigation")
                    .underline()
                    .large()
                    .selected_index(self.screen.index())
                    .children(Screen::ALL.map(|screen| Tab::new().label(screen.title())))
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        if let Some(screen) = Screen::ALL.get(*index) {
                            this.show(*screen, cx);
                        }
                    })),
            )
    }

    /// What the sidebar's footer holds, for the layout that has no sidebar:
    /// one menu at the top, beside the account.
    fn render_more_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = cx.entity();
        let syncing = self.book.read(cx).is_syncing();
        Button::new("more")
            .icon(Lucide::Ellipsis)
            .ghost()
            .loading(syncing)
            .accessibility_label("More")
            .dropdown_menu(move |menu, _, _| {
                let (sync, settings) = (workspace.clone(), workspace.clone());
                menu.item(
                    PopupMenuItem::new("Sync")
                        .icon(Lucide::RefreshCw)
                        .disabled(syncing)
                        .on_click(move |_, window, cx| {
                            sync.update(cx, |this, cx| this.sync_now(window, cx))
                        }),
                )
                .item(
                    PopupMenuItem::new("Settings…")
                        .icon(Lucide::Settings)
                        .on_click(move |_, window, cx| {
                            settings.update(cx, |this, cx| this.open_settings(window, cx))
                        }),
                )
            })
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(f32::from(window.viewport_size().width));
        self.layout = layout;

        let screen = match self.screen {
            Screen::Transactions => self.transactions.clone().into_any_element(),
            Screen::Categories => self.categories.clone().into_any_element(),
            Screen::Charts => self.charts.clone().into_any_element(),
        };
        let content = v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .when(layout.is_phone(), |column| {
                column.child(
                    div()
                        .flex_none()
                        .px_3()
                        .py_2()
                        .child(self.render_mode_switch(layout, cx)),
                )
            })
            .child(div().flex_1().min_h_0().child(screen))
            .when(layout.is_phone(), |column| {
                column.child(self.render_bottom_bar(cx))
            });

        v_flex()
            .id("workspace")
            .key_context(CONTEXT)
            .track_focus(&self.focus)
            .size_full()
            .on_action(
                cx.listener(|this, _: &ShowTransactions, _, cx| {
                    this.show(Screen::Transactions, cx)
                }),
            )
            .on_action(
                cx.listener(|this, _: &ShowCategories, _, cx| this.show(Screen::Categories, cx)),
            )
            .on_action(cx.listener(|this, _: &ShowCharts, _, cx| this.show(Screen::Charts, cx)))
            .on_action(cx.listener(|this, _: &ToggleMode, _, cx| this.toggle_mode(cx)))
            .on_action(cx.listener(|this, _: &NewRecord, window, cx| this.new_record(window, cx)))
            .on_action(
                cx.listener(|this, _: &NewTransfer, window, cx| this.new_transfer(window, cx)),
            )
            .on_action(cx.listener(|this, _: &SyncNow, window, cx| this.sync_now(window, cx)))
            .on_action(
                cx.listener(|this, _: &OpenSettings, window, cx| this.open_settings(window, cx)),
            )
            .child(self.render_top_bar(layout, cx))
            .child(
                // A row of full-height columns: without `items_stretch` the
                // sidebar would take its content's height and be centred.
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .items_stretch()
                    .when(!layout.is_phone(), |row| {
                        row.child(self.render_sidebar(layout, cx))
                    })
                    .child(content),
            )
    }
}
