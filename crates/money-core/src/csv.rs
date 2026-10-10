//! Nine-column CSV interchange for all accounts. Transfers are exported once.

use std::collections::HashMap;
use std::io::{Read, Write};

use chrono::NaiveDate;
use rdx_rs::RdxValue;
use uuid::Uuid;

use crate::models::{Account, Category, Expense, TopUp, TopUpCategory, Transfer};
use crate::store::{self, Store, hex_encode};

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

struct Row {
    date: NaiveDate,
    kind: String,
    account: String,
    category: String,
    amount_from: Option<f64>,
    currency_from: String,
    amount_to: Option<f64>,
    currency_to: String,
    comment: String,
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

impl Row {
    fn parse(fields: &::csv::StringRecord) -> Result<Self, String> {
        if fields.len() != 9 {
            return Err("Ожидается 9 колонок".into());
        }
        let row = Self {
            date: NaiveDate::parse_from_str(fields[0].trim(), "%Y-%m-%d")
                .or_else(|_| NaiveDate::parse_from_str(fields[0].trim(), "%d.%m.%Y"))
                .map_err(|_| "Дата должна быть YYYY-MM-DD или DD.MM.YYYY".to_string())?,
            kind: fields[1].trim().to_lowercase(),
            account: fields[2].trim().to_string(),
            category: fields[3].trim().to_string(),
            amount_from: amount(&fields[4])?,
            currency_from: currency(&fields[5])?,
            amount_to: amount(&fields[6])?,
            currency_to: currency(&fields[7])?,
            comment: fields[8].to_string(),
        };
        if row.account.is_empty() || row.category.is_empty() {
            return Err("Укажите счёт и категорию или счёт получателя".into());
        }
        let from = row.amount_from.is_some() && !row.currency_from.is_empty();
        let to = row.amount_to.is_some() && !row.currency_to.is_empty();
        match row.kind.as_str() {
            "расход" if from && row.amount_to.is_none() && row.currency_to.is_empty() => {}
            "доход" if to && row.amount_from.is_none() && row.currency_from.is_empty() => {}
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
    records.push((
        collection,
        hex_encode(&id),
        vec![RdxValue::Str(name.into())],
    ));
    categories.insert(name.into(), vec![id.clone()]);
    Ok(id)
}

/// Append operations, creating missing accounts (zero opening balance) and
/// categories. The entire file is validated before one atomic native-file edit.
/// Repeated imports append again: this format contains no operation IDs.
pub fn import(store: &mut Store, reader: impl Read) -> Result<usize, String> {
    let mut text = String::new();
    let mut reader = reader;
    reader
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    let text = text.trim_start_matches('\u{feff}');
    let mut csv = ::csv::ReaderBuilder::new()
        .has_headers(false)
        .from_reader(text.as_bytes());
    let mut accounts = Account::read_all(store)?;
    let mut expenses: HashMap<String, Vec<Vec<u8>>> = HashMap::new();
    for category in Category::read_all(store)? {
        expenses.entry(category.name).or_default().push(category.id);
    }
    let mut income: HashMap<String, Vec<Vec<u8>>> = HashMap::new();
    for category in TopUpCategory::read_all(store)? {
        income.entry(category.name).or_default().push(category.id);
    }
    let mut records = Vec::new();
    let mut count = 0;
    for (ix, fields) in csv.records().enumerate() {
        let fields = fields.map_err(|e| format!("CSV: {e}"))?;
        let line = fields.position().map_or(ix as u64 + 1, |p| p.line());
        if ix == 0
            && fields
                .get(0)
                .is_some_and(|s| s.trim().eq_ignore_ascii_case("дата"))
        {
            if fields.len() != 9 {
                return Err("Заголовок должен содержать 9 колонок".into());
            }
            continue;
        }
        let result = (|| {
            let row = Row::parse(&fields)?;
            let date = RdxValue::Str(row.date.format("%Y-%m-%d").to_string());
            let comment =
                store::opt_str_value((!row.comment.is_empty()).then_some(row.comment.as_str()));
            let (collection, values) = match row.kind.as_str() {
                "расход" | "доход" => {
                    let expense = row.kind == "расход";
                    let (value, code) = if expense {
                        (row.amount_from, &row.currency_from)
                    } else {
                        (row.amount_to, &row.currency_to)
                    };
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
                    (
                        if expense {
                            store::EXPENSES_IDX
                        } else {
                            store::TOP_UPS_IDX
                        },
                        vec![
                            RdxValue::Str(hex_encode(&category)),
                            RdxValue::Float(value.ok_or("Не указана сумма")?),
                            comment,
                            date,
                            RdxValue::Str(hex_encode(&account)),
                        ],
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
                    (
                        store::TRANSFERS_IDX,
                        vec![
                            RdxValue::Str(hex_encode(&from)),
                            RdxValue::Str(hex_encode(&to)),
                            RdxValue::Float(row.amount_from.ok_or("Не указана сумма из")?),
                            RdxValue::Float(row.amount_to.ok_or("Не указана сумма в")?),
                            comment,
                            date,
                        ],
                    )
                }
            };
            records.push((collection, hex_encode(&new_id()), values));
            Ok::<_, String>(())
        })();
        result.map_err(|e| format!("Строка {line}: {e}"))?;
        count += 1;
    }
    store.import_records(records)?;
    Ok(count)
}

/// Export all stored operations, ordered by date, using UTF-8 and comma delimiters.
/// Income uses the destination pair; expense uses the source pair.
pub fn export(store: &Store, writer: impl Write) -> Result<(), String> {
    let accounts = Account::read_all(store)?;
    let accounts: HashMap<_, _> = accounts.iter().map(|a| (a.id.as_slice(), a)).collect();
    let expenses = Category::read_all(store)?;
    let expenses: HashMap<_, _> = expenses
        .iter()
        .map(|c| (c.id.as_slice(), c.name.as_str()))
        .collect();
    let income = TopUpCategory::read_all(store)?;
    let income: HashMap<_, _> = income
        .iter()
        .map(|c| (c.id.as_slice(), c.name.as_str()))
        .collect();
    let account = |id: &[u8]| {
        accounts
            .get(id)
            .copied()
            .ok_or_else(|| format!("Не найден счёт {}", hex_encode(id)))
    };
    let mut rows = Vec::new();
    for entry in Expense::read_all(store)? {
        let a = account(&entry.account_id)?;
        let category = expenses
            .get(entry.category_id.as_slice())
            .ok_or("Не найдена категория расхода")?;
        rows.push([
            entry.date.to_string(),
            "расход".into(),
            a.name.clone(),
            (*category).into(),
            entry.amount.to_string(),
            a.currency.clone(),
            String::new(),
            String::new(),
            entry.comment.unwrap_or_default(),
        ]);
    }
    for entry in TopUp::read_all(store)? {
        let a = account(&entry.account_id)?;
        let category = income
            .get(entry.category_id.as_slice())
            .ok_or("Не найдена категория дохода")?;
        rows.push([
            entry.date.to_string(),
            "доход".into(),
            a.name.clone(),
            (*category).into(),
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
        rows.push([
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
    rows.sort();
    let mut writer = ::csv::Writer::from_writer(writer);
    writer.write_record(HEADER).map_err(|e| e.to_string())?;
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
        assert_eq!(import(&mut store, SAMPLE.as_bytes()).unwrap(), 3);
        let mut csv = Vec::new();
        export(&store, &mut csv).unwrap();
        let mut restored = Store::open(second.path(), 2).unwrap();
        assert_eq!(import(&mut restored, csv.as_slice()).unwrap(), 3);
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
            "2026-10-10,расход,А,Еда,1,RUB,,,коммент,лишнее",
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
}
