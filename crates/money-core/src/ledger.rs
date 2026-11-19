//! Domain logic for per-account ledgers and transfers. Pure functions with no
//! TUI dependency, so they can be tested in isolation.
//!
//! Transfers are stored as a single record; their two legs (the expense on the
//! source side and the top-up on the destination side) are *derived* at read
//! time. This keeps a transfer's deletion atomic under sync (one tombstone) and
//! guarantees the legs can't drift from the transfer itself.

use std::collections::HashMap;

use crate::models::{Account, Category, Expense, TopUp, TopUpCategory, Transfer, TransferLeg};

/// Virtual category that stands in for a transfer in the per-account ledger and
/// on charts. It is not a real category in the `categories` collection.
pub const TRANSFER_CATEGORY_ID: [u8; 16] = [0xFF; 16];
/// Display name for the virtual transfer category.
pub const TRANSFER_CATEGORY_NAME: &str = "⇄ Перевод";

/// Totals of a single account.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AccountSummary {
    /// opening_balance + total_in - total_out.
    pub balance: f64,
    /// Sum of all money that came in (top-ups + inbound transfer legs).
    pub total_in: f64,
    /// Sum of all money that went out (expenses + outbound transfer legs).
    pub total_out: f64,
}

/// Derived ledger for a single account: the account's real expenses/top-ups
/// plus the virtual transfer legs that belong to it. Its totals come from
/// [`summaries`].
#[derive(Debug, Clone)]
pub struct AccountLedger {
    pub expenses: Vec<Expense>,
    pub top_ups: Vec<TopUp>,
}

/// Name of `id` in `accounts`, or "Unknown" if the account is orphaned.
pub fn account_name<'a>(accounts: &'a [Account], id: &[u8]) -> &'a str {
    accounts
        .iter()
        .find(|a| a.id.as_slice() == id)
        .map(|a| a.name.as_str())
        .unwrap_or("Unknown")
}

/// Currency code of the account referenced by `id`, or "?" if absent.
pub fn currency_of<'a>(accounts: &'a [Account], id: &[u8]) -> &'a str {
    accounts
        .iter()
        .find(|a| a.id.as_slice() == id)
        .map(|a| a.currency.as_str())
        .unwrap_or("?")
}

/// Build the ledger for `account`: its real expenses/top-ups plus the transfer
/// legs that touch it (outbound -> expense, inbound -> top-up).
pub fn ledger_for(
    account: &Account,
    accounts: &[Account],
    expenses: &[Expense],
    top_ups: &[TopUp],
    transfers: &[Transfer],
) -> AccountLedger {
    let mut expenses_here: Vec<Expense> = expenses
        .iter()
        .filter(|e| e.account_id == account.id)
        .cloned()
        .collect();
    let mut top_ups_here: Vec<TopUp> = top_ups
        .iter()
        .filter(|t| t.account_id == account.id)
        .cloned()
        .collect();

    for tr in transfers {
        let (from_currency, to_currency) = (
            currency_of(accounts, &tr.from_account_id),
            currency_of(accounts, &tr.to_account_id),
        );
        // Within one currency there is no rate worth writing down.
        let rate = (from_currency != to_currency)
            .then(|| format_rate(tr.rate(), from_currency, to_currency));
        if tr.from_account_id == account.id {
            // Outbound leg: this account pays out `amount_from`.
            let counterparty = account_name(accounts, &tr.to_account_id);
            expenses_here.push(Expense {
                id: tr.id.clone(),
                category_id: TRANSFER_CATEGORY_ID.to_vec(),
                amount: tr.amount_from,
                comment: Some(leg_comment(
                    "→",
                    counterparty,
                    rate.as_deref(),
                    tr.comment.as_deref(),
                )),
                date: tr.date,
                account_id: account.id.clone(),
                transfer: Some(TransferLeg {
                    transfer_id: tr.id.clone(),
                }),
            });
        }
        if tr.to_account_id == account.id {
            // Inbound leg: this account receives `amount_to`.
            let counterparty = account_name(accounts, &tr.from_account_id);
            top_ups_here.push(TopUp {
                id: tr.id.clone(),
                category_id: TRANSFER_CATEGORY_ID.to_vec(),
                amount: tr.amount_to,
                comment: Some(leg_comment(
                    "←",
                    counterparty,
                    rate.as_deref(),
                    tr.comment.as_deref(),
                )),
                date: tr.date,
                account_id: account.id.clone(),
                transfer: Some(TransferLeg {
                    transfer_id: tr.id.clone(),
                }),
            });
        }
    }

    AccountLedger {
        expenses: expenses_here,
        top_ups: top_ups_here,
    }
}

