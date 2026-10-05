//! The account dialog: a name, a currency and what it held to begin with.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{ActiveTheme, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Entity, SharedString, Window, div};
use money_core::format::{format_amount, parse_amount};
use money_core::models::Account;

use crate::book::Book;
use crate::ui;

struct AccountForm {
    book: Entity<Book>,
    /// The account being changed, or nothing for a new one.
    editing: Option<Account>,
    name: Entity<InputState>,
    currency: Entity<InputState>,
    opening_balance: Entity<InputState>,
    error: Option<SharedString>,
}

impl AccountForm {
    fn new(
        book: Entity<Book>,
        editing: Option<Account>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut input = |value: Option<String>, placeholder: &'static str| {
            cx.new(|cx| {
                let state = InputState::new(window, cx).placeholder(placeholder);
                match value {
                    Some(value) => state.default_value(value),
                    None => state,
                }
            })
        };
        let name = input(editing.as_ref().map(|a| a.name.clone()), "Savings");
        let currency = input(editing.as_ref().map(|a| a.currency.clone()), "USD");
        let opening_balance = input(
            editing
                .as_ref()
                .map(|a| format_amount(a.opening_balance).replace(' ', "")),
            "0.00",
        );
        AccountForm {
            book,
            editing,
            name,
            currency,
            opening_balance,
            error: None,
        }
    }

    /// Saves the account, and makes a new one the current one: it was made
    /// to be used. Answers whether it saved.
    fn submit(&mut self, cx: &mut Context<Self>) -> bool {
        let name = self.name.read(cx).value().to_string();
        let currency = self.currency.read(cx).value().to_string();
        let balance_text = self.opening_balance.read(cx).value();
        let opening_balance = if balance_text.trim().is_empty() {
            Some(0.0)
        } else {
            parse_amount(&balance_text)
        };
        let Some(opening_balance) = opening_balance else {
            self.error = Some("Opening balance is not a number.".into());
            cx.notify();
            return false;
        };
        let id = self.editing.as_ref().map(|account| account.id.clone());
        let is_new = id.is_none();
        let saved = self.book.update(cx, |book, cx| {
            let saved = book.save_account(id.as_deref(), &name, &currency, opening_balance, cx)?;
            if is_new {
                book.select_account(&saved, cx);
            }
            Ok::<_, String>(())
        });
        match saved {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
                false
            }
        }
    }
}

impl Render for AccountForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(ui::field("Name", Input::new(&self.name)))
            .child(
                ui::field("Currency", Input::new(&self.currency)).child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("A three-letter code, like USD or EUR."),
                ),
            )
            .child(
                ui::field("Opening balance", Input::new(&self.opening_balance)).child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child("What the account held before its first record."),
                ),
            )
            .children(self.error.clone().map(|error| ui::form_error(error, cx)))
    }
}

/// Opens the dialog for a new account, or for `account`.
pub fn open_account_dialog(
    book: &Entity<Book>,
    account: Option<Account>,
    window: &mut Window,
    cx: &mut App,
) {
    let (title, commit) = match &account {
        Some(_) => ("Edit account", "Save"),
        None => ("New account", "Add"),
    };
    let form = cx.new(|cx| AccountForm::new(book.clone(), account, window, cx));
    let name = gpui_kit::Focusable::focus_handle(&form.read(cx).name, cx);
    ui::open_form(
        title,
        commit,
        form,
        Some(name),
        AccountForm::submit,
        window,
        cx,
    );
}
