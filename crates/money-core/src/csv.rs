//! Nine-column CSV interchange for all accounts. Transfers are exported once.
//!
//! A tenth column, `теги`, carries the tags of an expense or an income, their
//! names separated by `;`. It is written only when the ledger has a tagged
//! record, so a ledger without tags exports exactly the nine columns an
//! earlier version reads; an import takes either. A tag a file names that
//! the ledger does not have is made.
//!
//! Besides the operations (`расход`, `доход`, `перевод`) a file states each
//! account once, as a `счёт` row with no date: its currency and opening
//! balance in the "в" pair. That is what carries an account's opening
//! balance, and an account with no operations at all, through an export and
//! an import. A file without such rows (written by hand, or by an earlier
//! version) still imports: an account it names is made with a zero balance.
//!
//! An expense or an income whose category was removed (see
//! [`CategoryState::Removed`]) is written with an empty category, and a row
//! with an empty category is imported into such a category: one the ledger
//! already has, or one made for it. A merged category is written as the one
//! it went into, and a removed or merged tag as the interface shows it.
//!
//! Importing a file into a ledger that already holds its operations adds
//! nothing twice: a row is skipped while the ledger has an identical
//! operation (same day, accounts, category, amounts and comment) that no
//! earlier row of the file has already been matched with. Identical rows
//! within the file are all kept.

use std::collections::HashMap;
use std::io::{Read, Write};

use chrono::NaiveDate;
use rdx_rs::RdxValue;
use uuid::Uuid;

use crate::models::{
    Account, Category, CategoryState, Expense, Redirects, TopUp, TopUpCategory, Transfer,
};
use crate::store::{self, Store, hex_encode};
use crate::tags::{Tag, TagKind};

/// The fixed column order used by both import and export.
pub const HEADER: [&str; 9] = [
    "дата",
    "тип",
    "счёт",
    "категория или другой счет",
    "сумма в валюте из",
    "валюта из",
    "сумма в валюте в",
    "валюта в",
    "коммент",
];

type Record = (usize, String, Vec<RdxValue>);

/// The optional tenth column.
pub const TAGS_HEADER: &str = "теги";

/// What separates the names of a row's tags.
pub const TAG_SEPARATOR: char = ';';

/// The type of a row that states an account rather than an operation.
const ACCOUNT_ROW: &str = "счёт";

/// What an import did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImportReport {
    /// Operations written to the ledger.
    pub added: usize,
    /// Rows the ledger already had an identical operation for.
    pub skipped: usize,
}

struct Row {
    /// None only on an account row.
    date: Option<NaiveDate>,
    kind: String,
    account: String,
    category: String,
    amount_from: Option<f64>,
    currency_from: String,
    amount_to: Option<f64>,
    currency_to: String,
    comment: String,
    /// The names in the tenth column, if the file has one.
    tags: Vec<String>,
}

/// The names in a row's tag column: separated by `;`, blanks left out, each
/// once.
fn tag_names(fields: &::csv::StringRecord) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for name in fields.get(9).unwrap_or("").split(TAG_SEPARATOR) {
        let name = name.trim();
        if !name.is_empty() && !names.iter().any(|known| known == name) {
            names.push(name.to_string());
        }
    }
    names
}

fn amount(value: &str) -> Result<Option<f64>, String> {
    if value.trim().is_empty() {
        return Ok(None);
    }
    let amount: f64 = value
        .trim()
        .replace(',', ".")
        .parse()
        .map_err(|_| "Некорректная сумма".to_string())?;
    if !amount.is_finite() || amount <= 0.0 {
        return Err("Сумма должна быть положительным конечным числом".into());
    }
    Ok(Some(amount))
}

fn currency(value: &str) -> Result<String, String> {
    let value = value.trim().to_uppercase();
    if !value.is_empty() && (value.len() != 3 || !value.bytes().all(|c| c.is_ascii_alphabetic())) {
        return Err("Валюта должна быть трёхбуквенным кодом, например RUB".into());
    }
    Ok(value)
}

/// An account's opening balance: any finite number, zero and debts included.
fn balance(value: &str) -> Result<f64, String> {
    let balance: f64 = value
        .trim()
        .replace(',', ".")
        .parse()
        .map_err(|_| "Некорректный начальный остаток".to_string())?;
    if !balance.is_finite() {
        return Err("Начальный остаток должен быть конечным числом".into());
    }
    Ok(balance)
}

fn date(value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d")
        .or_else(|_| NaiveDate::parse_from_str(value.trim(), "%d.%m.%Y"))
        .map_err(|_| "Дата должна быть YYYY-MM-DD или DD.MM.YYYY".to_string())
}

/// An account row: `,счёт,<name>,,,,<opening balance>,<currency>,`.
struct AccountRow {
    name: String,
    currency: String,
    opening_balance: f64,
}

