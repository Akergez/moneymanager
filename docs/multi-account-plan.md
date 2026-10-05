# План: мультивалютные счета, переводы, удаление записей, баланс

## Контекст

Сейчас в money-manager-ng один неявный «кошелёк»: все расходы (Expense) и пополнения (TopUp) лежат в общих коллекциях без привязки к счёту и валюте, суммы — безразмерные f64. Удалять записи из TUI нельзя (Store::delete и Expense::delete/TopUp::delete есть, но помечены #[allow(dead_code)]), баланс нигде не считается.

## Цель

Несколько счетов, у каждого своя валюта. Для каждого счёта свои списки трат и пополнений и свои графики, всё как сейчас работает для единственного счёта.

Переводы между счетами. Пользователь вводит, сколько списано со счёта A (в валюте A) и сколько зачислено на счёт B (в валюте B). Курс вычисляется автоматически как amount_to / amount_from. На счёте A перевод отображается как трата категории «Перевод», на счёте B — как пополнение категории «Перевод».

Удаление записей через tombstone. Удаление траты или пополнения удаляет только эту запись. Удаление перевода удаляет обе его ноги, и расход, и доход.

Баланс каждого счёта считается и показывается в интерфейсе.

## Что важно в текущей архитектуре

src/store.rs: документ — это Tuple из COLLECTION_COUNT = 4 коллекций Eulerian (индексы CATEGORIES_IDX…TOP_UPS_IDX). Запись — Tuple[key_hex, field1, field2, …]. upsert штампует все поля одним stamp, delete пишет tombstone (нечётное time), а records(idx) отфильтровывает tombstone.

Проверено в rdx-rs (merge/collection.rs::merge_tuple_children): кортежи разной длины сливаются позиционно, с дополнением до большей длины, и LWW идёт по каждой позиции отдельно. Из этого следует:

- поднять COLLECTION_COUNT с 4 до 6 безопасно: старые 4-элементные документы и чанки сливаются с 6-элементными патчами без потерь;
- новые поля можно дописывать в конец записей. У старых записей этих полей нет: child_str вернёт "", child_f64 вернёт 0.0;
- если старый клиент сделает upsert 5-полевой записи, он не затрёт 6-е поле (account_id), потому что позиция отсутствует в его патче.

Сервисы src/services/*_service.rs устроены одинаково: create / create_with_id / read_all / update / delete и хелпер fields(...). Новые сервисы пишем по тому же шаблону.

Вьюхи и графики получают срезы &[Expense]/&[Category] (например, ExpensesView::new(&state.expenses, &state.categories) в src/tui/ui.rs), а графики — через трейты ChartableItem/ChartableCategory (src/tui/views/generic_chart.rs). Поэтому фильтрацию по счёту делаем в AppState, и вьюхи почти не меняются.

## Ключевые проектные решения

Категории остаются общими для всех счетов. «Еда» одинаково используется и в рублёвом, и в долларовом счёте.

Перевод хранится одной записью в новой коллекции TRANSFERS_IDX. Ноги перевода (трата на A, пополнение на B) не хранятся, а строятся на лету при чтении. Почему так:

- удаление перевода — это один tombstone, и оно атомарно при синхронизации. Если бы ноги были отдельными записями, при конкурентной правке на другом устройстве одна нога могла бы «воскреснуть» (LWW по записи), и перевод стал бы наполовину удалённым;
- ноги не могут разъехаться по сумме или дате с самим переводом.

Курс не хранится, а вычисляется: rate = amount_to / amount_from. Показываем оба направления: 1 USD = 92.35 RUB и 1 RUB = 0.0108 USD.

Счёт по умолчанию для старых данных. Используется фиксированный id DEFAULT_ACCOUNT_ID = [0u8; 16]. Записи с пустым account_id считаются принадлежащими ему. Существующие записи не переписываем, чтобы не плодить огромный чанк и конфликты. Id детерминированный, поэтому если два устройства одновременно создадут дефолтный счёт, при синхронизации получится одна запись (LWW).

Баланс = opening_balance + Σ пополнений + Σ входящих переводов − Σ трат − Σ исходящих переводов. Учитываются все живые записи. Суммы по разным валютам не складываются: итог «по всем счетам» в единой валюте в этот объём не входит.

Переводы на графиках показываются как отдельная виртуальная категория «⇄ Перевод», как того требует ТЗ («записывается в траты как перевод»). На bar/line её можно отключить существующим выбором категорий. На pie (treemap) добавляется глобальный переключатель x — «скрыть переводы на графиках».

## Модель данных

### Раскладка записей в RDX

| Коллекция | idx | Поля после ключа |
|---|---|---|
| categories | 0 | name (без изменений) |
| expenses | 1 | category_id, amount, comment, date, **account_id** |
| top-up categories | 2 | name (без изменений) |
| top_ups | 3 | category_id, amount, comment, date, **account_id** |
| accounts | 4 | name, currency, opening_balance |
| transfers | 5 | from_account_id, to_account_id, amount_from, amount_to, comment, date |

### src/models.rs

```rust
pub struct Account { id, name: String, currency: String /* ISO-4217, "RUB" */, opening_balance: f64 }
pub struct Transfer { id, from_account_id, to_account_id, amount_from: f64, amount_to: f64, comment: Option<String>, date: NaiveDate }

