//! CSV import that mirrors the original SQL tables, identifiers included.
//!
//! One CSV file per table, with a header row whose column names match the old
//! SQLite/diesel schema:
//!
//! | Table                  | Columns                                  |
//! |------------------------|------------------------------------------|
//! | `categories`           | `id,name`                                |
//! | `top-up-categories`    | `id,name`                                |
//! | `expenses`             | `id,category_id,amount,comment,date`     |
//! | `top-ups`              | `id,category_id,amount,comment,date`     |
//!
//! `id` and `category_id` carry UUIDs (canonical hyphenated or 32-char hex), so
//! identity is preserved exactly as in the SQL version — re-importing the same
//! file is idempotent, and an expense's `category_id` keeps pointing at the same
//! category. A blank `id` cell is filled with a fresh UUID; a blank `comment`
//! becomes NULL/None. Columns are matched by header name, so their order is free.

use std::path::Path;

use chrono::NaiveDate;
use clap::ValueEnum;
use uuid::Uuid;

use crate::models::{Category, Expense, TopUp, TopUpCategory};
use crate::store::Store;

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
    let (name_i, cat_i, amt_i, com_i, date_i) = match table {
        Table::Categories | Table::TopUpCategories => (Some(need("name")?), None, None, None, None),
        Table::Expenses | Table::TopUps => (
            None,
            Some(need("category_id")?),
            Some(need("amount")?),
            col("comment"),
            Some(need("date")?),
        ),
    };

    let mut count = 0usize;
    for (row, rec) in reader.records().enumerate() {
        let loc = format!("{}:{}", path.display(), row + 2); // +2: 1-based, past header
        let rec = rec.map_err(|e| format!("{loc}: {e}"))?;

        match table {
            Table::Categories | Table::TopUpCategories => {
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
            }
            Table::Expenses | Table::TopUps => {
                let id = parse_id(get(&rec, id_i)).map_err(|e| format!("{loc}: {e}"))?;
                let category_id =
                    parse_uuid(get(&rec, cat_i)).map_err(|e| format!("{loc}: {e}"))?;
                let amount = get(&rec, amt_i)
                    .parse::<f64>()
                    .map_err(|_| format!("{loc}: invalid amount {:?}", get(&rec, amt_i)))?;
                let comment_s = get(&rec, com_i);
                let comment = (!comment_s.is_empty()).then(|| comment_s.to_string());
                let date = NaiveDate::parse_from_str(get(&rec, date_i), DATE_FMT)
                    .map_err(|_| format!("{loc}: invalid date (want YYYY-MM-DD)"))?;
                match table {
                    Table::Expenses => Expense::create_with_id(
                        store,
                        &id,
                        &category_id,
                        amount,
                        comment.as_deref(),
                        date,
                    ),
                    _ => TopUp::create_with_id(
                        store,
                        &id,
                        &category_id,
                        amount,
                        comment.as_deref(),
                        date,
                    ),
                }
                .map_err(|e| format!("{loc}: {e}"))?;
            }
        }
        count += 1;
    }
    Ok(count)
}

/// Trimmed cell at `idx`, or "" if the column is absent / the row is short.
fn get<'a>(rec: &'a csv::StringRecord, idx: Option<usize>) -> &'a str {
    idx.and_then(|i| rec.get(i)).unwrap_or("").trim()
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
    fn missing_required_column_errors_before_writing() {
        let f = tmp("nohdr", "name\nFood\n");
        let mut s = store("nohdr");
        let err = import_csv(&mut s, Table::Expenses, &f).unwrap_err();
        assert!(err.contains("category_id"), "got: {err}");
        std::fs::remove_file(&f).ok();
    }
}
