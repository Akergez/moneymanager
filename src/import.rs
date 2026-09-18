//! CSV import that mirrors the original SQL tables, identifiers included.
//!
//! One CSV file per table, with a header row whose column names match the old
//! SQLite/diesel schema:
//!
//! | Table                  | Columns                                  |
//! |------------------------|------------------------------------------|
//! | `categories`           | `id,name`                                |
//! | `top-up-categories`    | `id,name`                                |
//! | `expenses`             | `id,category_id,amount,comment,date[,account_id]` |
//! | `top-ups`              | `id,category_id,amount,comment,date[,account_id]` |
//! | `accounts`             | `id,name,currency,opening_balance`       |
//! | `transfers`            | `id,from_account_id,to_account_id,amount_from,amount_to,comment,date` |
//!
//! `id` and the `*_id` references carry UUIDs (canonical hyphenated or 32-char
//! hex), so identity is preserved exactly as in the SQL version — re-importing
//! the same file is idempotent, and an expense's `category_id` keeps pointing at
//! the same category. A blank `id` cell is filled with a fresh UUID; a blank
//! `comment` becomes NULL/None. Columns are matched by header name, so their
//! order is free.
//!
//! `account_id` is optional for expenses/top-ups: a missing column or blank cell
//! assigns the record to the default account (the nil UUID
//! `00000000-0000-0000-0000-000000000000`). For accounts, `currency` is a
//! 3-letter ISO code and a blank `opening_balance` means 0.

use std::path::Path;

use chrono::NaiveDate;
use clap::ValueEnum;
use uuid::Uuid;

use crate::models::{Account, Category, Expense, TopUp, TopUpCategory, Transfer};
use crate::store::{DEFAULT_ACCOUNT_ID, Store};

const DATE_FMT: &str = "%Y-%m-%d";

/// Which table a CSV file maps to.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Table {
    Categories,
    Expenses,
    #[value(name = "top-up-categories")]
    TopUpCategories,
    #[value(name = "top-ups")]
    TopUps,
    Accounts,
    Transfers,
}

/// Import every row of `path` into `table`, returning the number of rows
/// imported. Stops at the first malformed row with a line-tagged error.
pub fn import_csv(store: &mut Store, table: Table, path: &Path) -> Result<usize, String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .trim(csv::Trim::All)
        .flexible(true)
        .from_path(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?;

    let headers = reader
        .headers()
        .map_err(|e| format!("read header: {e}"))?
        .clone();
    let col = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));
    let need = |name: &str| col(name).ok_or_else(|| format!("missing '{name}' column"));

    // Resolve column positions up front so a bad header fails before any write.
    let id_i = col("id");
    let mut count = 0usize;
    match table {
        Table::Categories | Table::TopUpCategories => {
            let name_i = Some(need("name")?);
            for (row, rec) in reader.records().enumerate() {
                let loc = format!("{}:{}", path.display(), row + 2); // +2: 1-based, past header
                let rec = rec.map_err(|e| format!("{loc}: {e}"))?;
                let id = parse_id(get(&rec, id_i)).map_err(|e| format!("{loc}: {e}"))?;
                let name = get(&rec, name_i);
                if name.is_empty() {
                    return Err(format!("{loc}: empty name"));
                }
                match table {
                    Table::Categories => Category::create_with_id(store, &id, name),
                    _ => TopUpCategory::create_with_id(store, &id, name),
                }
                .map_err(|e| format!("{loc}: {e}"))?;
                count += 1;
            }
        }
        Table::Expenses | Table::TopUps => {
            let cat_i = Some(need("category_id")?);
            let amt_i = Some(need("amount")?);
            let com_i = col("comment");
            let date_i = Some(need("date")?);
            let acc_i = col("account_id");
            for (row, rec) in reader.records().enumerate() {
                let loc = format!("{}:{}", path.display(), row + 2);
                let rec = rec.map_err(|e| format!("{loc}: {e}"))?;
                let id = parse_id(get(&rec, id_i)).map_err(|e| format!("{loc}: {e}"))?;
                let category_id =
                    parse_uuid(get(&rec, cat_i)).map_err(|e| format!("{loc}: {e}"))?;
                let amount = parse_amount(get(&rec, amt_i)).map_err(|e| format!("{loc}: {e}"))?;
                let comment = parse_comment(get(&rec, com_i));
                let date = parse_date(get(&rec, date_i)).map_err(|e| format!("{loc}: {e}"))?;
                let account_id = match get(&rec, acc_i) {
                    "" => DEFAULT_ACCOUNT_ID.to_vec(),
                    s => parse_uuid(s).map_err(|e| format!("{loc}: {e}"))?,
                };
                match table {
                    Table::Expenses => Expense::create_with_id(
                        store,
                        &id,
                        &category_id,
                        amount,
                        comment.as_deref(),
                        date,
                        &account_id,
                    ),
                    _ => TopUp::create_with_id(
                        store,
                        &id,
                        &category_id,
                        amount,
                        comment.as_deref(),
                        date,
                        &account_id,
                    ),
                }
                .map_err(|e| format!("{loc}: {e}"))?;
                count += 1;
            }
        }
        Table::Accounts => {
            let name_i = Some(need("name")?);
            let cur_i = Some(need("currency")?);
            let bal_i = col("opening_balance");
            for (row, rec) in reader.records().enumerate() {
                let loc = format!("{}:{}", path.display(), row + 2);
                let rec = rec.map_err(|e| format!("{loc}: {e}"))?;
                let id = parse_id(get(&rec, id_i)).map_err(|e| format!("{loc}: {e}"))?;
                let name = get(&rec, name_i);
                if name.is_empty() {
                    return Err(format!("{loc}: empty name"));
                }
                let currency = get(&rec, cur_i).to_ascii_uppercase();
                if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
                    return Err(format!("{loc}: invalid currency {currency:?} (want e.g. RUB)"));
                }
                let opening_balance = match get(&rec, bal_i) {
                    "" => 0.0,
                    s => parse_amount(s).map_err(|e| format!("{loc}: {e}"))?,
                };
                Account::create_with_id(store, &id, name, &currency, opening_balance)
                    .map_err(|e| format!("{loc}: {e}"))?;
                count += 1;
            }
        }
        Table::Transfers => {
            let from_i = Some(need("from_account_id")?);
            let to_i = Some(need("to_account_id")?);
            let af_i = Some(need("amount_from")?);
            let at_i = Some(need("amount_to")?);
            let com_i = col("comment");
            let date_i = Some(need("date")?);
            for (row, rec) in reader.records().enumerate() {
                let loc = format!("{}:{}", path.display(), row + 2);
                let rec = rec.map_err(|e| format!("{loc}: {e}"))?;
                let id = parse_id(get(&rec, id_i)).map_err(|e| format!("{loc}: {e}"))?;
                let from = parse_uuid(get(&rec, from_i)).map_err(|e| format!("{loc}: {e}"))?;
                let to = parse_uuid(get(&rec, to_i)).map_err(|e| format!("{loc}: {e}"))?;
                let amount_from = parse_amount(get(&rec, af_i)).map_err(|e| format!("{loc}: {e}"))?;
                let amount_to = parse_amount(get(&rec, at_i)).map_err(|e| format!("{loc}: {e}"))?;
                Transfer::validate(&from, &to, amount_from, amount_to)
                    .map_err(|e| format!("{loc}: {e}"))?;
                let comment = parse_comment(get(&rec, com_i));
                let date = parse_date(get(&rec, date_i)).map_err(|e| format!("{loc}: {e}"))?;
                Transfer::create_with_id(
                    store,
                    &id,
                    &from,
                    &to,
                    amount_from,
                    amount_to,
                    comment.as_deref(),
                    date,
                )
                .map_err(|e| format!("{loc}: {e}"))?;
                count += 1;
            }
        }
    }
    Ok(count)
}