// Expense / TopUp: новые поля (только в памяти):
pub account_id: Vec<u8>,
pub transfer: Option<TransferLeg>,   // Some(..) — виртуальная нога перевода

pub struct TransferLeg { transfer_id: Vec<u8>, counterparty_account_id: Vec<u8>, rate: f64 }

impl Transfer { pub fn rate(&self) -> f64 { self.amount_to / self.amount_from } }
```

## Этапы реализации

### Этап 1. Хранилище и сервисы

src/store.rs: добавить ACCOUNTS_IDX = 4, TRANSFERS_IDX = 5, COLLECTION_COUNT = 6, pub const DEFAULT_ACCOUNT_ID: [u8; 16] = [0; 16]. Снять #[allow(dead_code)] с Store::delete.

src/services/expense_service.rs, top_up_service.rs:

- create/create_with_id принимают account_id: &[u8], а fields(...) дописывает его 5-м полем;
- read_all: account_id = hex_decode(&child_str(rec, 5)), при пустом значении подставляется DEFAULT_ACCOUNT_ID; transfer: None;
- update сохраняет и category, и account (по образцу того, как сейчас сохраняется category);
- снять dead_code с delete.

Новые файлы src/services/account_service.rs и transfer_service.rs, по шаблону category_service.rs: create, create_with_id, read_all, update, delete. Transfer::create валидирует: from != to, обе суммы > 0.

Account::ensure_default(store, currency): если среди счетов нет DEFAULT_ACCOUNT_ID, создаёт его («Основной», валюта из конфига). Вызывается в main.rs после Store::open и миграции legacy-блоба.

src/config.rs: необязательное поле default_currency: Option<String> (по умолчанию "RUB") и необязательное last_account: Option<String> (hex). Это локальное состояние UI, оно не синхронизируется. Дополнить CONFIG_TEMPLATE.

### Этап 2. Доменная логика: src/ledger.rs (новый модуль)

Чистые функции без TUI, покрываемые тестами:

```rust
pub struct AccountLedger { pub expenses: Vec<Expense>, pub top_ups: Vec<TopUp>, pub balance: f64,
                           pub total_in: f64, pub total_out: f64 }

pub const TRANSFER_CATEGORY_ID: [u8; 16] = [0xFF; 16];   // виртуальная категория

pub fn ledger_for(account: &Account, expenses: &[Expense], top_ups: &[TopUp],
                  transfers: &[Transfer]) -> AccountLedger;
