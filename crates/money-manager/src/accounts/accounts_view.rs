//! All accounts, including hidden ones, with synced visibility and startup choice.
//! The book owns mutations; this view observes it and remains usable after sync.

use gpui_kit::component::button::Button;
use gpui_kit::component::dialog::{DialogClose, DialogFooter};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable, StyledExt, WindowExt, h_flex, v_flex,
};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Entity, SharedString, Subscription, Window, div};
use money_core::format::format_money;

use crate::book::Book;

struct AccountsView {
    book: Entity<Book>,
    _watch: Subscription,
}

impl Render for AccountsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let book = self.book.read(cx);
        let busy = book.is_busy();
        let main = book.main_account().map(|account| account.id.clone());
        let rows: Vec<_> = book
            .accounts()
            .iter()
            .map(|account| {
                (
                    account.clone(),
                    book.is_account_hidden(&account.id),
                    book.summary(&account.id).balance,
                )
            })
            .collect();
        v_flex().gap_3()
            .child(div().text_sm().text_color(cx.theme().muted_foreground)
                .child("Hidden accounts keep their history and transfers. The main account opens at startup."))
            .child(div().id("all-accounts").max_h_96().overflow_y_scroll().child(
                v_flex().gap_4().children(rows.into_iter().map(|(account, hidden, balance)| {
                    let is_main = main.as_deref() == Some(account.id.as_slice());
                    let id = account.id.clone();
                    let main_id = account.id.clone();
                    let book = self.book.clone();
                    let main_book = self.book.clone();
                    v_flex().id(SharedString::from(format!("account-{}", money_core::store::hex_encode(&id)))).gap_2()
                        .child(h_flex().gap_2()
                            .child(div().flex_1().min_w_0().truncate().font_medium().child(account.name))
                            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(format_money(balance, &account.currency))))
                        .child(h_flex().gap_2()
                            .child(Button::new(SharedString::from(format!("visibility-{}", money_core::store::hex_encode(&id))))
                                .label(if hidden { "Show" } else { "Hide" }).small().outline().disabled(busy)
                                .on_click(move |_, window, cx| {
                                    if let Err(error) = book.update(cx, |book, cx| book.set_account_hidden(&id, !hidden, cx)) {
                                        window.push_notification(Notification::error(error), cx);
                                    }
                                }))
                            .child(Button::new(SharedString::from(format!("main-{}", money_core::store::hex_encode(&main_id))))
                                .label(if is_main { "Main account" } else { "Make main" }).small().outline().disabled(busy || is_main)
                                .on_click(move |_, window, cx| {
                                    if let Err(error) = main_book.update(cx, |book, cx| book.set_main_account(&main_id, cx)) {
                                        window.push_notification(Notification::error(error), cx);
                                    }
                                })))
                }))
            ))
    }
}

pub fn open_accounts(book: &Entity<Book>, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| AccountsView {
        book: book.clone(),
        _watch: cx.observe(book, |_, _, cx| cx.notify()),
    });
    if crate::shell::Layout::of(window).is_phone() {
        return crate::bottom_sheet::open_panel("Accounts".into(), view.into(), window, cx);
    }
    window.open_dialog(cx, move |dialog, _, _| {
        dialog.title("Accounts").child(view.clone()).footer(
            DialogFooter::new().child(DialogClose::new().child(Button::new("done").label("Done"))),
        )
    });
}