/// Trimmed cell at `idx`, or "" if the column is absent / the row is short.
fn get<'a>(rec: &'a csv::StringRecord, idx: Option<usize>) -> &'a str {
    idx.and_then(|i| rec.get(i)).unwrap_or("").trim()
}

fn parse_amount(s: &str) -> Result<f64, String> {
    s.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("invalid amount {s:?}"))
}

fn parse_date(s: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(s, DATE_FMT).map_err(|_| "invalid date (want YYYY-MM-DD)".to_string())
}

/// Blank comment → `None`.
fn parse_comment(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_string())
}

/// Parse a record id: blank → a fresh UUID, otherwise a supplied UUID's bytes.
fn parse_id(s: &str) -> Result<Vec<u8>, String> {
    if s.is_empty() {
        return Ok(Uuid::new_v4().as_bytes().to_vec());
    }
    parse_uuid(s)
}

/// Parse a required UUID (canonical or 32-hex) into its 16 raw bytes.
fn parse_uuid(s: &str) -> Result<Vec<u8>, String> {
    if s.is_empty() {
        return Err("missing UUID".to_string());
    }
    Uuid::parse_str(s)
        .map(|u| u.as_bytes().to_vec())
        .map_err(|_| format!("invalid UUID {s:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp(tag: &str, body: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let mut p = std::env::temp_dir();
        p.push(format!("mm_csv_{tag}_{nanos}.csv"));
        std::fs::File::create(&p).unwrap().write_all(body.as_bytes()).unwrap();
        p
    }

    fn store(tag: &str) -> Store {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let mut p = std::env::temp_dir();
        p.push(format!("mm_csv_store_{tag}_{nanos}.rdx"));
        Store::open(&p, 7).unwrap()
    }

    #[test]
    fn preserves_ids_and_foreign_keys() {
        let cat = "11111111-1111-4111-8111-111111111111";
        let cat2 = "22222222-2222-4222-8222-222222222222";
        let cats = tmp("cats", &format!("id,name\n{cat},Food\n{cat2},Transport\n"));
        // category_id references the explicit category id; comment has a comma.
        let exps = tmp(
            "exps",
            &format!("id,category_id,amount,comment,date\n,{cat},45.5,\"Lunch, tax\",2024-12-01\n,{cat},3.0,,2024-12-03\n"),
        );

        let mut s = store("fk");
        assert_eq!(import_csv(&mut s, Table::Categories, &cats).unwrap(), 2);
        assert_eq!(import_csv(&mut s, Table::Expenses, &exps).unwrap(), 2);

        let categories = Category::read_all(&s).unwrap();
        assert_eq!(categories.len(), 2);
        let want_bytes = Uuid::parse_str(cat).unwrap().as_bytes().to_vec();
        let food = categories.iter().find(|c| c.name == "Food").unwrap();
        assert_eq!(food.id, want_bytes, "explicit id must be preserved verbatim");

        let mut expenses = Expense::read_all(&s).unwrap();
        expenses.sort_by(|a, b| a.amount.partial_cmp(&b.amount).unwrap());
        assert_eq!(expenses.len(), 2);
        // Foreign key still points at the imported category.
        assert!(expenses.iter().all(|e| e.category_id == want_bytes));
        assert_eq!(expenses[0].comment, None);
        assert_eq!(expenses[1].comment, Some("Lunch, tax".to_string()));

        // Re-import is idempotent: same explicit ids overwrite in place.
        assert_eq!(import_csv(&mut s, Table::Categories, &cats).unwrap(), 2);
        assert_eq!(Category::read_all(&s).unwrap().len(), 2);

        std::fs::remove_file(&cats).ok();
        std::fs::remove_file(&exps).ok();
    }

    #[test]
    fn imports_accounts_transfers_and_expenses_without_account_column() {
        let card = "33333333-3333-4333-8333-333333333333";
        let nil = "00000000-0000-0000-0000-000000000000";
        let cat = "11111111-1111-4111-8111-111111111111";
        let accounts = tmp(
            "accs",
            &format!("id,name,currency,opening_balance\n{nil},Cash,rub,10\n{card},Card USD,USD,\n"),
        );
        let transfers = tmp(
            "trs",
            &format!(
                "id,from_account_id,to_account_id,amount_from,amount_to,comment,date\n\
                 ,{card},{nil},100,9235,fx,2026-09-12\n"
            ),
        );
        // No account_id column → the default account; explicit column → that one.
        let exps = tmp("exps_noacc", &format!("id,category_id,amount,date\n,{cat},5,2026-09-01\n"));
        let tops = tmp(
            "tops_acc",
            &format!("id,category_id,amount,date,account_id\n,{cat},7,2026-09-01,{card}\n"),
        );

        let mut s = store("accs");
        assert_eq!(import_csv(&mut s, Table::Accounts, &accounts).unwrap(), 2);
        assert_eq!(import_csv(&mut s, Table::Transfers, &transfers).unwrap(), 1);
        assert_eq!(import_csv(&mut s, Table::Expenses, &exps).unwrap(), 1);
        assert_eq!(import_csv(&mut s, Table::TopUps, &tops).unwrap(), 1);

        let card_id = Uuid::parse_str(card).unwrap().as_bytes().to_vec();
        let accs = Account::read_all(&s).unwrap();
        assert_eq!(accs.len(), 2);
        let cash = accs.iter().find(|a| a.id == DEFAULT_ACCOUNT_ID.to_vec()).unwrap();
        assert_eq!((cash.name.as_str(), cash.currency.as_str(), cash.opening_balance), ("Cash", "RUB", 10.0));
        let usd = accs.iter().find(|a| a.id == card_id).unwrap();
        assert_eq!(usd.opening_balance, 0.0);

        let trs = Transfer::read_all(&s).unwrap();
        assert_eq!(trs.len(), 1);
        assert_eq!(trs[0].from_account_id, card_id);
        assert_eq!(trs[0].amount_to, 9235.0);
        assert_eq!(trs[0].comment.as_deref(), Some("fx"));

        assert_eq!(Expense::read_all(&s).unwrap()[0].account_id, DEFAULT_ACCOUNT_ID.to_vec());
        assert_eq!(TopUp::read_all(&s).unwrap()[0].account_id, card_id);

        for f in [accounts, transfers, exps, tops] {
            std::fs::remove_file(&f).ok();
        }
    }

    #[test]
    fn rejects_invalid_transfer_rows() {
        let a = "33333333-3333-4333-8333-333333333333";
        let f = tmp(
            "trs_bad",
            &format!("from_account_id,to_account_id,amount_from,amount_to,date\n{a},{a},1,1,2026-01-01\n"),
        );
        let mut s = store("trs_bad");
        let err = import_csv(&mut s, Table::Transfers, &f).unwrap_err();
        assert!(err.contains("different accounts"), "got: {err}");
        assert!(Transfer::read_all(&s).unwrap().is_empty());
        std::fs::remove_file(&f).ok();
    }

    #[test]
    fn missing_required_column_errors_before_writing() {
        let f = tmp("nohdr", "name\nFood\n");
        let mut s = store("nohdr");
        let err = import_csv(&mut s, Table::Expenses, &f).unwrap_err();
        assert!(err.contains("category_id"), "got: {err}");
        std::fs::remove_file(&f).ok();
    }
}
