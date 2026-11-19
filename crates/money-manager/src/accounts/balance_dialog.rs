//! The form behind "Set balance…": the amount an account holds now.
//!
//! It is the account's amount that is typed, not its opening balance, because
//! that is the figure a person knows. What it writes is the opening balance:
//! it takes the difference between the amount typed and the one the records
//! add up to, so the balance comes out as typed and no record is touched.

use gpui_adaptive::{Fields, Form, Note, Purpose, TextField, TextFieldState, Tone};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Entity, Focusable, SharedString, Window};
use money_core::format::{format_amount, parse_amount};

use crate::book::Book;

struct BalanceForm {
    book: Entity<Book>,
    account: Vec<u8>,
    balance: Entity<TextFieldState>,
    error: Option<SharedString>,
}

impl BalanceForm {
    fn new(
        book: Entity<Book>,
        account: Vec<u8>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let balance = book.read(cx).summary(&account).balance;
        let balance = cx.new(|cx| {
            let mut state = TextFieldState::new(Purpose::Number, window, cx);
            state.set_placeholder("0.00", window, cx);
            state.set_value(format_amount(balance).replace(' ', ""), window, cx);
            state
        });
        BalanceForm {
            book,
            account,
            balance,
            error: None,
        }
    }

    /// Writes the amount typed as the account's balance. Answers whether it
    /// did: `false` keeps the form up with what is wrong said in it.
    fn submit(&mut self, cx: &mut Context<Self>) -> bool {
        let typed = self.balance.read(cx).value(cx);
        let Some(balance) = parse_amount(&typed) else {
            self.error = Some("Write the amount the account holds.".into());
            cx.notify();
            return false;
        };
        let account = self.account.clone();
        let saved = self.book.update(cx, |book, cx| {
            book.set_account_balance(&account, balance, cx)
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

impl Render for BalanceForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let currency = self
            .book
            .read(cx)
            .account_currency(&self.account)
            .to_string();
        Fields::new()
            .child(
                TextField::new(&self.balance)
                    .label("Balance")
                    .help(format!("What the account holds now, in {currency}.")),
            )
            .child(Note::new(
                "Records stay as they are: the starting balance takes the difference.",
            ))
            .children(
                self.error
                    .clone()
                    .map(|error| Note::new(error).tone(Tone::Danger)),
            )
    }
}

/// Opens the form that sets the balance of `account_id`.
pub fn open_balance_dialog(
    book: &Entity<Book>,
    account_id: &[u8],
    window: &mut Window,
    cx: &mut App,
) {
    let form = cx.new(|cx| BalanceForm::new(book.clone(), account_id.to_vec(), window, cx));
    let balance = form.read(cx).balance.focus_handle(cx);
    let saving = form.clone();
    Form::new("Set balance", form)
        .commit("Set", move |_, cx| {
            saving.update(cx, |form, cx| form.submit(cx))
        })
        .focus(balance)
        .open(window, cx);
}