pub fn balances(accounts, expenses, top_ups, transfers) -> HashMap<Vec<u8>, f64>;
pub fn format_rate(t: &Transfer, from: &Account, to: &Account) -> String; // "1 USD = 92.35 RUB"
```

Исходящий перевод (from == account) превращается в Expense { id: transfer.id, category_id: TRANSFER_CATEGORY_ID, amount: amount_from, date, comment: "→ <имя B> (1 A = r B)", transfer: Some(..) }.

Входящий перевод (to == account) превращается в TopUp { .., amount: amount_to, comment: "← <имя A> (…)" }.

with_transfer_category(categories) -> Vec<Category> дописывает виртуальную категорию «⇄ Перевод» (то же для TopUpCategory). Её не видно в формах создания трат и пополнений.

Переводы с «осиротевшим» счётом (счёт удалён или ещё не синхронизирован) по-прежнему попадают в ленту существующей стороны, контрагент показывается как «Unknown».

### Этап 3. AppState: текущий счёт и отфильтрованные данные

src/tui/app_state.rs:

- новые поля accounts: Vec<Account>, transfers: Vec<Transfer>, current_account: Vec<u8>, balances: HashMap<Vec<u8>, f64>;
- кэш для вьюх: account_expenses, account_top_ups, expense_categories_ext, top_up_categories_ext (с «Переводом»). Всё пересчитывается в reload_data() и при смене счёта (recompute_view());
- ui.rs::draw_content передаёт во вьюхи кэш текущего счёта вместо state.expenses/state.top_ups. Сами вьюхи и графики не трогаем;
- last_expense_date()/last_top_up_date() (есть в незакоммиченном диффе) считаются по текущему счёту;
- переключение счёта: глобальные клавиши [ / ] (предыдущий/следующий счёт) и Enter во вкладке «Счета». Выбор сохраняется в config.last_account;
- при смене счёта сбрасываются scroll_offset/выделение во вьюхах таблиц.

Показ баланса: в верхней строке (draw_footer, сейчас там подсказки) появляется компактный префикс Наличные · RUB · 12 345.67. Отрицательный баланс выделяется красным. На узком экране — только RUB 12 345.

### Этап 4. Вкладка «Счета» и форма счёта

Tab::Accounts (клавиша 0, первая в цикле Tab), src/tui/views/accounts_widget.rs. Таблица: Название | Валюта | Доходы | Расходы | Баланс, текущий счёт помечен ●. Клавиши: n — новый счёт, e — редактировать, Enter — сделать текущим, ↑/↓ — выбор.

src/tui/forms/account_form_widget.rs по образцу category_form_widget.rs, поля Название, Валюта (3 латинские буквы, приводятся к верхнему регистру), Начальный остаток (по умолчанию 0). Режим редактирования делает upsert с тем же id.

Удаление счетов в этот объём не входит: без каскадной политики оно небезопасно. Правка названия и валюты разрешена.

Обновить types.rs (next/previous), ui.rs::tab_titles_for_width, draw_tabs, маппинг кликов в app_state.rs::handle_mouse (индексы сдвигаются, нужно вынести в одну функцию Tab::from_index/Tab::index, чтобы не дублировать match в трёх местах).

### Этап 5. Переводы: форма и вкладка

src/tui/forms/transfer_form_widget.rs по образцу expense_form_widget.rs (там уже есть селектор категории category_next/prev, его переиспользуем для счетов). Поля:

- Со счёта (селектор, по умолчанию текущий счёт),
- На счёт (селектор, текущий счёт исключён),
- Списано (в валюте A, суффикс валюты рядом с полем),
- Зачислено (в валюте B). Если валюты совпадают и поле пустое, при сабмите подставляется значение Списано,
- Дата (по умолчанию дата последнего перевода или сегодня),
- Комментарий.

Под полями — живая строка «Курс: 1 USD = 92.35 RUB · 1 RUB = 0.0108 USD», которая пересчитывается при каждом вводе. Валидация: from != to, суммы > 0, корректная дата.

Tab::Transfers (src/tui/views/transfers_widget.rs): таблица всех переводов Дата | Откуда | Куда | Списано | Зачислено | Курс | Комментарий с сортировкой через SortableState. n открывает форму перевода. Форму также можно открыть глобальной клавишей t с любой вкладки.

ViewInputResult::OpenTransferForm, OpenAccountForm.

### Этап 6. Удаление записей (tombstones)

Выделение строки. Сейчас таблицы только скроллятся (scroll_offset) и выбранной строки нет. В ExpensesViewState/TopUpsViewState/TransfersViewState добавляется selected: usize: ↑/↓ двигают выделение, скролл следует за ним, используется TableState::select + row_highlight_style (уже задан). Клик мышью по строке выделяет её.

Запрос удаления. d / Delete возвращает ViewInputResult::RequestDelete(RecordRef). Сейчас ViewInputResult имеет derive(Copy), поэтому либо переходим на Clone, либо используем [u8; 16]. Рекомендация: [u8; 16] (все id — UUID), Copy сохраняется.

```rust
pub enum RecordRef { Expense([u8;16]), TopUp([u8;16]), Transfer([u8;16]) }
```

Вьюха трат и пополнений сама отличает ногу перевода: если row.transfer.is_some(), отдаёт RecordRef::Transfer(transfer_id).

Подтверждение. Новый src/tui/forms/confirm_dialog_widget.rs: модалка «Удалить трату 450.00 RUB · Еда · 2026-09-12?» или «Удалить перевод 100 USD → 9 235 RUB? Будут удалены расход на „Карта USD“ и пополнение на „Наличные“». y/Enter подтверждает, n/Esc отменяет. Ставится в тот же ряд if form.is_active в handle_input/handle_mouse/draw.

Исполнение (AppState::delete_record): Expense::delete / TopUp::delete / Transfer::delete, то есть store.delete(idx, key). Для перевода достаточно одного tombstone, обе ноги исчезнут автоматически, потому что они строятся из transfers. Затем reload_data() и статус «Удалено» в state.status.

Клавиша d сейчас не занята ни во вьюхах, ни глобально (занятые буквы: n, m, a, c, r, s, q, пробел), конфликта нет. Удаление в этот объём включает только траты, пополнения и переводы. Удаление категорий и счетов вынесено за рамки.

Undo: необязательно, как будущее улучшение. Повторный upsert с тем же id и новым stamp «воскрешает» запись, для этого достаточно держать последнюю удалённую запись в AppState.

### Этап 7. Сопутствующее

- src/import.rs: необязательная колонка account_id для expenses/top-ups (если её нет — DEFAULT_ACCOUNT_ID), новые таблицы accounts (id,name,currency,opening_balance) и transfers (id,from_account_id,to_account_id,amount_from,amount_to,comment,date). Обновить doc-таблицу в шапке модуля.
- Форматирование денег: format_money(amount, currency) в src/tui/utils.rs. Два знака, разделитель тысяч, символ для частых валют (RUB ₽, USD $, EUR €, остальное — ISO-код). Заголовки таблиц и графиков показывают валюту текущего счёта (Expenses · Наличные (RUB) · Sum: …).
- Подсказки в футере (draw_footer): добавить d:Del, [/]:Acc, t:Transfer.
- README.md: раздел про счета, переводы, удаление и новые таблицы CSV. sample_data.sql/load_sample_data.sh при необходимости.

Поднять версию до 0.4.0. Описать совместимость: старый клиент видит все записи как «дефолтный счёт», не видит переводов (его баланс будет неверен), но ничего не портит благодаря позиционному merge.

## Критичные файлы

- src/store.rs, src/models.rs, src/main.rs, src/config.rs, src/import.rs
- src/services/{expense,top_up}_service.rs (изменения), account_service.rs, transfer_service.rs (новые)
- src/ledger.rs (новый)
- src/tui/app_state.rs, src/tui/ui.rs, src/tui/types.rs
- src/tui/views/{expenses,top_ups}_widget.rs (выделение строки, удаление), accounts_widget.rs, transfers_widget.rs (новые)
- src/tui/forms/{account_form,transfer_form,confirm_dialog}_widget.rs (новые)

## Порядок коммитов

1. Хранилище, модели, сервисы, дефолтный счёт, тесты хранилища.
2. ledger.rs и его тесты.
3. AppState с текущим счётом, фильтрацией и балансом в шапке.
4. Вкладка и форма счетов.
5. Форма и вкладка переводов.
6. Выделение строк, удаление, диалог подтверждения.
7. CSV, README, версия.

## Проверка

Юнит-тесты (cargo test, по образцу тестов в store.rs/import.rs на временных директориях):

- compat: документ, записанный с COLLECTION_COUNT = 4, открывается новым кодом; старые расходы получают DEFAULT_ACCOUNT_ID; 5-полевой upsert не затирает account_id у 6-полевой записи (моделирует старый клиент);
- ensure_default идемпотентен; на двух Store с разными source после rdx_rs::merge остаётся ровно один дефолтный счёт (аналог divergent_docs_merge_like_sync);
- ledger_for: трата и пополнение попадают только на свой счёт; перевод даёт трату на A (amount_from) и пополнение на B (amount_to); баланс считается по формуле; rate() и format_rate верны (включая одинаковую валюту, rate = 1);
- удаление: Expense::delete убирает только трату; Transfer::delete убирает обе ноги из обоих леджеров, и баланс обоих счетов возвращается к исходному; tombstone переживает seal() и переоткрытие;
- Transfer::create отклоняет from == to и суммы ≤ 0;
- импорт CSV: accounts, transfers, expenses без колонки account_id.

Ручная проверка в TUI (cargo run -- --data /tmp/mm-test.chunks, плюс копия реальных данных для проверки миграции):

- На старых данных появляется счёт «Основной» со всеми тратами и пополнениями, графики совпадают с прежними.
- Создать счёт «Карта USD» (USD) и переключиться на него через [/]: списки и графики пустые, баланс равен начальному остатку.
- Сделать перевод 100 USD → 9 235 RUB. В форме показывается курс 1 USD = 92.35 RUB. На USD-счёте появилась трата «⇄ Перевод 100», на рублёвом — пополнение «⇄ Перевод 9 235», балансы в шапке и во вкладке «Счета» изменились.
- Удалить ногу перевода (d на вкладке трат) → в диалоге упоминаются обе ноги → после подтверждения перевод пропал на обоих счетах и во вкладке «Переводы».
- Удалить обычную трату: удаляется только она, баланс растёт.
- s (sync) между двумя директориями или устройствами: удаление и новые счета доезжают, дубликатов дефолтного счёта нет.

cargo clippy без новых предупреждений, dead_code-атрибуты с delete сняты.
