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
        let rate = format_rate(
            tr.rate(),
            currency_of(accounts, &tr.from_account_id),
            currency_of(accounts, &tr.to_account_id),
        );
        if tr.from_account_id == account.id {
            // Outbound leg: this account pays out `amount_from`.
            let counterparty = account_name(accounts, &tr.to_account_id);
            expenses_here.push(Expense {
                id: tr.id.clone(),
                category_id: TRANSFER_CATEGORY_ID.to_vec(),
                amount: tr.amount_from,
                comment: Some(leg_comment("→", counterparty, &rate, tr.comment.as_deref())),
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
                comment: Some(leg_comment("←", counterparty, &rate, tr.comment.as_deref())),
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
    let mut m: HashMap<Vec<u8>, AccountSummary> = accounts
        .iter()
        .map(|a| {
            (
                a.id.clone(),
                AccountSummary {
                    balance: a.opening_balance,
                    ..Default::default()
                },
            )
        })
        .collect();

    let mut add = |id: &[u8], incoming: bool, amount: f64| {
        if let Some(s) = m.get_mut(id) {
            if incoming {
                s.total_in += amount;
                s.balance += amount;
            } else {
                s.total_out += amount;
                s.balance -= amount;
            }
        }
    };
    for e in expenses {
        add(&e.account_id, false, e.amount);
    }
    for t in top_ups {
        add(&t.account_id, true, t.amount);
    }
    for tr in transfers {
        add(&tr.from_account_id, false, tr.amount_from);
        add(&tr.to_account_id, true, tr.amount_to);
    }

    m
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
fn leg_comment(arrow: &str, counterparty: &str, rate: &str, note: Option<&str>) -> String {
    match note {
        Some(n) => format!("{arrow} {counterparty} ({rate}) · {n}"),
        None => format!("{arrow} {counterparty} ({rate})"),
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

        let l1 = ledger_for(&accounts[0], &accounts, &[e.clone()], &[t.clone()], &[]);
        // e lands on A only; t on B only.
        assert_eq!(l1.expenses.len(), 1);
        assert_eq!(l1.top_ups.len(), 0);

        let l2 = ledger_for(&accounts[1], &accounts, &[e.clone()], &[t.clone()], &[]);
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
