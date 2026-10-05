//! What the charts draw, worked out from the records.
//!
//! Plain functions over plain data, so the arithmetic can be tested without a
//! window: totals by category, totals by month, and the running total through
//! one month.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, NaiveDate};

use crate::book::Entry;

/// A calendar month.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Month {
    pub year: i32,
    pub month: u32,
}

impl Month {
    pub fn of(date: NaiveDate) -> Self {
        Month {
            year: date.year(),
            month: date.month(),
        }
    }

    pub fn contains(self, date: NaiveDate) -> bool {
        Month::of(date) == self
    }

    pub fn previous(self) -> Self {
        if self.month == 1 {
            Month {
                year: self.year - 1,
                month: 12,
            }
        } else {
            Month {
                year: self.year,
                month: self.month - 1,
            }
        }
    }

    pub fn next(self) -> Self {
        if self.month == 12 {
            Month {
                year: self.year + 1,
                month: 1,
            }
        } else {
            Month {
                year: self.year,
                month: self.month + 1,
            }
        }
    }

    /// How many days it has.
    pub fn days(self) -> u32 {
        let first_of_next = {
            let next = self.next();
            NaiveDate::from_ymd_opt(next.year, next.month, 1)
        };
        first_of_next
            .and_then(|first| first.pred_opt())
            .map_or(30, |last| last.day())
    }

    fn name(self) -> &'static str {
        const NAMES: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        NAMES[(self.month.clamp(1, 12) - 1) as usize]
    }

    /// `September 2026`, as a heading.
    pub fn title(self) -> String {
        format!("{} {}", self.name(), self.year)
    }

    /// `Sep`, as an axis label.
    pub fn short(self) -> String {
        self.name()[..3].to_string()
    }
}

/// One category's share of a total.
#[derive(Debug, Clone, PartialEq)]
pub struct Slice {
    pub category_id: Vec<u8>,
    pub amount: f64,
    /// Its part of the whole, 0–1.
    pub share: f64,
}

/// Totals by category, largest first, over one month or over everything.
pub fn by_category(entries: &[Entry], month: Option<Month>) -> Vec<Slice> {
    let mut totals: HashMap<&[u8], f64> = HashMap::new();
    for entry in entries {
        if month.is_none_or(|month| month.contains(entry.date)) {
            *totals.entry(entry.category_id.as_slice()).or_default() += entry.amount;
        }
    }
    let whole: f64 = totals.values().sum();
    let mut slices: Vec<Slice> = totals
        .into_iter()
        .filter(|(_, amount)| *amount > 0.0)
        .map(|(category_id, amount)| Slice {
            category_id: category_id.to_vec(),
            amount,
            share: if whole > 0.0 { amount / whole } else { 0.0 },
        })
        .collect();
    // Ties are broken by id so the order does not flicker between frames.
    slices.sort_by(|a, b| {
        b.amount
            .partial_cmp(&a.amount)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.category_id.cmp(&b.category_id))
    });
    slices
}

/// One month's total.
#[derive(Debug, Clone, PartialEq)]
pub struct MonthTotal {
    pub month: Month,
    pub amount: f64,
}

/// The totals of the `count` months ending with `last`, oldest first, leaving
/// out the categories in `hidden`. A month with no records is there with a
/// total of zero: a gap in the bars would read as two neighbouring months.
pub fn by_month(
    entries: &[Entry],
    last: Month,
    count: usize,
    hidden: &HashSet<Vec<u8>>,
) -> Vec<MonthTotal> {
    let mut months = Vec::with_capacity(count);
    let mut month = last;
    for _ in 0..count {
        months.push(month);
        month = month.previous();
    }
    months.reverse();
    months
        .into_iter()
        .map(|month| MonthTotal {
            month,
            amount: entries
                .iter()
                .filter(|entry| month.contains(entry.date) && !hidden.contains(&entry.category_id))
                .map(|entry| entry.amount)
                .sum(),
        })
        .collect()
}

/// The records dated within the `count` months ending with `last`: what the
/// bars are drawn from, and so whose categories their filter offers.
pub fn within_months(entries: &[Entry], last: Month, count: usize) -> Vec<Entry> {
    let mut first = last;
    for _ in 1..count {
        first = first.previous();
    }
    entries
        .iter()
        .filter(|entry| (first..=last).contains(&Month::of(entry.date)))
        .cloned()
        .collect()
}

/// The running total at the end of one day of a month.
#[derive(Debug, Clone, PartialEq)]
pub struct DayTotal {
    pub day: u32,
    pub total: f64,
}