impl AccountRow {
    fn parse(fields: &::csv::StringRecord) -> Result<Self, String> {
        let name = fields[2].trim().to_string();
        let currency = currency(&fields[7])?;
        if name.is_empty() || currency.is_empty() {
            return Err("Строка счёта требует название счёта и валюту в".into());
        }
        if [0, 3, 4, 5, 8, 9]
            .iter()
            .any(|&ix| fields.get(ix).is_some_and(|field| !field.trim().is_empty()))
        {
            return Err(
                "В строке счёта заполняются только счёт, сумма в валюте в и валюта в".into(),
            );
        }
        let opening_balance = if fields[6].trim().is_empty() {
            0.0
        } else {
            balance(&fields[6])?
        };
        Ok(AccountRow {
            name,
            currency,
            opening_balance,
        })
    }
}

impl Row {
    fn parse(fields: &::csv::StringRecord) -> Result<Self, String> {
        let row = Self {
            date: Some(date(&fields[0])?),
            kind: fields[1].trim().to_lowercase(),
            account: fields[2].trim().to_string(),
            category: fields[3].trim().to_string(),
            amount_from: amount(&fields[4])?,
            currency_from: currency(&fields[5])?,
            amount_to: amount(&fields[6])?,
            currency_to: currency(&fields[7])?,
            comment: fields[8].to_string(),
            tags: tag_names(fields),
        };
        // An expense or an income may have none: its category was removed.
        if row.account.is_empty() || (row.category.is_empty() && row.kind == "перевод") {
            return Err("Укажите счёт и категорию или счёт получателя".into());
        }
        let from = row.amount_from.is_some() && !row.currency_from.is_empty();
        let to = row.amount_to.is_some() && !row.currency_to.is_empty();
        match row.kind.as_str() {
            "расход" if from && row.amount_to.is_none() && row.currency_to.is_empty() => {}
            "доход" if to && row.amount_from.is_none() && row.currency_from.is_empty() => {}
            "перевод" if !row.tags.is_empty() => return Err("У перевода не бывает тегов".into()),
            "перевод" if from && to && row.account != row.category => {}
            "расход" | "доход" | "перевод" => return Err("Расход требует только сумму и валюту из, доход — только в, перевод — обе пары и разные счета".into()),
            _ => return Err("Тип должен быть расход, доход или перевод".into()),
        }
        Ok(row)
    }
}

fn new_id() -> Vec<u8> {
    Uuid::new_v4().as_bytes().to_vec()
}

fn account_id(
    accounts: &mut Vec<Account>,
    records: &mut Vec<Record>,
    name: &str,
    currency: &str,
) -> Result<Vec<u8>, String> {
    let matches: Vec<_> = accounts.iter().filter(|a| a.name == name).collect();
    match matches.as_slice() {
        [account] if account.currency.eq_ignore_ascii_case(currency) => Ok(account.id.clone()),
        [_] => Err(format!("Валюта счёта «{name}» не совпадает с {currency}")),
        [] => {
            let id = new_id();
            records.push((
                store::ACCOUNTS_IDX,
                hex_encode(&id),
                vec![
                    RdxValue::Str(name.into()),
                    RdxValue::Str(currency.into()),
                    RdxValue::Float(0.0),
                ],
            ));
            accounts.push(Account {
                id: id.clone(),
                name: name.into(),
                currency: currency.into(),
                opening_balance: 0.0,
            });
            Ok(id)
        }
        _ => Err(format!("Несколько счетов с названием «{name}»")),
    }
}

fn category_id(
    categories: &mut HashMap<String, Vec<Vec<u8>>>,
    records: &mut Vec<Record>,
    name: &str,
    collection: usize,
) -> Result<Vec<u8>, String> {
    if let Some(ids) = categories.get(name) {
        return match ids.as_slice() {
            [id] => Ok(id.clone()),
            _ => Err(format!("Несколько категорий с названием «{name}»")),
        };
    }
    let id = new_id();
    // A row without a category goes into a removed one, as it came out.
    let state = match name.is_empty() {
        true => CategoryState::Removed,
        false => CategoryState::Visible,
    };
    records.push((
        collection,
        hex_encode(&id),
        store::category_fields(name, &Default::default(), &state),
    ));
    categories.insert(name.into(), vec![id.clone()]);
    Ok(id)
}

/// The parts of an operation that say whether two are the same one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Signature {
    collection: usize,
    date: String,
    account: Vec<u8>,
    other: Vec<u8>,
    amount_from: u64,
    amount_to: u64,
    comment: Option<String>,
}

/// What a signature says of a record's category: the one it is shown in, or
/// nothing for one shown without a category, as a row without one says.
fn shown_category(redirects: &Redirects, id: &[u8]) -> Vec<u8> {
    match redirects.is_live(id) {
        true => redirects.resolve(id),
        false => Vec::new(),
    }
}

/// How each category of a kind is shown.
fn expense_redirects(store: &Store) -> Result<Redirects, String> {
    Ok(Redirects::new(
        Category::read_all(store)?
            .into_iter()
            .map(|c| (c.id, c.state)),
    ))
}

fn income_redirects(store: &Store) -> Result<Redirects, String> {
    Ok(Redirects::new(
        TopUpCategory::read_all(store)?
            .into_iter()
            .map(|c| (c.id, c.state)),
    ))
}

