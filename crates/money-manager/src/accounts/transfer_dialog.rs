//! The transfer dialog: what left one account and what arrived in another.
//!
//! The two amounts are entered separately because the accounts may be in
//! different currencies; the rate is whatever the two imply, and is shown
//! back as they are typed so a slipped digit is seen before it is saved.

use gpui_adaptive::{
    Choice, ChoiceEvent, ChoiceOption, ChoiceState, DateField, DateFieldState, FieldEvent, Fields,
    Form, Note, Notice, Purpose, TextField, TextFieldState, Tone,
};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Entity, Focusable, SharedString, Subscription, Window};
use money_core::format::parse_amount;
use money_core::ledger::{currency_of, format_rate_both};
use money_core::store::{hex_decode, hex_encode};

use crate::book::{Book, TransferDraft};

struct TransferForm {
    book: Entity<Book>,
    from: Entity<ChoiceState>,
    to: Entity<ChoiceState>,
    amount_from: Entity<TextFieldState>,
    amount_to: Entity<TextFieldState>,
    date: Entity<DateFieldState>,
    comment: Entity<TextFieldState>,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl TransferForm {
    fn new(book: Entity<Book>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (options, keys, current) = {
            let book = book.read(cx);
            let accounts: Vec<_> = book.visible_accounts().cloned().collect();
            let options: Vec<ChoiceOption> = accounts
                .iter()
                .map(|account| {
                    ChoiceOption::new(
                        hex_encode(&account.id),
                        format!("{} · {}", account.name, account.currency),
                    )
                })
                .collect();
            let keys: Vec<SharedString> = options.iter().map(|o| o.key().clone()).collect();
            let current = book
                .current_account()
                .and_then(|current| accounts.iter().position(|a| a.id == current.id))
                .unwrap_or(0);
            (options, keys, current)
        };
        // Out of the account being looked at, into the next one.
        let other = (current + 1) % keys.len().max(1);
        let (first, second) = (keys.get(current).cloned(), keys.get(other).cloned());
        let from = cx.new(|cx| ChoiceState::new(options.clone(), first, window, cx));
        let to = cx.new(|cx| ChoiceState::new(options, second, window, cx));
        let mut amount = || {
            cx.new(|cx| {
                let mut state = TextFieldState::amount(window, cx);
                state.set_placeholder("0.00", window, cx);
                state
            })
        };
        let (amount_from, amount_to) = (amount(), amount());
        let date = cx.new(|cx| DateFieldState::new(Some(crate::today()), window, cx));
        let comment = cx.new(|cx| {
            let mut state = TextFieldState::new(Purpose::Message, window, cx);
            state.set_placeholder("Optional", window, cx);
            state
        });

        // The rate line and the two currency labels follow what is typed
        // and chosen.
        let redraw_on_typing =
            |this: &mut Self, _: Entity<TextFieldState>, event: &FieldEvent, cx: &mut Context<Self>| {
                if matches!(event, FieldEvent::Change) {
                    this.error = None;
                    cx.notify();
                }
            };
        let subscriptions = vec![
            cx.subscribe(&amount_from, redraw_on_typing),
            cx.subscribe(&amount_to, redraw_on_typing),
            cx.subscribe(&from, |_, _, _: &ChoiceEvent, cx| cx.notify()),
            cx.subscribe(&to, |_, _, _: &ChoiceEvent, cx| cx.notify()),
        ];
        TransferForm {
            book,
            from,
            to,
            amount_from,
            amount_to,
            date,
            comment,
            error: None,
            _subscriptions: subscriptions,
        }
    }

    /// The currencies of the two chosen accounts.
    fn currencies(&self, cx: &App) -> (String, String) {
        let book = self.book.read(cx);
        let currency = |choice: &Entity<ChoiceState>| {
            chosen_account(choice, cx)
                .map(|id| currency_of(book.accounts(), &id).to_string())
                .unwrap_or_default()
        };
        (currency(&self.from), currency(&self.to))
    }

    /// "1 USD = 92.35 RUB · 1 RUB = 0.0108 USD", once both amounts are in.
    fn rate(&self, cx: &App) -> Option<String> {
        let sent = parse_amount(&self.amount_from.read(cx).value(cx)).filter(|v| *v > 0.0)?;
        let received = parse_amount(&self.amount_to.read(cx).value(cx)).filter(|v| *v > 0.0)?;
        let (from, to) = self.currencies(cx);
        Some(format_rate_both(received / sent, &from, &to))
    }

    fn read(&self, cx: &App) -> Result<TransferDraft, String> {
        let (Some(from), Some(to)) = (chosen_account(&self.from, cx), chosen_account(&self.to, cx))
        else {
            return Err("Choose both accounts.".to_string());
        };
        let amount = |input: &Entity<TextFieldState>| {
            parse_amount(&input.read(cx).value(cx))
                .ok_or_else(|| "Both amounts must be numbers greater than 0.".to_string())
        };
        let (amount_from, amount_to) = (amount(&self.amount_from)?, amount(&self.amount_to)?);
        let date = self
            .date
            .read(cx)
            .date()
            .ok_or("Date is not a date. Write it like 2026-10-04.")?;
        let comment = self.comment.read(cx).value(cx).trim().to_string();
        Ok(TransferDraft {
            from,
            to,
            amount_from,
            amount_to,
            comment: (!comment.is_empty()).then_some(comment),
            date,
        })
    }

    fn submit(&mut self, cx: &mut Context<Self>) -> bool {
        let result = self.read(cx).and_then(|draft| {
            self.book
                .update(cx, |book, cx| book.add_transfer(&draft, cx))
        });
        match result {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error.into());
                cx.notify();
                false
            }
        }
    }
}

/// The id of the account `choice` is on.
fn chosen_account(choice: &Entity<ChoiceState>, cx: &App) -> Option<Vec<u8>> {
    choice.read(cx).selected().map(|key| hex_decode(key))
}

impl Render for TransferForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (from_currency, to_currency) = self.currencies(cx);
        let rate = self.rate(cx);
        let received = TextField::new(&self.amount_to).label(format!("Amount received, {to_currency}"));
        Fields::new()
            .child(Choice::new(&self.from).label("From").id("transfer-from"))
            .child(TextField::new(&self.amount_from).label(format!("Amount sent, {from_currency}")))
            .child(Choice::new(&self.to).label("To").id("transfer-to"))
            .child(match rate {
                Some(rate) => received.help(rate),
                None => received,
            })
            .child(DateField::new(&self.date).label("Date"))
            .child(TextField::new(&self.comment).label("Comment"))
            .children(
                self.error
                    .clone()
                    .map(|error| Note::new(error).tone(Tone::Danger)),
            )
    }
}

/// Opens the form for a new transfer out of the current account.
pub fn open_transfer_dialog(book: &Entity<Book>, window: &mut Window, cx: &mut App) {
    // A transfer needs somewhere to go: with one account, the next step is
    // making the second, and that is where this leads.
    if book.read(cx).visible_accounts().count() < 2 {
        Notice::new("A transfer needs two accounts. Add the second one first.").push(window, cx);
        return super::open_account_dialog(book, None, window, cx);
    }
    let form = cx.new(|cx| TransferForm::new(book.clone(), window, cx));
    let amount = form.read(cx).amount_from.focus_handle(cx);
    let saving = form.clone();
    Form::new("New transfer", form)
        .commit("Transfer", move |_, cx| saving.update(cx, |form, cx| form.submit(cx)))
        .focus(amount)
        .open(window, cx);
}
