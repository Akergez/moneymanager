//! The page of one account: what its balance is made of.
//!
//! A balance is never stored, so the page shows the sum it comes to — the
//! opening balance, then what came in and what went out, each with the number
//! of records that did it — and the one action that changes it without
//! rewriting history: setting the amount the account holds now, which moves
//! the opening balance and leaves every record as it is.

use gpui_adaptive::prim::{Text, column, row};
use gpui_adaptive::{ActiveScale as _, Collection, Command, ListRow, Note, Tone};
use gpui_kit::prelude::*;
use gpui_kit::{App, Context, Entity, SharedString, Subscription, Window};
use money_core::format::format_money;
use money_core::ledger::{AccountBreakdown, Tally};

use crate::book::Book;
use crate::ui::Lucide;

pub struct AccountView {
    book: Entity<Book>,
    account: Vec<u8>,
    _watch: Subscription,
}

impl AccountView {
    pub fn new(book: Entity<Book>, account: Vec<u8>, cx: &mut Context<Self>) -> Self {
        let watch = cx.observe(&book, |_, _, cx| cx.notify());
        AccountView {
            book,
            account,
            _watch: watch,
        }
    }

    /// "USD · 1 record": the page's line under its title, and what a scenario
    /// asks about.
    pub fn detail(&self, cx: &App) -> String {
        let book = self.book.read(cx);
        let parts = book.breakdown(&self.account);
        let records = parts.income.count
            + parts.expenses.count
            + parts.transfers_in.count
            + parts.transfers_out.count;
        let currency = book.account_currency(&self.account);
        match records {
            0 => format!("{currency} · no records"),
            1 => format!("{currency} · 1 record"),
            n => format!("{currency} · {n} records"),
        }
    }

    /// One part of the balance: its name, how many records made it, and what
    /// they came to, signed the way it moves the balance.
    fn part(&self, id: &str, label: &str, tally: Tally, sign: &str, currency: &str) -> ListRow {
        let records = match tally.count {
            1 => "1 record".to_string(),
            n => format!("{n} records"),
        };
        ListRow::new(
            SharedString::from(id.to_string()),
            SharedString::from(label),
        )
        .subtitle(SharedString::from(records))
        .trailing(Text::small(format!(
            "{sign}{}",
            format_money(tally.amount.abs(), currency)
        )))
    }

    /// The rows that add up to the balance, each only while it has records to
    /// go by: an account that never saw a transfer has nothing to say about
    /// them.
    fn render_breakdown(&self, parts: AccountBreakdown, currency: &str) -> Collection {
        let mut rows = vec![
            ListRow::new("part-opening", "Starting balance")
                .subtitle("Before the first record")
                .trailing(Text::small(format_money(parts.opening_balance, currency))),
        ];
        for (id, label, tally, sign) in [
            ("part-income", "Income", parts.income, "+"),
            ("part-expenses", "Expenses", parts.expenses, "−"),
            ("part-transfers-in", "Transfers in", parts.transfers_in, "+"),
            (
                "part-transfers-out",
                "Transfers out",
                parts.transfers_out,
                "−",
            ),
        ] {
            if tally.count > 0 {
                rows.push(self.part(id, label, tally, sign, currency));
            }
        }
        Collection::new("account-breakdown").children(rows)
    }
}

impl Render for AccountView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scale = cx.scale();
        let (parts, currency) = {
            let book = self.book.read(cx);
            let parts = book.breakdown(&self.account);
            let currency = book.account_currency(&self.account).to_string();
            (parts, currency)
        };
        let balance = format_money(parts.balance(), &currency);
        let setting = {
            let (book, account) = (self.book.clone(), self.account.clone());
            Command::new("set-balance", "Set balance…")
                .icon(Lucide::Pencil)
                .on_trigger(move |window, cx| {
                    crate::accounts::open_balance_dialog(&book, &account, window, cx)
                })
                .as_button()
        };
        column()
            .w_full()
            .gap(scale.space_l)
            .p(scale.space_m)
            .child(
                column()
                    .gap(scale.space_xs)
                    .child(Text::small("Balance").muted())
                    .child(Text::title(balance)),
            )
            .child(self.render_breakdown(parts, &currency))
            .child(
                column()
                    .gap(scale.space_s)
                    .child(row().gap(scale.space_m).child(setting))
                    .child(
                        Text::small("Changes the starting balance. Records stay as they are.")
                            .muted(),
                    ),
            )
            .when(parts.balance() < 0.0, |page| {
                page.child(
                    Note::new("This account is in debt: what went out is more than what came in.")
                        .tone(Tone::Warning),
                )
            })
    }
}
