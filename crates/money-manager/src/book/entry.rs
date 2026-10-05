//! One line of an account's list, whichever kind it is.

use chrono::NaiveDate;
use money_core::models::{Expense, TopUp};

/// Which half of the ledger a screen is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    #[default]
    Expense,
    Income,
}

impl Mode {
    pub const ALL: [Mode; 2] = [Mode::Expense, Mode::Income];

    /// The plural a tab or a heading wears.
    pub fn title(self) -> &'static str {
        match self {
            Mode::Expense => "Expenses",
            Mode::Income => "Income",
        }
    }

    /// The singular a command names: "New expense…".
    pub fn noun(self) -> &'static str {
        match self {
            Mode::Expense => "expense",
            Mode::Income => "income",
        }
    }

    /// The sign an amount of this kind is written with.
    pub fn sign(self) -> &'static str {
        match self {
            Mode::Expense => "−",
            Mode::Income => "+",
        }
    }

    pub fn index(self) -> usize {
        match self {
            Mode::Expense => 0,
            Mode::Income => 1,
        }
    }

    pub fn from_index(index: usize) -> Self {
        if index == 1 {
            Mode::Income
        } else {
            Mode::Expense
        }
    }
}

/// An expense or an income, or the leg of a transfer standing in for one.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: Vec<u8>,
    pub category_id: Vec<u8>,
    pub amount: f64,
    pub comment: Option<String>,
    pub date: NaiveDate,
    /// A leg is derived from its transfer and cannot be edited by itself;
    /// deleting it deletes the transfer.
    pub is_transfer: bool,
}

/// What a person filled in for an expense or an income, before it is a
/// record: everything but the id and the account, which the book supplies.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryDraft {
    pub category_id: Vec<u8>,
    pub amount: f64,
    pub comment: Option<String>,
    pub date: NaiveDate,
}

/// What a person filled in for a transfer.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferDraft {
    pub from: Vec<u8>,
    pub to: Vec<u8>,
    /// What left `from`, in its currency.
    pub amount_from: f64,
    /// What arrived in `to`, in its currency.
    pub amount_to: f64,
    pub comment: Option<String>,
    pub date: NaiveDate,
}

impl Entry {
    /// The first eight hex digits of the id, which is what the terminal
    /// version showed and is enough to tell two records apart by eye.
    pub fn short_id(&self) -> String {
        self.id.iter().take(4).map(|b| format!("{b:02x}")).collect()
    }
}

/// The date a new record starts on: that of the latest record a person
/// entered, or nothing if there is none. Records are usually entered a few
/// at a time for the same day, often not today's. Transfer legs do not count:
/// they are not entered here.
pub fn latest_date(entries: &[Entry]) -> Option<NaiveDate> {
    entries
        .iter()
        .filter(|entry| !entry.is_transfer)
        .map(|entry| entry.date)
        .max()
}

impl From<&Expense> for Entry {
    fn from(expense: &Expense) -> Self {
        Entry {
            id: expense.id.clone(),
            category_id: expense.category_id.clone(),
            amount: expense.amount,
            comment: expense.comment.clone(),
            date: expense.date,
            is_transfer: expense.transfer.is_some(),
        }
    }
}

impl From<&TopUp> for Entry {
    fn from(top_up: &TopUp) -> Self {
        Entry {
            id: top_up.id.clone(),
            category_id: top_up.category_id.clone(),
            amount: top_up.amount,
            comment: top_up.comment.clone(),
            date: top_up.date,
            is_transfer: top_up.transfer.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mode_survives_its_tab_index() {
        for mode in Mode::ALL {
            assert_eq!(Mode::from_index(mode.index()), mode);
        }
        // A tab that is not there is the first one.
        assert_eq!(Mode::from_index(9), Mode::Expense);
    }

    #[test]
    fn a_new_record_starts_on_the_day_of_the_latest_one_entered() {
        let on = |day: u32, is_transfer: bool| Entry {
            id: vec![day as u8],
            category_id: Vec::new(),
            amount: 1.0,
            comment: None,
            date: NaiveDate::from_ymd_opt(2026, 9, day).unwrap(),
            is_transfer,
        };
        let entries = [on(3, false), on(12, false), on(7, false), on(20, true)];
        // The latest by date, whatever the order; the transfer leg after it
        // is not something a person entered as a record.
        assert_eq!(latest_date(&entries), NaiveDate::from_ymd_opt(2026, 9, 12));
        assert_eq!(latest_date(&[on(20, true)]), None);
        assert_eq!(latest_date(&[]), None);
    }

    #[test]
    fn a_short_id_is_the_first_four_bytes() {
        let entry = Entry {
            id: vec![0x9f, 0x3a, 0x1c, 0x07, 0xaa, 0xbb],
            category_id: Vec::new(),
            amount: 1.0,
            comment: None,
            date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            is_transfer: false,
        };
        assert_eq!(entry.short_id(), "9f3a1c07");
    }
}
