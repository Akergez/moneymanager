//! All accounts, including hidden ones, with synced visibility and startup choice.
//! The book owns mutations; this view observes it and remains usable after sync.

use gpui_adaptive::prim::row;
use gpui_adaptive::{ActiveScale as _, Collection, Command, Fields, Form, ListRow, Note, Notice};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Entity, SharedString, Subscription, Window};
use money_core::format::format_money;
use money_core::store::hex_encode;

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
        let accounts: Vec<_> = book
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
        let gap = cx.scale().space_s;
        let rows = accounts.into_iter().map(|(account, hidden, balance)| {
            let is_main = main.as_deref() == Some(account.id.as_slice());
            let hex = hex_encode(&account.id);
            let (id, visible_book) = (account.id.clone(), self.book.clone());
            let (main_id, main_book) = (account.id.clone(), self.book.clone());
            let visibility = Command::new(
                SharedString::from(format!("visibility-{hex}")),
                if hidden { "Show" } else { "Hide" },
            )
            .disabled(busy)
            .on_trigger(move |window, cx| {
                if let Err(error) =
                    visible_book.update(cx, |book, cx| book.set_account_hidden(&id, !hidden, cx))
                {
                    Notice::error(error).push(window, cx);
                }
            });
            let make_main = Command::new(
                SharedString::from(format!("main-{hex}")),
                if is_main { "Main account" } else { "Make main" },
            )
            .disabled(busy || is_main)
            .on_trigger(move |window, cx| {
                if let Err(error) =
                    main_book.update(cx, |book, cx| book.set_main_account(&main_id, cx))
                {
                    Notice::error(error).push(window, cx);
                }
            });
            ListRow::new(SharedString::from(format!("account-{hex}")), account.name.clone())
                .subtitle(format_money(balance, &account.currency))
                .muted(hidden)
                .body(
                    row()
                        .gap(gap)
                        .child(visibility.as_button().dense())
                        .child(make_main.as_button().dense()),
                )
        });
        Fields::new()
            .child(Note::new(
                "Hidden accounts keep their history and transfers. The main account opens at startup.",
            ))
            .child(Collection::new("all-accounts").children(rows))
    }
}

/// Opens the list of every account: a panel with Done.
pub fn open_accounts(book: &Entity<Book>, window: &mut Window, cx: &mut App) {
    let view = cx.new(|cx| AccountsView {
        book: book.clone(),
        _watch: cx.observe(book, |_, _, cx| cx.notify()),
    });
    Form::new("Accounts", view).open(window, cx);
}