/// The running total through `month`, a point for every day of it, leaving
/// out the categories in `hidden`. Days after `until` are left off, so the
/// line of the month in progress stops at today instead of running flat to
/// the end of it.
pub fn cumulative(
    entries: &[Entry],
    month: Month,
    hidden: &HashSet<Vec<u8>>,
    until: NaiveDate,
) -> Vec<DayTotal> {
    let mut per_day = vec![0.0f64; month.days() as usize + 1];
    for entry in entries {
        if month.contains(entry.date) && !hidden.contains(&entry.category_id) {
            per_day[entry.date.day() as usize] += entry.amount;
        }
    }
    let last_day = if Month::of(until) == month {
        until.day()
    } else if Month::of(until) < month {
        0
    } else {
        month.days()
    };
    let mut total = 0.0;
    (1..=last_day)
        .map(|day| {
            total += per_day[day as usize];
            DayTotal { day, total }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(category: u8, amount: f64, y: i32, m: u32, d: u32) -> Entry {
        Entry {
            id: vec![category, d as u8],
            category_id: vec![category],
            amount,
            comment: None,
            date: NaiveDate::from_ymd_opt(y, m, d).unwrap(),
            is_transfer: false,
        }
    }

    const SEP: Month = Month {
        year: 2026,
        month: 9,
    };

    #[test]
    fn months_step_across_a_year_and_know_their_length() {
        let january = Month {
            year: 2026,
            month: 1,
        };
        assert_eq!(
            january.previous(),
            Month {
                year: 2025,
                month: 12
            }
        );
        assert_eq!(january.previous().next(), january);
        assert_eq!(SEP.days(), 30);
        assert_eq!(
            Month {
                year: 2024,
                month: 2
            }
            .days(),
            29
        );
        assert_eq!(SEP.title(), "September 2026");
        assert_eq!(SEP.short(), "Sep");
    }

    #[test]
    fn categories_are_totalled_within_the_month_and_ordered_by_size() {
        let entries = [
            entry(1, 30.0, 2026, 9, 2),
            entry(2, 50.0, 2026, 9, 3),
            entry(1, 30.0, 2026, 9, 9),
            entry(3, 999.0, 2026, 8, 9),
        ];
        let slices = by_category(&entries, Some(SEP));
        assert_eq!(slices.len(), 2);
        assert_eq!(
            (slices[0].category_id.clone(), slices[0].amount),
            (vec![1], 60.0)
        );
        assert_eq!(slices[1].amount, 50.0);
        let shares: f64 = slices.iter().map(|s| s.share).sum();
        assert!((shares - 1.0).abs() < 1e-9);

        // Over everything, August's record is in.
        assert_eq!(by_category(&entries, None)[0].amount, 999.0);
        // A month with nothing in it has no slices, not one of zero.
        assert!(by_category(&entries, Some(SEP.next())).is_empty());
    }

    #[test]
    fn every_month_of_the_window_has_a_bar_even_an_empty_one() {
        let entries = [entry(1, 10.0, 2026, 9, 1), entry(1, 5.0, 2026, 7, 1)];
        let totals = by_month(&entries, SEP, 4, &HashSet::new());
        let amounts: Vec<f64> = totals.iter().map(|t| t.amount).collect();
        assert_eq!(amounts, [0.0, 5.0, 0.0, 10.0]);
        assert_eq!(totals[0].month.month, 6);
        assert_eq!(totals[3].month, SEP);
    }

    #[test]
    fn a_hidden_category_is_left_out_of_every_bar() {
        let entries = [
            entry(1, 10.0, 2026, 9, 1),
            entry(2, 4.0, 2026, 9, 2),
            entry(2, 6.0, 2026, 8, 2),
        ];
        let hidden: HashSet<Vec<u8>> = [vec![2]].into();
        let amounts: Vec<f64> = by_month(&entries, SEP, 2, &hidden)
            .iter()
            .map(|t| t.amount)
            .collect();
        // The months stay, emptied: hiding a category removes no bar.
        assert_eq!(amounts, [0.0, 10.0]);
    }

    #[test]
    fn the_bars_filter_offers_only_what_the_window_holds() {
        let entries = [
            entry(1, 10.0, 2026, 9, 30),
            entry(2, 5.0, 2026, 8, 1),
            entry(3, 7.0, 2026, 7, 31),
            entry(4, 1.0, 2026, 10, 1),
        ];
        let inside = within_months(&entries, SEP, 2);
        let categories: Vec<u8> = inside.iter().map(|e| e.category_id[0]).collect();
        // August and September are in; July and October are not.
        assert_eq!(categories, [1, 2]);
        assert_eq!(within_months(&entries, SEP, 1).len(), 1);
    }

    #[test]
    fn the_running_total_only_grows_and_stops_at_today() {
        let entries = [
            entry(1, 10.0, 2026, 9, 2),
            entry(2, 5.0, 2026, 9, 2),
            entry(1, 7.0, 2026, 9, 4),
        ];
        let nothing_hidden = HashSet::new();
        let today = NaiveDate::from_ymd_opt(2026, 9, 5).unwrap();
        let line = cumulative(&entries, SEP, &nothing_hidden, today);
        assert_eq!(line.len(), 5);
        let totals: Vec<f64> = line.iter().map(|p| p.total).collect();
        assert_eq!(totals, [0.0, 15.0, 15.0, 22.0, 22.0]);

        // A hidden category is left out of every point.
        let hidden: HashSet<Vec<u8>> = [vec![2]].into();
        let line = cumulative(&entries, SEP, &hidden, today);
        assert_eq!(line.last().unwrap().total, 17.0);

        // A month that is over has all its days; one not begun has none.
        let later = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        assert_eq!(cumulative(&entries, SEP, &nothing_hidden, later).len(), 30);
        let earlier = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();
        assert!(cumulative(&entries, SEP, &nothing_hidden, earlier).is_empty());
    }
}
