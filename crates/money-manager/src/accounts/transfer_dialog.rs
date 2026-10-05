//! The transfer dialog: what left one account and what arrived in another.
//!
//! The two amounts are entered separately because the accounts may be in
//! different currencies; the rate is whatever the two imply, and is shown
//! back as they are typed so a slipped digit is seen before it is saved.

use crate::date_field::DateField;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::{ActiveTheme, IndexPath, WindowExt, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{App, AppContext, Context, Entity, SharedString, Subscription, Window, div};
use money_core::format::parse_amount;
use money_core::ledger::{currency_of, format_rate_both};

use crate::book::{Book, TransferDraft};
use crate::ui::{self, Choice};

struct TransferForm {
    book: Entity<Book>,
    from: Entity<SelectState<Vec<Choice>>>,
    to: Entity<SelectState<Vec<Choice>>>,
    amount_from: Entity<InputState>,
    amount_to: Entity<InputState>,
    date: Entity<DateField>,
    comment: Entity<InputState>,
    error: Option<SharedString>,
    _subscriptions: Vec<Subscription>,
}

impl TransferForm {
    fn new(book: Entity<Book>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (choices, current) = {
            let book = book.read(cx);
            let choices: Vec<Choice> = book
                .accounts()
                .iter()
                .map(|account| {
                    Choice::new(
                        account.id.clone(),
                        format!("{} · {}", account.name, account.currency),
                    )
                })
                .collect();
            let current = book
                .current_account()
                .and_then(|current| choices.iter().position(|c| c.id == current.id))
                .unwrap_or(0);
            (choices, current)
        };
        // Out of the account being looked at, into the next one.
        let other = (current + 1) % choices.len().max(1);
        let from = cx.new(|cx| {
            SelectState::new(
                choices.clone(),
                Some(IndexPath::default().row(current)),
                window,
                cx,
            )
        });
        let to = cx
            .new(|cx| SelectState::new(choices, Some(IndexPath::default().row(other)), window, cx));
        let amount_from = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
        let amount_to = cx.new(|cx| InputState::new(window, cx).placeholder("0.00"));
        let date = cx.new(|cx| DateField::new(crate::today(), window, cx));
        let comment = cx.new(|cx| InputState::new(window, cx).placeholder("Optional"));

        // The rate line and the two currency labels follow what is typed
        // and chosen.
        let redraw_on_typing =
            |this: &mut Self, _: Entity<InputState>, event: &InputEvent, cx: &mut Context<Self>| {
                if matches!(event, InputEvent::Change) {
                    this.error = None;
                    cx.notify();
                }
            };
        let subscriptions = vec![
            cx.subscribe(&amount_from, redraw_on_typing),
            cx.subscribe(&amount_to, redraw_on_typing),
            cx.subscribe(&from, |_, _, _: &SelectEvent<Vec<Choice>>, cx| cx.notify()),
            cx.subscribe(&to, |_, _, _: &SelectEvent<Vec<Choice>>, cx| cx.notify()),
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
        let currency = |select: &Entity<SelectState<Vec<Choice>>>| {
            select
                .read(cx)
                .selected_value()
                .map(|id| currency_of(book.accounts(), id).to_string())
                .unwrap_or_default()
        };
        (currency(&self.from), currency(&self.to))
    }

    /// "1 USD = 92.35 RUB · 1 RUB = 0.0108 USD", once both amounts are in.
    fn rate(&self, cx: &App) -> Option<String> {
        let sent = parse_amount(&self.amount_from.read(cx).value()).filter(|v| *v > 0.0)?;
        let received = parse_amount(&self.amount_to.read(cx).value()).filter(|v| *v > 0.0)?;
        let (from, to) = self.currencies(cx);
        Some(format_rate_both(received / sent, &from, &to))
    }

    fn read(&self, cx: &App) -> Result<TransferDraft, String> {
        let chosen =
            |select: &Entity<SelectState<Vec<Choice>>>| select.read(cx).selected_value().cloned();
        let (Some(from), Some(to)) = (chosen(&self.from), chosen(&self.to)) else {
            return Err("Choose both accounts.".to_string());
        };
        let amount = |input: &Entity<InputState>| {
            parse_amount(&input.read(cx).value())
                .ok_or_else(|| "Both amounts must be numbers greater than 0.".to_string())
        };
        let (amount_from, amount_to) = (amount(&self.amount_from)?, amount(&self.amount_to)?);
        let date = self
            .date
            .read(cx)
            .date(cx)
            .ok_or("Date is not a date. Write it like 2026-10-04.")?;
        let comment = self.comment.read(cx).value().trim().to_string();
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

impl Render for TransferForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (from_currency, to_currency) = self.currencies(cx);
        let rate = self.rate(cx);
        v_flex()
            .gap_3()
            .child(ui::field("From", ui::choosing(Select::new(&self.from))))
            .child(ui::field(
                format!("Amount sent, {from_currency}"),
                Input::new(&self.amount_from),
            ))
            .child(ui::field("To", ui::choosing(Select::new(&self.to))))
            .child(
                ui::field(
                    format!("Amount received, {to_currency}"),
                    Input::new(&self.amount_to),
                )
                .children(rate.map(|rate| {
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(rate)
                })),
            )
            .child(ui::field("Date", self.date.clone()))
            .child(ui::field("Comment", Input::new(&self.comment)))
            .children(self.error.clone().map(|error| ui::form_error(error, cx)))
    }
}

/// Opens the dialog for a new transfer out of the current account.
pub fn open_transfer_dialog(book: &Entity<Book>, window: &mut Window, cx: &mut App) {
    // A transfer needs somewhere to go: with one account, the next step is
    // making the second, and that is where this leads.
    if book.read(cx).accounts().len() < 2 {
        window.push_notification(
            Notification::info("A transfer needs two accounts. Add the second one first."),
            cx,
        );
        return super::open_account_dialog(book, None, window, cx);
    }
    let form = cx.new(|cx| TransferForm::new(book.clone(), window, cx));
    let amount = gpui_kit::Focusable::focus_handle(&form.read(cx).amount_from, cx);
    ui::open_form(
        "New transfer",
        "Transfer",
        form,
        Some(amount),
        TransferForm::submit,
        window,
        cx,
    );
}