/// One part of a balance: how many records it takes and what they come to.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Tally {
    pub count: usize,
    pub amount: f64,
}

/// What an account's balance is made of: where it started and every kind of
/// record that moved it since, each part with its own count and total.
///
/// This is the arithmetic [`summaries`] reports, opened up: a balance is
/// `opening_balance + income + transfers_in - expenses - transfers_out`, all
/// in the account's own currency.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AccountBreakdown {
    pub opening_balance: f64,
    /// The account's own expenses, its transfer legs left out.
    pub expenses: Tally,
    /// The account's own top-ups, its transfer legs left out.
    pub income: Tally,
    /// Legs of the transfers that left this account.
    pub transfers_out: Tally,
    /// Legs of the transfers that arrived in this account.
    pub transfers_in: Tally,
}

impl AccountBreakdown {
    /// What came in: top-ups and transfers that arrived.
    pub fn total_in(&self) -> f64 {
        self.income.amount + self.transfers_in.amount
    }

    /// What went out: expenses and transfers that left.
    pub fn total_out(&self) -> f64 {
        self.expenses.amount + self.transfers_out.amount
    }

    /// The balance this adds up to.
    pub fn balance(&self) -> f64 {
        self.opening_balance + self.total_in() - self.total_out()
    }
}

/// The parts of `account`'s balance. `expenses` and `top_ups` may be either
/// the stored records or a ledger's; a transfer leg is counted once, from the
/// transfer itself.
pub fn breakdown(
    account: &Account,
    expenses: &[Expense],
    top_ups: &[TopUp],
    transfers: &[Transfer],
) -> AccountBreakdown {
    let mut parts = AccountBreakdown {
        opening_balance: account.opening_balance,
        ..Default::default()
    };
    for expense in expenses
        .iter()
        .filter(|e| e.account_id == account.id && e.transfer.is_none())
    {
        parts.expenses.count += 1;
        parts.expenses.amount += expense.amount;
    }
    for top_up in top_ups
        .iter()
        .filter(|t| t.account_id == account.id && t.transfer.is_none())
    {
        parts.income.count += 1;
        parts.income.amount += top_up.amount;
    }
    for tr in transfers {
        if tr.from_account_id == account.id {
            parts.transfers_out.count += 1;
            parts.transfers_out.amount += tr.amount_from;
        }
        if tr.to_account_id == account.id {
            parts.transfers_in.count += 1;
            parts.transfers_in.amount += tr.amount_to;
        }
    }
    parts
}

/// Totals of every account, keyed by account id:
/// balance = opening_balance + top-ups + inbound transfers - expenses - outbound
/// transfers, over all live records. Amounts in different currencies are never
/// summed across accounts, and records pointing at an unknown (orphaned)
/// account are not attributed to any account.
pub fn summaries(
    accounts: &[Account],
    expenses: &[Expense],
    top_ups: &[TopUp],
    transfers: &[Transfer],
) -> HashMap<Vec<u8>, AccountSummary> {
    accounts
        .iter()
        .map(|account| {
            let parts = breakdown(account, expenses, top_ups, transfers);
            (
                account.id.clone(),
                AccountSummary {
                    balance: parts.balance(),
                    total_in: parts.total_in(),
                    total_out: parts.total_out(),
                },
            )
        })
        .collect()
}

/// Format a rate with enough precision to be useful for both strong and weak
/// currencies: 92.35, 0.0108, 0.000123.
pub fn format_rate_value(rate: f64) -> String {
    let abs = rate.abs();
    if abs >= 1.0 {
        format!("{rate:.2}")
    } else if abs >= 0.01 {
        format!("{rate:.4}")
    } else {
        format!("{rate:.6}")
    }
}

/// "1 USD = 92.35 RUB": one unit of the source currency in the destination one.
pub fn format_rate(rate: f64, from_currency: &str, to_currency: &str) -> String {
    format!(
        "1 {from_currency} = {} {to_currency}",
        format_rate_value(rate)
    )
}

/// Both directions: "1 USD = 92.35 RUB · 1 RUB = 0.0108 USD".
pub fn format_rate_both(rate: f64, from_currency: &str, to_currency: &str) -> String {
    format!(
        "{} · {}",
        format_rate(rate, from_currency, to_currency),
        format_rate(1.0 / rate, to_currency, from_currency)
    )
}