/// How many of each operation the ledger already holds.
fn existing_operations(store: &Store) -> Result<HashMap<Signature, usize>, String> {
    let (expense_categories, income_categories) =
        (expense_redirects(store)?, income_redirects(store)?);
    let mut seen = HashMap::new();
    let mut add = |signature: Signature| *seen.entry(signature).or_insert(0) += 1;
    for e in Expense::read_all(store)? {
        add(Signature {
            collection: store::EXPENSES_IDX,
            date: e.date.format("%Y-%m-%d").to_string(),
            account: e.account_id,
            other: shown_category(&expense_categories, &e.category_id),
            amount_from: e.amount.to_bits(),
            amount_to: 0,
            comment: e.comment,
        });
    }
    for t in TopUp::read_all(store)? {
        add(Signature {
            collection: store::TOP_UPS_IDX,
            date: t.date.format("%Y-%m-%d").to_string(),
            account: t.account_id,
            other: shown_category(&income_categories, &t.category_id),
            amount_from: 0,
            amount_to: t.amount.to_bits(),
            comment: t.comment,
        });
    }
    for t in Transfer::read_all(store)? {
        add(Signature {
            collection: store::TRANSFERS_IDX,
            date: t.date.format("%Y-%m-%d").to_string(),
            account: t.from_account_id,
            other: t.to_account_id,
            amount_from: t.amount_from.to_bits(),
            amount_to: t.amount_to.to_bits(),
            comment: t.comment,
        });
    }
    Ok(seen)
}

