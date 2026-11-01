//! The account form: a name, a currency and what it held to begin with.

use gpui_adaptive::{Fields, Form, Note, Purpose, TextField, TextFieldState, Tone};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Entity, Focusable, SharedString, Window};
use money_core::format::{format_amount, parse_amount};
use money_core::models::Account;

use crate::book::Book;

struct AccountForm {
    book: Entity<Book>,
    /// The account being changed, or nothing for a new one.
    editing: Option<Account>,
    name: Entity<TextFieldState>,
    currency: Entity<TextFieldState>,
    opening_balance: Entity<TextFieldState>,
    error: Option<SharedString>,
}

impl AccountForm {
    fn new(
        book: Entity<Book>,
        editing: Option<Account>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut input = |purpose: Purpose, value: Option<String>, placeholder: &'static str| {
            cx.new(|cx| {
                let mut state = TextFieldState::new(purpose, window, cx);
                state.set_placeholder(placeholder, window, cx);
                if let Some(value) = value {
                    state.set_value(value, window, cx);
                }
                state
            })
        };
        let name = input(Purpose::Message, editing.as_ref().map(|a| a.name.clone()), "Savings");
        let currency = input(Purpose::Plain, editing.as_ref().map(|a| a.currency.clone()), "USD");
        let opening_balance = input(
            Purpose::Number,
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
        let name = self.name.read(cx).value(cx).to_string();
        let currency = self.currency.read(cx).value(cx).to_string();
        let balance_text = self.opening_balance.read(cx).value(cx);
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
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Fields::new()
            .child(TextField::new(&self.name).label("Name"))
            .child(
                TextField::new(&self.currency)
                    .label("Currency")
                    .help("A three-letter code, like USD or EUR."),
            )
            .child(
                TextField::new(&self.opening_balance)
                    .label("Opening balance")
                    .help("What the account held before its first record."),
            )
            .children(
                self.error
                    .clone()
                    .map(|error| Note::new(error).tone(Tone::Danger)),
            )
    }
}

/// Opens the form for a new account, or for `account`.
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
    let name = form.read(cx).name.focus_handle(cx);
    let saving = form.clone();
    Form::new(title, form)
        .commit(commit, move |_, cx| saving.update(cx, |form, cx| form.submit(cx)))
        .focus(name)
        .open(window, cx);
}
