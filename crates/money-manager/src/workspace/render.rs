use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::sidebar::{Sidebar, SidebarMenu, SidebarMenuItem};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    Action, AnyElement, Context, Hsla, MouseButton, SharedString, Window, div, img, rems,
};
use money_core::format::format_money;

use super::{Screen, Workspace};
use crate::book::Mode;
use crate::shell::{
    CONTEXT, EditRecord, Layout, NewRecord, NewTransfer, OpenSettings, ShowCategories, ShowCharts,
    ShowTransactions, SyncNow, ToggleMode, title_frame,
};
use crate::ui::{self, Lucide};

/// How wide the sidebar is with its labels, in rems: the widest thing in it
/// is "New category…" beside its shortcut.
const SIDEBAR_WIDTH: f32 = 15.0;

/// A command of the workspace as a button. With room for words it carries
/// its icon and name at the leading edge and its shortcut at the trailing
/// one, as a key cap read off the binding itself — so what the button says
/// can never be a key that no longer does it. Without room it is the icon,
/// and the name and the key cap are its tooltip.
///
/// `cap` is the key cap's fill and text colour where the theme's own would
/// not show against the button.
fn command_button(
    button: Button,
    icon: Lucide,
    label: SharedString,
    action: &dyn Action,
    layout: Layout,
    cap: Option<(Hsla, Hsla)>,
    window: &Window,
) -> Button {
    if layout != Layout::Desktop {
        return button
            .icon(icon)
            .tooltip_with_action(label, action, Some(CONTEXT));
    }
    let shortcut = Kbd::binding_for_action(action, Some(CONTEXT), window).map(|key| match cap {
        Some((fill, text)) => key.bg(fill).text_color(text),
        None => key.outline(),
    });
    button.w_full().child(
        h_flex()
            .w_full()
            .gap_2()
            .child(Icon::new(icon))
            .child(div().flex_1().min_w_0().truncate().child(label))
            .children(shortcut),
    )
}

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

    /// The screen's one command: a new record, or a new category on the
    /// categories screen. It stands where the hand already is — at the head
    /// of the navigation on a desktop, beside it on a phone — and says its
    /// shortcut wherever there is a keyboard to press it on.
    fn render_new_button(&self, layout: Layout, window: &Window, cx: &mut Context<Self>) -> Button {
        command_button(
            Button::new("new").primary(),
            Lucide::Plus,
            self.new_label().into(),
            &NewRecord,
            layout,
            // On the primary fill, a key cap in the theme's muted colours
            // would be a grey patch; this one is cut out of the fill.
            Some((
                cx.theme().primary_foreground.opacity(0.18),
                cx.theme().primary_foreground,
            )),
            window,
        )
        .on_click(cx.listener(|this, _, window, cx| this.new_record(window, cx)))
    }

    /// The window's title bar, carrying the account and, where there is room,
    /// the mode.
    fn render_top_bar(&self, layout: Layout, cx: &mut Context<Self>) -> AnyElement {
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
            .when(layout.is_phone(), |bar| {
                bar.child(self.render_more_menu(cx))
            });
        title_frame(controls.pr_2(), cx)
    }

    /// The sidebar of the two wider layouts; collapsed to icons on a tablet.
    fn render_sidebar(
        &self,
        layout: Layout,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let collapsed = layout == Layout::Tablet;
        let new = self.render_new_button(layout, window, cx);
        let syncing = self.book.read(cx).is_syncing();
        let command = |button: Button, icon: Lucide, label: &'static str, action: &dyn Action| {
            command_button(button, icon, label.into(), action, layout, None, window)
        };
        // Under "new", because it is the other thing done to a record.
        let edit = command(
            Button::new("edit").outline(),
            Lucide::Pencil,
            "Edit…",
            &EditRecord,
        )
        .disabled(!self.can_edit(cx))
        .on_click(cx.listener(|this, _, window, cx| this.edit_record(window, cx)));
        // Outlined, so it reads as a button of its own and not as one row
        // of a list with Settings under it.
        let sync = command(
            Button::new("sync").outline(),
            Lucide::RefreshCw,
            "Sync",
            &SyncNow,
        )
        .loading(syncing)
        .on_click(cx.listener(|this, _, window, cx| this.sync_now(window, cx)));
        let settings = command(
            Button::new("settings").ghost(),
            Lucide::Settings,
            "Settings…",
            &OpenSettings,
        )
        .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx)));
        Sidebar::new("navigation")
            .collapsible(true)
            .collapsed(collapsed)
            // In rems, so the labels still fit when the text is larger; the
            // collapsed rail keeps the library's own width.
            .when(!collapsed, |sidebar| sidebar.w(rems(SIDEBAR_WIDTH)))
            .header(v_flex().w_full().gap_2().child(new).child(edit))
            .child(SidebarMenu::new().children(Screen::ALL.map(|screen| {
                SidebarMenuItem::new(screen.title())
                    .icon(screen.icon())
                    .active(self.screen == screen)
                    .on_click(cx.listener(move |this, _, _, cx| this.show(screen, cx)))
            })))
            .footer(
                // A plain column, not the library's footer: that one is a
                // single pressable block, lit as a whole under the pointer,
                // which made two commands read as one.
                v_flex().w_full().gap_2().child(sync).child(settings),
            )
    }

    /// The navigation of the phone layout: the three screens along the
    /// bottom, each an icon over its name, sharing the width evenly. The one
    /// that is up has its icon on a pill of the accent colour and its name
    /// in full strength — the same two signals a phone's own navigation bar
    /// gives.
    fn render_bottom_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        // A tint of the primary colour: the theme's accent is a grey too
        // close to the bar's own to mark anything.
        let (accent, on_accent) = (theme.primary.opacity(0.14), theme.foreground);
        let (strong, quiet) = (theme.foreground, theme.muted_foreground);
        h_flex()
            .flex_none()
            .w_full()
            .items_stretch()
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .children(Screen::ALL.map(|screen| {
                let active = self.screen == screen;
                v_flex()
                    .id(screen.name())
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_0p5()
                    .pt_2()
                    .pb_1p5()
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| this.show(screen, cx)))
                    .child(
                        h_flex()
                            .w_12()
                            .h_7()
                            .justify_center()
                            .rounded_full()
                            .when(active, |pill| pill.bg(accent).text_color(on_accent))
                            .when(!active, |pill| pill.text_color(quiet))
                            .child(Icon::new(screen.icon())),
                    )
                    .child(
                        div()
                            .text_xs()
                            .when(active, |name| name.font_semibold().text_color(strong))
                            .when(!active, |name| name.text_color(quiet))
                            .child(screen.title()),
                    )
            }))
    }

    /// The phone layout's "new": a round button floating over the content's
    /// trailing corner, where a thumb rests, clear of the navigation under
    /// it.
    fn render_floating_new(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("floating-new")
            .absolute()
            .right_4()
            .bottom_4()
            // It lies over the list, and a press on it is a press on it
            // alone: without this the row underneath would open too.
            .occlude()
            .child(
                Button::new("new")
                    // Its own icon, at a size that fills the circle: the one a
                    // button draws for itself is sized for a line of text.
                    .child(Icon::new(Lucide::Plus).size_6())
                    .primary()
                    .size_12()
                    .rounded_full()
                    .shadow_lg()
                    .tooltip(self.new_label())
                    .on_click(cx.listener(|this, _, window, cx| this.new_record(window, cx))),
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
        let layout = Layout::of(window);
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
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(screen)
                    .when(layout.is_phone(), |area| {
                        area.child(self.render_floating_new(cx))
                    }),
            )
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
            .on_action(cx.listener(|this, _: &EditRecord, window, cx| this.edit_record(window, cx)))
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
                        row.child(self.render_sidebar(layout, window, cx))
                    })
                    .child(content),
            )
    }
}
