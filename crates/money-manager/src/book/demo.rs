//! Sample records, for looking at the interface without a ledger of one's own.
//!
//! `MONEY_MANAGER_DEMO=1` skips the first-run question and fills an empty
//! ledger with these. The UI scenarios in `tests/ui` run against them, so the
//! counts here are load-bearing: change them and the scenarios that state
//! them change too.

use chrono::{Datelike, Days, Months, NaiveDate};
use money_core::models::{
    Account, Category, CategoryStyle, Expense, TopUp, TopUpCategory, Transfer,
};
use money_core::store::{DEFAULT_ACCOUNT_ID, Store};

/// How the sample categories are drawn, by name: (name, colour, icon key).
const LOOKS: [(&str, &str, &str); 9] = [
    ("Food & dining", "#e8853b", "utensils"),
    ("Transportation", "#4c8df6", "car"),
    ("Entertainment", "#9a6cf0", "film"),
    ("Utilities", "#d9a514", "zap"),
    ("Healthcare", "#2fb39a", "heart"),
    ("Shopping", "#e0569a", "shopping-bag"),
    ("Salary", "#4c8df6", "briefcase"),
    ("Freelance", "#2fb39a", "laptop"),
    ("Gifts", "#e0569a", "gift"),
];

/// The first of the month `back` months before `today`'s.
fn month_start(today: NaiveDate, back: u32) -> NaiveDate {
    today
        .with_day(1)
        .and_then(|first| first.checked_sub_months(Months::new(back)))
        .unwrap_or(today)
}

/// The look of the sample category called `name`.
fn look(name: &str) -> CategoryStyle {
    LOOKS
        .iter()
        .find(|(category, _, _)| *category == name)
        .map(|(_, color, icon)| CategoryStyle {
            color: Some(color.to_string()),
            icon: Some(icon.to_string()),
        })
        .unwrap_or_default()
}

pub fn seed(store: &mut Store, today: NaiveDate) -> Result<(), String> {
    Account::ensure_default(store, "RUB")?;
    let savings = Account::create(store, "Savings", "USD", 1500.0)?;

    let food = Category::create_styled(store, "Food & dining", &look("Food & dining"))?;
    let transport = Category::create_styled(store, "Transportation", &look("Transportation"))?;
    let fun = Category::create_styled(store, "Entertainment", &look("Entertainment"))?;
    let utilities = Category::create_styled(store, "Utilities", &look("Utilities"))?;
    let health = Category::create_styled(store, "Healthcare", &look("Healthcare"))?;
    let shopping = Category::create_styled(store, "Shopping", &look("Shopping"))?;

    let salary = TopUpCategory::create_styled(store, "Salary", &look("Salary"))?;
    let freelance = TopUpCategory::create_styled(store, "Freelance", &look("Freelance"))?;
    let gifts = TopUpCategory::create_styled(store, "Gifts", &look("Gifts"))?;

    // Six months of the same shape, so every chart has something to say.
    // (day of month, category, amount, comment)
    let month: [(u64, &Category, f64, &str); 8] = [
        (1, &utilities, 5200.0, "Electricity bill"),
        (2, &food, 1240.0, "Grocery shopping"),
        (4, &transport, 450.0, "Taxi"),
        (7, &food, 385.5, "Coffee shop"),
        (11, &fun, 890.0, "Movie tickets"),
        (15, &shopping, 4990.0, "New shoes"),
        (19, &health, 3200.0, "Doctor visit"),
        (23, &food, 2150.0, "Dinner with friends"),
    ];
    for back in 0..6u32 {
        let first = month_start(today, back);
        // A little more is spent every month, so the bars are not a wall.
        let growth = 1.0 - f64::from(back) * 0.06;
        for (day, category, amount, comment) in month {
            let Some(date) = first.checked_add_days(Days::new(day - 1)) else {
                continue;
            };
            // Nothing is dated in the future.
            if date > today {
                continue;
            }
            Expense::create(
                store,
                &category.id,
                (amount * growth * 100.0).round() / 100.0,
                Some(comment),
                date,
                &DEFAULT_ACCOUNT_ID,
            )?;
        }
        TopUp::create(
            store,
            &salary.id,
            185_000.0,
            Some("Monthly salary"),
            first,
            &DEFAULT_ACCOUNT_ID,
        )?;
    }
    let last_month = month_start(today, 1);
    TopUp::create(
        store,
        &freelance.id,
        42_000.0,
        Some("Website project"),
        last_month
            .checked_add_days(Days::new(19))
            .unwrap_or(last_month),
        &DEFAULT_ACCOUNT_ID,
    )?;
    TopUp::create(
        store,
        &gifts.id,
        5_000.0,
        Some("Birthday gift"),
        last_month
            .checked_add_days(Days::new(27))
            .unwrap_or(last_month),
        &DEFAULT_ACCOUNT_ID,
    )?;
    // One transfer, so both accounts show a leg.
    Transfer::create(
        store,
        &DEFAULT_ACCOUNT_ID,
        &savings.id,
        20_000.0,
        216.5,
        Some("To savings"),
        month_start(today, 0),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_ledger_has_both_accounts_and_nothing_in_the_future() {
        let dir = std::env::temp_dir().join(format!("money-manager-demo-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut store = Store::open(&dir, 1).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
        seed(&mut store, today).unwrap();

        assert_eq!(Account::read_all(&store).unwrap().len(), 2);
        let categories = Category::read_all(&store).unwrap();
        assert_eq!(categories.len(), 6);
        // Every sample category has a look, so none was misspelt in `LOOKS`.
        assert!(categories.iter().all(|c| !c.style.is_empty()));
        assert!(
            TopUpCategory::read_all(&store)
                .unwrap()
                .iter()
                .all(|c| !c.style.is_empty())
        );
        assert_eq!(TopUpCategory::read_all(&store).unwrap().len(), 3);
        assert_eq!(Transfer::read_all(&store).unwrap().len(), 1);
        let expenses = Expense::read_all(&store).unwrap();
        assert!(expenses.iter().all(|e| e.date <= today));
        // Five full months of eight, and the three of this month's that fall
        // on or before the 5th.
        assert_eq!(expenses.len(), 5 * 8 + 3);
        assert_eq!(TopUp::read_all(&store).unwrap().len(), 6 + 2);
        std::fs::remove_dir_all(&dir).ok();
    }
}