/// Comment shown on a transfer leg: "→ Card USD (1 RUB = 0.0108 USD) · note".
/// A transfer within one currency has no rate, so the brackets are left out.
fn leg_comment(arrow: &str, counterparty: &str, rate: Option<&str>, note: Option<&str>) -> String {
    match (rate, note) {
        (Some(rate), Some(n)) => format!("{arrow} {counterparty} ({rate}) · {n}"),
        (Some(rate), None) => format!("{arrow} {counterparty} ({rate})"),
        (None, Some(n)) => format!("{arrow} {counterparty} · {n}"),
        (None, None) => format!("{arrow} {counterparty}"),
    }
}

/// Add the virtual transfer category so charts can label and (optionally)
/// hide transfer legs. The virtual category is *not* offered to the user in
/// the creation forms.
pub fn with_transfer_category(categories: &[Category]) -> Vec<Category> {
    let mut c = categories.to_vec();
    c.push(Category {
        id: TRANSFER_CATEGORY_ID.to_vec(),
        name: TRANSFER_CATEGORY_NAME.to_string(),
        ..Default::default()
    });
    c
}

pub fn with_transfer_top_up_category(categories: &[TopUpCategory]) -> Vec<TopUpCategory> {
    let mut c = categories.to_vec();
    c.push(TopUpCategory {
        id: TRANSFER_CATEGORY_ID.to_vec(),
        name: TRANSFER_CATEGORY_NAME.to_string(),
        ..Default::default()
    });
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn id(v: u64) -> Vec<u8> {
        v.to_be_bytes().to_vec()
    }

    fn date() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
    }

    fn account(id_: Vec<u8>, name: &str, currency: &str, opening_balance: f64) -> Account {
        Account {
            id: id_,
            name: name.into(),
            currency: currency.into(),
            opening_balance,
        }
    }

    fn expense(id_: u64, account_id: &[u8], amount: f64) -> Expense {
        Expense {
            id: id(id_),
            category_id: id(9),
            amount,
            comment: None,
            date: date(),
            account_id: account_id.to_vec(),
            transfer: None,
        }
    }

    fn top_up(id_: u64, account_id: &[u8], amount: f64) -> TopUp {
        TopUp {
            id: id(id_),
            category_id: id(9),
            amount,
            comment: None,
            date: date(),
            account_id: account_id.to_vec(),
            transfer: None,
        }
    }

    fn transfer(from: &[u8], to: &[u8], amount_from: f64, amount_to: f64) -> Transfer {
        Transfer {
            id: id(20),
            from_account_id: from.to_vec(),
            to_account_id: to.to_vec(),
            amount_from,
            amount_to,
            comment: None,
            date: date(),
        }
    }

    #[test]
    fn ledger_isolates_records_per_account() {
        let (a1, a2) = (id(1), id(2));
        let e = expense(10, &a1, 5.0);
        let t = top_up(11, &a2, 20.0);
        let accounts = vec![
            account(a1.clone(), "A", "RUB", 100.0),
            account(a2.clone(), "B", "USD", 50.0),
        ];

        let l1 = ledger_for(
            &accounts[0],
            &accounts,
            std::slice::from_ref(&e),
            std::slice::from_ref(&t),
            &[],
        );
        // e lands on A only; t on B only.
        assert_eq!(l1.expenses.len(), 1);
        assert_eq!(l1.top_ups.len(), 0);

        let l2 = ledger_for(
            &accounts[1],
            &accounts,
            std::slice::from_ref(&e),
            std::slice::from_ref(&t),
            &[],
        );
        assert_eq!(l2.expenses.len(), 0);
        assert_eq!(l2.top_ups.len(), 1);

        let m = summaries(&accounts, &[e], &[t], &[]);
        assert_eq!(
            m[&a1],
            AccountSummary {
                balance: 95.0,
                total_in: 0.0,
                total_out: 5.0
            }
        );
        assert_eq!(m[&a2].balance, 70.0);
    }

    #[test]
    fn transfer_creates_outgoing_expense_and_inbound_top_up() {
        let (a1, a2) = (id(1), id(2));
        let mut tr = transfer(&a1, &a2, 100.0, 9235.0);
        tr.comment = Some("savings".into());
        let accounts = vec![
            account(a1.clone(), "Card", "USD", 0.0),
            account(a2.clone(), "Cash", "RUB", 0.0),
        ];

        let l1 = ledger_for(&accounts[0], &accounts, &[], &[], &[tr.clone()]);
        assert_eq!(l1.expenses.len(), 1);
        assert_eq!(l1.top_ups.len(), 0);
        let leg = &l1.expenses[0];
        assert_eq!(leg.amount, 100.0);
        assert_eq!(leg.category_id, TRANSFER_CATEGORY_ID.to_vec());
        assert_eq!(leg.transfer.as_ref().unwrap().transfer_id, tr.id);
        assert_eq!(
            leg.comment.as_deref(),
            Some("→ Cash (1 USD = 92.35 RUB) · savings")
        );

        let l2 = ledger_for(&accounts[1], &accounts, &[], &[], &[tr.clone()]);
        assert_eq!(l2.expenses.len(), 0);
        assert_eq!(l2.top_ups.len(), 1);
        assert_eq!(l2.top_ups[0].amount, 9235.0);
        assert_eq!(
            l2.top_ups[0].comment.as_deref(),
            Some("← Card (1 USD = 92.35 RUB) · savings")
        );

        let m = summaries(&accounts, &[], &[], &[tr]);
        assert_eq!(
            m[&a1],
            AccountSummary {
                balance: -100.0,
                total_in: 0.0,
                total_out: 100.0
            }
        );
        assert_eq!(
            m[&a2],
            AccountSummary {
                balance: 9235.0,
                total_in: 9235.0,
                total_out: 0.0
            }
        );
    }

    #[test]
    fn a_transfer_within_one_currency_says_no_rate() {
        let (a1, a2) = (id(1), id(2));
        let mut tr = transfer(&a1, &a2, 250.0, 250.0);
        tr.comment = Some("savings".into());
        let accounts = vec![
            account(a1.clone(), "Wallet", "RUB", 0.0),
            account(a2.clone(), "Cash", "RUB", 0.0),
        ];

        let l1 = ledger_for(&accounts[0], &accounts, &[], &[], &[tr.clone()]);
        assert_eq!(l1.expenses[0].comment.as_deref(), Some("→ Cash · savings"));
        let l2 = ledger_for(&accounts[1], &accounts, &[], &[], &[tr]);
        assert_eq!(l2.top_ups[0].comment.as_deref(), Some("← Wallet · savings"));

        // Without a note the counterparty stands alone.
        let bare = transfer(&a1, &a2, 250.0, 250.0);
        let l1 = ledger_for(&accounts[0], &accounts, &[], &[], &[bare]);
        assert_eq!(l1.expenses[0].comment.as_deref(), Some("→ Cash"));
    }

    #[test]
    fn rate_formatting_including_same_currency_and_weak_currency() {
        let same = transfer(&id(1), &id(2), 4.0, 4.0);
        assert_eq!(same.rate(), 1.0);
        assert_eq!(format_rate(same.rate(), "EUR", "EUR"), "1 EUR = 1.00 EUR");

        let usd_rub = transfer(&id(1), &id(2), 100.0, 9235.0);
        assert_eq!(
            format_rate(usd_rub.rate(), "USD", "RUB"),
            "1 USD = 92.35 RUB"
        );
        assert_eq!(
            format_rate_both(usd_rub.rate(), "USD", "RUB"),
            "1 USD = 92.35 RUB · 1 RUB = 0.0108 USD"
        );
        // A weak source currency still shows significant digits.
        assert_eq!(format_rate_value(0.000123), "0.000123");
    }

    #[test]
    fn a_breakdown_names_every_part_of_the_balance() {
        let (a1, a2) = (id(1), id(2));
        let accounts = vec![
            account(a1.clone(), "Card", "USD", 10.0),
            account(a2.clone(), "Cash", "RUB", 0.0),
        ];
        let expenses = [
            expense(10, &a1, 5.0),
            expense(11, &a1, 2.5),
            // Another account's records are none of this balance.
            expense(12, &a2, 999.0),
        ];
        let top_ups = [top_up(13, &a1, 100.0)];
        // 100 USD out to Cash, 9 235 RUB in; 20 RUB back the other way.
        let transfers = [
            transfer(&a1, &a2, 100.0, 9235.0),
            transfer(&a2, &a1, 20.0, 0.22),
        ];

        let card = breakdown(&accounts[0], &expenses, &top_ups, &transfers);
        assert_eq!(
            card,
            AccountBreakdown {
                opening_balance: 10.0,
                expenses: Tally {
                    count: 2,
                    amount: 7.5
                },
                income: Tally {
                    count: 1,
                    amount: 100.0
                },
                transfers_out: Tally {
                    count: 1,
                    amount: 100.0
                },
                transfers_in: Tally {
                    count: 1,
                    amount: 0.22
                },
            }
        );
        // 10 + 100 + 0.22 - 7.5 - 100
        assert!((card.balance() - 2.72).abs() < 1e-9);
        assert!((card.total_in() - 100.22).abs() < 1e-9);
        assert!((card.total_out() - 107.5).abs() < 1e-9);

        // The same parts are what summaries has: one formula, not two.
        let m = summaries(&accounts, &expenses, &top_ups, &transfers);
        for account in &accounts {
            let parts = breakdown(account, &expenses, &top_ups, &transfers);
            assert_eq!(m[&account.id].balance, parts.balance());
            assert_eq!(m[&account.id].total_in, parts.total_in());
            assert_eq!(m[&account.id].total_out, parts.total_out());
        }
    }

    #[test]
    fn a_ledger_s_own_legs_are_not_counted_twice() {
        let (a1, a2) = (id(1), id(2));
        let accounts = vec![
            account(a1.clone(), "Card", "USD", 0.0),
            account(a2.clone(), "Cash", "RUB", 0.0),
        ];
        let transfers = [transfer(&a1, &a2, 100.0, 9235.0)];
        let ledger = ledger_for(&accounts[0], &accounts, &[], &[], &transfers);

        let parts = breakdown(
            &accounts[0],
            &ledger.expenses,
            &ledger.top_ups,
            &transfers,
        );
        assert_eq!(parts.expenses.count, 0);
        assert_eq!(parts.transfers_out.count, 1);
        assert_eq!(parts.balance(), -100.0);
    }

    #[test]
    fn summaries_match_formula_and_ignore_orphans() {
        let (a1, a2, orph) = (id(1), id(2), id(3));
        let accounts = vec![
            account(a1.clone(), "A", "RUB", 100.0),
            account(a2.clone(), "B", "USD", 50.0),
        ];
        let m = summaries(
            &accounts,
            &[expense(10, &a1, 5.0)],
            &[top_up(11, &a2, 20.0)],
            &[
                transfer(&a1, &orph, 100.0, 923.5),
                transfer(&a2, &a1, 10.0, 900.0),
            ],
        );
        // A: 100 - 5 (expense) - 100 (to orphan) + 900 (from B)
        assert_eq!(
            m[&a1],
            AccountSummary {
                balance: 895.0,
                total_in: 900.0,
                total_out: 105.0
            }
        );
        // B: 50 + 20 (top-up) - 10 (to A)
        assert_eq!(
            m[&a2],
            AccountSummary {
                balance: 60.0,
                total_in: 20.0,
                total_out: 10.0
            }
        );
        assert!(!m.contains_key(&orph));
    }

    #[test]
    fn orphaned_transfer_still_shows_in_ledger_with_unknown_name() {
        let (a1, orph) = (id(1), id(2));
        let accounts = vec![account(a1.clone(), "A", "RUB", 0.0)];
        let l1 = ledger_for(
            &accounts[0],
            &accounts,
            &[],
            &[],
            &[transfer(&a1, &orph, 10.0, 92.35)],
        );
        assert_eq!(l1.expenses.len(), 1);
        assert!(
            l1.expenses[0]
                .comment
                .as_deref()
                .unwrap_or("")
                .contains("Unknown")
        );
    }

    #[test]
    fn with_transfer_category_appends_virtual() {
        let real = Category {
            id: id(1),
            name: "Food".into(),
            ..Default::default()
        };
        let ext = with_transfer_category(&[real]);
        assert_eq!(ext.len(), 2);
        assert_eq!(ext[0].id, id(1));
        assert_eq!(ext[1].id, TRANSFER_CATEGORY_ID.to_vec());
        assert_eq!(ext[1].name, "⇄ Перевод");

        let real_up = TopUpCategory {
            id: id(1),
            name: "Salary".into(),
            ..Default::default()
        };
        let ext_up = with_transfer_top_up_category(&[real_up]);
        assert_eq!(ext_up.len(), 2);
        assert_eq!(ext_up[1].id, TRANSFER_CATEGORY_ID.to_vec());
        assert_eq!(ext_up[1].name, TRANSFER_CATEGORY_NAME);
    }
}