/// Adds the file's operations to the ledger, creating missing accounts and
/// categories, and setting the opening balance of each account the file
/// states. The entire file is validated before one atomic native-file edit.
/// See the module documentation for which rows are skipped.
pub fn import(store: &mut Store, reader: impl Read) -> Result<ImportReport, String> {
    let mut text = String::new();
    let mut reader = reader;
    reader
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    let text = text.trim_start_matches('\u{feff}');
    let mut csv = ::csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.as_bytes());

    // Accounts first, wherever their rows are: an operation of an account
    // stated later in the file must find it.
    let mut account_rows = Vec::new();
    let mut operation_rows = Vec::new();
    for (ix, fields) in csv.records().enumerate() {
        let fields = fields.map_err(|e| format!("CSV: {e}"))?;
        let line = fields.position().map_or(ix as u64 + 1, |p| p.line());
        if ix == 0
            && fields
                .get(0)
                .is_some_and(|s| s.trim().eq_ignore_ascii_case("дата"))
        {
            if !matches!(fields.len(), 9 | 10) {
                return Err("Заголовок должен содержать 9 колонок (или 10, с тегами)".into());
            }
            continue;
        }
        let at = |e: String| format!("Строка {line}: {e}");
        if !matches!(fields.len(), 9 | 10) {
            return Err(at("Ожидается 9 колонок (или 10, с тегами)".into()));
        }
        if fields[1].trim().to_lowercase() == ACCOUNT_ROW {
            account_rows.push((line, AccountRow::parse(&fields).map_err(at)?));
        } else {
            operation_rows.push((line, Row::parse(&fields).map_err(at)?));
        }
    }

    let mut accounts = Account::read_all(store)?;
    // By name, the categories the interface has; under no name, the first
    // removed one, for the rows without a category.
    let by_name = |categories: Vec<(Vec<u8>, String, CategoryState)>| {
        let mut names: HashMap<String, Vec<Vec<u8>>> = HashMap::new();
        let mut removed: Vec<Vec<u8>> = Vec::new();
        for (id, name, state) in categories {
            match state {
                CategoryState::Visible | CategoryState::Hidden => {
                    names.entry(name).or_default().push(id)
                }
                CategoryState::Removed => removed.push(id),
                CategoryState::Merged(_) => {}
            }
        }
        if let Some(first) = removed.into_iter().min() {
            names.insert(String::new(), vec![first]);
        }
        names
    };
    let mut expenses = by_name(
        Category::read_all(store)?
            .into_iter()
            .map(|c| (c.id, c.name, c.state))
            .collect(),
    );
    let mut income = by_name(
        TopUpCategory::read_all(store)?
            .into_iter()
            .map(|c| (c.id, c.name, c.state))
            .collect(),
    );
    let mut tags: HashMap<TagKind, HashMap<String, Vec<Vec<u8>>>> = HashMap::new();
    for kind in [TagKind::Expense, TagKind::TopUp] {
        let names = tags.entry(kind).or_default();
        for tag in Tag::read_all(store, kind) {
            if tag.state.is_live() {
                names.entry(tag.name).or_default().push(tag.id);
            }
        }
    }
    let mut existing = existing_operations(store)?;
    let mut records = Vec::new();
    let mut report = ImportReport::default();

    for (line, row) in account_rows {
        let at = |e: String| format!("Строка {line}: {e}");
        let matches: Vec<usize> = (0..accounts.len())
            .filter(|&ix| accounts[ix].name == row.name)
            .collect();
        match matches[..] {
            [] => {
                let id = new_id();
                records.push((
                    store::ACCOUNTS_IDX,
                    hex_encode(&id),
                    vec![
                        RdxValue::Str(row.name.clone()),
                        RdxValue::Str(row.currency.clone()),
                        RdxValue::Float(row.opening_balance),
                    ],
                ));
                accounts.push(Account {
                    id,
                    name: row.name,
                    currency: row.currency,
                    opening_balance: row.opening_balance,
                });
            }
            [ix] => {
                let account = &mut accounts[ix];
                if !account.currency.eq_ignore_ascii_case(&row.currency) {
                    return Err(at(format!(
                        "Валюта счёта «{}» не совпадает с {}",
                        row.name, row.currency
                    )));
                }
                if account.opening_balance.to_bits() != row.opening_balance.to_bits() {
                    account.opening_balance = row.opening_balance;
                    records.push((
                        store::ACCOUNTS_IDX,
                        hex_encode(&account.id),
                        vec![
                            RdxValue::Str(account.name.clone()),
                            RdxValue::Str(account.currency.clone()),
                            RdxValue::Float(row.opening_balance),
                        ],
                    ));
                }
            }
            _ => return Err(at(format!("Несколько счетов с названием «{}»", row.name))),
        }
    }

    for (line, row) in operation_rows {
        let result = (|| {
            let day = row
                .date
                .ok_or("Не указана дата")?
                .format("%Y-%m-%d")
                .to_string();
            let comment_text = (!row.comment.is_empty()).then(|| row.comment.clone());
            let comment = store::opt_str_value(comment_text.as_deref());
            let date = RdxValue::Str(day.clone());
            let mut tag_ids = Vec::new();
            let (collection, values, signature) = match row.kind.as_str() {
                "расход" | "доход" => {
                    let expense = row.kind == "расход";
                    let (value, code) = if expense {
                        (row.amount_from, &row.currency_from)
                    } else {
                        (row.amount_to, &row.currency_to)
                    };
                    let value = value.ok_or("Не указана сумма")?;
                    let account = account_id(&mut accounts, &mut records, &row.account, code)?;
                    let category = category_id(
                        if expense { &mut expenses } else { &mut income },
                        &mut records,
                        &row.category,
                        if expense {
                            store::CATEGORIES_IDX
                        } else {
                            store::TOP_UP_CATEGORIES_IDX
                        },
                    )?;
                    let collection = if expense {
                        store::EXPENSES_IDX
                    } else {
                        store::TOP_UPS_IDX
                    };
                    let kind = if expense {
                        TagKind::Expense
                    } else {
                        TagKind::TopUp
                    };
                    for name in &row.tags {
                        let names = tags.entry(kind).or_default();
                        let id = match names.get(name).map(Vec::as_slice) {
                            Some([id]) => id.clone(),
                            Some(_) => return Err(format!("Несколько тегов с названием «{name}»")),
                            None => {
                                let id = new_id();
                                records.push(Tag::record(kind, &id, name));
                                names.insert(name.clone(), vec![id.clone()]);
                                id
                            }
                        };
                        tag_ids.push((kind, id));
                    }
                    let signature = Signature {
                        collection,
                        date: day,
                        account: account.clone(),
                        other: match row.category.is_empty() {
                            true => Vec::new(),
                            false => category.clone(),
                        },
                        amount_from: if expense { value.to_bits() } else { 0 },
                        amount_to: if expense { 0 } else { value.to_bits() },
                        comment: comment_text,
                    };
                    (
                        collection,
                        vec![
                            RdxValue::Str(hex_encode(&category)),
                            RdxValue::Float(value),
                            comment,
                            date,
                            RdxValue::Str(hex_encode(&account)),
                        ],
                        signature,
                    )
                }
                _ => {
                    let from = account_id(
                        &mut accounts,
                        &mut records,
                        &row.account,
                        &row.currency_from,
                    )?;
                    let to =
                        account_id(&mut accounts, &mut records, &row.category, &row.currency_to)?;
                    let amount_from = row.amount_from.ok_or("Не указана сумма из")?;
                    let amount_to = row.amount_to.ok_or("Не указана сумма в")?;
                    let signature = Signature {
                        collection: store::TRANSFERS_IDX,
                        date: day,
                        account: from.clone(),
                        other: to.clone(),
                        amount_from: amount_from.to_bits(),
                        amount_to: amount_to.to_bits(),
                        comment: comment_text,
                    };
                    (
                        store::TRANSFERS_IDX,
                        vec![
                            RdxValue::Str(hex_encode(&from)),
                            RdxValue::Str(hex_encode(&to)),
                            RdxValue::Float(amount_from),
                            RdxValue::Float(amount_to),
                            comment,
                            date,
                        ],
                        signature,
                    )
                }
            };
            match existing.get_mut(&signature) {
                Some(left) if *left > 0 => {
                    *left -= 1;
                    report.skipped += 1;
                }
                _ => {
                    let id = new_id();
                    records.push((collection, hex_encode(&id), values));
                    for (kind, tag) in &tag_ids {
                        records.extend(Tag::link_records(*kind, &id, std::slice::from_ref(tag)));
                    }
                    report.added += 1;
                }
            }
            Ok::<_, String>(())
        })();
        result.map_err(|e| format!("Строка {line}: {e}"))?;
    }
    store.import_records(records)?;
    Ok(report)
}

/// Export every account and all stored operations, ordered by date, using
/// UTF-8 and comma delimiters. Income uses the destination pair; expense uses
/// the source pair; an account row its opening balance and currency.
pub fn export(store: &Store, writer: impl Write) -> Result<(), String> {
    let accounts = Account::read_all(store)?;
    let accounts: HashMap<_, _> = accounts.iter().map(|a| (a.id.as_slice(), a)).collect();
    // A record's category as the interface shows it: where a merge leads,
    // and no name for one removed.
    let shown_names = |categories: Vec<(Vec<u8>, String, CategoryState)>| {
        let redirects = Redirects::new(
            categories
                .iter()
                .map(|(id, _, state)| (id.clone(), state.clone())),
        );
        let names: HashMap<Vec<u8>, String> = categories
            .into_iter()
            .map(|(id, name, _)| (id, name))
            .collect();
        move |id: &[u8]| -> Option<String> {
            if !names.contains_key(id) {
                return None;
            }
            Some(match redirects.is_live(id) {
                true => names[&redirects.resolve(id)].clone(),
                false => String::new(),
            })
        }
    };
    let expenses = shown_names(
        Category::read_all(store)?
            .into_iter()
            .map(|c| (c.id, c.name, c.state))
            .collect(),
    );
    let income = shown_names(
        TopUpCategory::read_all(store)?
            .into_iter()
            .map(|c| (c.id, c.name, c.state))
            .collect(),
    );
    let account = |id: &[u8]| {
        accounts
            .get(id)
            .copied()
            .ok_or_else(|| format!("Не найден счёт {}", hex_encode(id)))
    };
    let mut rows: Vec<Vec<String>> = Vec::new();
    // No date: they sort before every operation.
    for a in accounts.values() {
        rows.push(vec![
            String::new(),
            ACCOUNT_ROW.into(),
            a.name.clone(),
            String::new(),
            String::new(),
            String::new(),
            a.opening_balance.to_string(),
            a.currency.clone(),
            String::new(),
        ]);
    }
    let names = |kind: TagKind| -> HashMap<Vec<u8>, String> {
        Tag::read_all(store, kind)
            .into_iter()
            .map(|tag| (tag.id, tag.name))
            .collect()
    };
    let tag_cells = |kind: TagKind| -> HashMap<Vec<u8>, String> {
        let names = names(kind);
        Tag::shown_links(store, kind)
            .into_iter()
            .map(|(record, tags)| {
                let mut tags: Vec<&str> = tags
                    .iter()
                    .filter_map(|tag| names.get(tag).map(String::as_str))
                    .collect();
                tags.sort_unstable();
                (record, tags.join(&TAG_SEPARATOR.to_string()))
            })
            .filter(|(_, cell)| !cell.is_empty())
            .collect()
    };
    let (expense_tags, income_tags) = (tag_cells(TagKind::Expense), tag_cells(TagKind::TopUp));
    let with_tags = !expense_tags.is_empty() || !income_tags.is_empty();
    // Where each row's tag cell goes, once the rows are sorted.
    let mut row_tags: HashMap<usize, String> = HashMap::new();
    for entry in Expense::read_all(store)? {
        if let Some(cell) = expense_tags.get(&entry.id) {
            row_tags.insert(rows.len(), cell.clone());
        }
        let a = account(&entry.account_id)?;
        let category = expenses(&entry.category_id).ok_or("Не найдена категория расхода")?;
        rows.push(vec![
            entry.date.to_string(),
            "расход".into(),
            a.name.clone(),
            category,
            entry.amount.to_string(),
            a.currency.clone(),
            String::new(),
            String::new(),
            entry.comment.unwrap_or_default(),
        ]);
    }
    for entry in TopUp::read_all(store)? {
        if let Some(cell) = income_tags.get(&entry.id) {
            row_tags.insert(rows.len(), cell.clone());
        }
        let a = account(&entry.account_id)?;
        let category = income(&entry.category_id).ok_or("Не найдена категория дохода")?;
        rows.push(vec![
            entry.date.to_string(),
            "доход".into(),
            a.name.clone(),
            category,
            String::new(),
            String::new(),
            entry.amount.to_string(),
            a.currency.clone(),
            entry.comment.unwrap_or_default(),
        ]);
    }
    for entry in Transfer::read_all(store)? {
        let from = account(&entry.from_account_id)?;
        let to = account(&entry.to_account_id)?;
        rows.push(vec![
            entry.date.to_string(),
            "перевод".into(),
            from.name.clone(),
            to.name.clone(),
            entry.amount_from.to_string(),
            from.currency.clone(),
            entry.amount_to.to_string(),
            to.currency.clone(),
            entry.comment.unwrap_or_default(),
        ]);
    }
    if with_tags {
        for (ix, row) in rows.iter_mut().enumerate() {
            row.push(row_tags.remove(&ix).unwrap_or_default());
        }
    }
    rows.sort();
    let mut writer = ::csv::Writer::from_writer(writer);
    let mut header: Vec<&str> = HEADER.to_vec();
    if with_tags {
        header.push(TAGS_HEADER);
    }
    writer.write_record(header).map_err(|e| e.to_string())?;
    for row in rows {
        writer.write_record(row).map_err(|e| e.to_string())?;
    }
    writer.flush().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = concat!(
        "дата,тип,счёт,категория или другой счет,сумма в валюте из,валюта из,сумма в валюте в,валюта в,коммент\n",
        "2026-10-01,доход,Кошелёк,Зарплата,,,10000,RUB,\n",
        "2026-10-02,расход,Кошелёк,Еда,125.55,RUB,,,\"Обед, кофе \"\"большой\"\"\nв кафе\"\n",
        "2026-10-03,перевод,Кошелёк,Доллары,900,RUB,10,USD,Обмен\n",
    );

    #[test]
    fn round_trip_preserves_operations_comments_and_balances() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let mut store = Store::open(first.path(), 1).unwrap();
        assert_eq!(import(&mut store, SAMPLE.as_bytes()).unwrap().added, 3);
        let mut csv = Vec::new();
        export(&store, &mut csv).unwrap();
        let mut restored = Store::open(second.path(), 2).unwrap();
        assert_eq!(import(&mut restored, csv.as_slice()).unwrap().added, 3);
        let mut again = Vec::new();
        export(&restored, &mut again).unwrap();
        assert_eq!(csv, again);
        assert_eq!(Transfer::read_all(&restored).unwrap().len(), 1);
        assert_eq!(
            Expense::read_all(&restored).unwrap()[0].comment.as_deref(),
            Some("Обед, кофе \"большой\"\nв кафе")
        );
        let accounts = Account::read_all(&restored).unwrap();
        let summaries = crate::ledger::summaries(
            &accounts,
            &Expense::read_all(&restored).unwrap(),
            &TopUp::read_all(&restored).unwrap(),
            &Transfer::read_all(&restored).unwrap(),
        );
        let wallet = accounts.iter().find(|a| a.name == "Кошелёк").unwrap();
        assert!((summaries[&wallet.id].balance - 8974.45).abs() < 0.000001);
        let reopened = Store::open(second.path(), 2).unwrap();
        assert_eq!(TopUp::read_all(&reopened).unwrap().len(), 1);
    }

    #[test]
    fn invalid_later_row_does_not_write_anything() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path(), 1).unwrap();
        let csv = format!("{SAMPLE}2026-10-04,расход,Кошелёк,Еда,NaN,RUB,,,\n");
        assert!(
            import(&mut store, csv.as_bytes())
                .unwrap_err()
                .contains("Строка 6")
        );
        assert!(store.is_empty().unwrap());
        assert!(Store::open(dir.path(), 1).unwrap().is_empty().unwrap());
    }

    #[test]
    fn accepts_bom_headerless_dates_and_quoted_decimal_comma() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path(), 1).unwrap();
        import(
            &mut store,
            "\u{feff}10.10.2026,расход,Карта,Еда,\"12,34\",rub,,,\r\n".as_bytes(),
        )
        .unwrap();
        let expenses = Expense::read_all(&store).unwrap();
        assert_eq!(expenses[0].amount, 12.34);
        assert_eq!(expenses[0].date.to_string(), "2026-10-10");
    }

    #[test]
    fn rejects_currency_conflicts_without_changing_existing_data() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path(), 1).unwrap();
        Account::create(&mut store, "Карта", "RUB", 100.0).unwrap();
        let before = std::fs::read(dir.path().join(store::LEDGER_FILE)).unwrap();
        assert!(
            import(
                &mut store,
                "2026-10-10,расход,Карта,Еда,12,USD,,,\n".as_bytes()
            )
            .is_err()
        );
        assert_eq!(
            before,
            std::fs::read(dir.path().join(store::LEDGER_FILE)).unwrap()
        );
    }

    #[test]
    fn rejects_bad_dates_types_columns_amounts_and_transfers() {
        for row in [
            "2026-02-30,расход,А,Еда,1,RUB,,,",
            "2026-10-10,покупка,А,Еда,1,RUB,,,",
            "2026-10-10,расход,А,Еда,-1,RUB,,,",
            "2026-10-10,расход,А,Еда,0,RUB,,,",
            "2026-10-10,перевод,А,А,1,RUB,1,RUB,",
            "2026-10-10,перевод,А,Б,1,RUB,,,",
            "2026-10-10,доход,А,Зарплата,1,RUB,,,",
            "2026-10-10,расход,А,Еда,1,RUB,,,коммент,тег,лишнее",
            "2026-10-10,перевод,А,Б,1,RUB,1,RUB,,тег",
            ",счёт,А,,,,10,RUB,,тег",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut store = Store::open(dir.path(), 1).unwrap();
            assert!(import(&mut store, row.as_bytes()).is_err(), "{row}");
            assert!(store.is_empty().unwrap(), "{row}");
        }
    }

    #[test]
    fn reuses_existing_accounts_and_categories_and_rejects_ambiguous_names() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path(), 1).unwrap();
        let account = Account::create(&mut store, "А", "RUB", 100.0).unwrap();
        let category = Category::create(&mut store, "Еда").unwrap();
        let row = "2026-10-10,расход,А,Еда,1,RUB,,,";
        import(&mut store, row.as_bytes()).unwrap();
        let expense = &Expense::read_all(&store).unwrap()[0];
        assert_eq!(expense.account_id, account.id);
        assert_eq!(expense.category_id, category.id);
        Account::create(&mut store, "А", "RUB", 0.0).unwrap();
        assert!(
            import(&mut store, row.as_bytes())
                .unwrap_err()
                .contains("Несколько счетов")
        );
        assert_eq!(Expense::read_all(&store).unwrap().len(), 1);
    }

    #[test]
    fn opening_balances_and_accounts_without_operations_travel() {
        let (first, second) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let mut store = Store::open(first.path(), 1).unwrap();
        Account::ensure_default(&mut store, "RUB").unwrap();
        Account::update(
            &mut store,
            &store::DEFAULT_ACCOUNT_ID,
            "Основной",
            "RUB",
            250.5,
        )
        .unwrap();
        Account::create(&mut store, "Сбережения", "USD", 1500.0).unwrap();
        Account::create(&mut store, "Долг", "EUR", -40.0).unwrap();
        import(&mut store, SAMPLE.as_bytes()).unwrap();
        let mut csv = Vec::new();
        export(&store, &mut csv).unwrap();
        let text = String::from_utf8(csv.clone()).unwrap();
        assert!(text.contains("\n,счёт,Сбережения,,,,1500,USD,\n"), "{text}");

        // A new ledger, as "Start without sync" leaves it.
        let mut restored = Store::open(second.path(), 2).unwrap();
        Account::ensure_default(&mut restored, "RUB").unwrap();
        let report = import(&mut restored, csv.as_slice()).unwrap();
        assert_eq!(
            report,
            ImportReport {
                added: 3,
                skipped: 0
            }
        );
        let balances = |store: &Store| {
            let accounts = Account::read_all(store).unwrap();
            let summaries = crate::ledger::summaries(
                &accounts,
                &Expense::read_all(store).unwrap(),
                &TopUp::read_all(store).unwrap(),
                &Transfer::read_all(store).unwrap(),
            );
            let mut named: Vec<_> = accounts
                .iter()
                .map(|a| {
                    (
                        a.name.clone(),
                        a.currency.clone(),
                        summaries[&a.id].balance.to_bits(),
                    )
                })
                .collect();
            named.sort();
            named
        };
        assert_eq!(balances(&restored), balances(&store));
        // The default account is the one the file's "Основной" lands in.
        assert_eq!(Account::read_all(&restored).unwrap().len(), 5);
        let mut again = Vec::new();
        export(&restored, &mut again).unwrap();
        assert_eq!(again, csv);
    }

    #[test]
    fn importing_a_ledger_into_itself_adds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path(), 1).unwrap();
        let twice = format!(
            "{SAMPLE}2026-10-02,расход,Кошелёк,Еда,10,RUB,,,\n2026-10-02,расход,Кошелёк,Еда,10,RUB,,,\n"
        );
        assert_eq!(import(&mut store, twice.as_bytes()).unwrap().added, 5);
        let mut csv = Vec::new();
        export(&store, &mut csv).unwrap();

        let report = import(&mut store, csv.as_slice()).unwrap();
        assert_eq!(
            report,
            ImportReport {
                added: 0,
                skipped: 5
            }
        );
        let mut after = Vec::new();
        export(&store, &mut after).unwrap();
        assert_eq!(after, csv);

        // One more identical coffee than the ledger has is one more.
        let three = "2026-10-02,расход,Кошелёк,Еда,10,RUB,,,\n".repeat(3);
        let report = import(&mut store, three.as_bytes()).unwrap();
        assert_eq!(
            report,
            ImportReport {
                added: 1,
                skipped: 2
            }
        );
    }

    #[test]
    fn an_account_row_is_checked_like_any_other() {
        for row in [
            ",счёт,,,,,10,RUB,",
            ",счёт,А,,,,10,,",
            ",счёт,А,,,,abc,RUB,",
            "2026-10-10,счёт,А,,,,10,RUB,",
            ",счёт,А,Еда,,,10,RUB,",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let mut store = Store::open(dir.path(), 1).unwrap();
            assert!(import(&mut store, row.as_bytes()).is_err(), "{row}");
            assert!(store.is_empty().unwrap(), "{row}");
        }
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path(), 1).unwrap();
        Account::create(&mut store, "А", "RUB", 0.0).unwrap();
        assert!(import(&mut store, ",счёт,А,,,,10,USD,".as_bytes()).is_err());
        // An operation before the row of its account still finds it.
        import(
            &mut store,
            "2026-10-10,расход,Б,Еда,1,EUR,,,\n,счёт,Б,,,,5,EUR,\n".as_bytes(),
        )
        .unwrap();
        let accounts = Account::read_all(&store).unwrap();
        let b = accounts.iter().find(|a| a.name == "Б").unwrap();
        assert_eq!(b.opening_balance, 5.0);
        assert_eq!(accounts.len(), 2);
    }

    #[test]
    fn a_merged_category_exports_as_its_target_and_a_removed_one_without_a_name() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path(), 7).unwrap();
        import(
            &mut store,
            concat!(
                "2026-10-01,расход,Кошелёк,Такси,10,RUB,,,\n",
                "2026-10-02,расход,Кошелёк,Транспорт,20,RUB,,,\n",
                "2026-10-03,расход,Кошелёк,Кафе,30,RUB,,,\n",
            )
            .as_bytes(),
        )
        .unwrap();
        let id = |name: &str| {
            Category::read_all(&store)
                .unwrap()
                .into_iter()
                .find(|c| c.name == name)
                .unwrap()
                .id
        };
        let (taxi, transport, cafe) = (id("Такси"), id("Транспорт"), id("Кафе"));
        let style = Default::default();
        Category::create_with_id(
            &mut store,
            &taxi,
            "Такси",
            &style,
            &CategoryState::Merged(transport),
        )
        .unwrap();
        Category::create_with_id(&mut store, &cafe, "Кафе", &style, &CategoryState::Removed)
            .unwrap();

        let mut out = Vec::new();
        export(&store, &mut out).unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("2026-10-01,расход,Кошелёк,Транспорт,10,RUB"));
        assert!(out.contains("2026-10-03,расход,Кошелёк,,30,RUB"));
        assert!(!out.contains("Такси") && !out.contains("Кафе"));

        // Into itself: nothing twice, the row without a category included.
        assert_eq!(import(&mut store, out.as_bytes()).unwrap().added, 0);

        // Into another ledger: the row without a category goes into a
        // removed category made for it, and comes out the same way.
        let other = tempfile::tempdir().unwrap();
        let mut copy = Store::open(other.path(), 8).unwrap();
        assert_eq!(import(&mut copy, out.as_bytes()).unwrap().added, 3);
        let removed: Vec<_> = Category::read_all(&copy)
            .unwrap()
            .into_iter()
            .filter(|c| c.state == CategoryState::Removed)
            .collect();
        assert_eq!(removed.len(), 1);
        assert_eq!(import(&mut copy, out.as_bytes()).unwrap().added, 0);
        let mut again = Vec::new();
        export(&copy, &mut again).unwrap();
        assert_eq!(String::from_utf8(again).unwrap(), out);
        // A transfer still needs both accounts.
        assert!(
            import(
                &mut copy,
                "2026-10-04,перевод,Кошелёк,,1,RUB,1,RUB,\n".as_bytes()
            )
            .is_err()
        );
    }

    #[test]
    fn tags_travel_in_a_tenth_column_only_when_there_are_any() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = Store::open(dir.path(), 1).unwrap();
        import(&mut store, SAMPLE.as_bytes()).unwrap();
        let mut csv = Vec::new();
        export(&store, &mut csv).unwrap();
        let text = String::from_utf8(csv).unwrap();
        assert!(text.lines().next().unwrap().ends_with(",коммент"), "{text}");

        let tagged = concat!(
            "дата,тип,счёт,категория или другой счет,сумма в валюте из,валюта из,сумма в валюте в,валюта в,коммент,теги\n",
            "2026-10-04,расход,Кошелёк,Еда,10,RUB,,,,Поездка; Работа ;\n",
            "2026-10-05,доход,Кошелёк,Зарплата,,,5,RUB,,Поездка\n",
            "2026-10-06,расход,Кошелёк,Еда,3,RUB,,,,\n",
        );
        assert_eq!(import(&mut store, tagged.as_bytes()).unwrap().added, 3);
        // An expense tag and an income tag are two tags, of their kinds.
        assert_eq!(Tag::read_all(&store, TagKind::Expense).len(), 2);
        assert_eq!(Tag::read_all(&store, TagKind::TopUp).len(), 1);
        let expense = Expense::read_all(&store)
            .unwrap()
            .into_iter()
            .find(|e| e.amount == 10.0)
            .unwrap();
        assert_eq!(Tag::links(&store, TagKind::Expense)[&expense.id].len(), 2);

        let mut csv = Vec::new();
        export(&store, &mut csv).unwrap();
        let text = String::from_utf8(csv.clone()).unwrap();
        assert!(
            text.lines().next().unwrap().ends_with(",коммент,теги"),
            "{text}"
        );
        assert!(
            text.contains("2026-10-04,расход,Кошелёк,Еда,10,RUB,,,,Поездка;Работа"),
            "{text}"
        );

        // Into a fresh ledger, and back out the same.
        let other = tempfile::tempdir().unwrap();
        let mut restored = Store::open(other.path(), 2).unwrap();
        import(&mut restored, csv.as_slice()).unwrap();
        let mut again = Vec::new();
        export(&restored, &mut again).unwrap();
        assert_eq!(String::from_utf8(again).unwrap(), text);
    }
}
