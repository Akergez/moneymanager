# Money Manager

Expenses, income and transfers across accounts in different currencies, in one
adaptive window: a table and a sidebar on a desktop, a list and a bottom bar
on a phone. Written in Rust and drawn with [GPUI](https://www.gpui.rs/) through
[gpui-mobile-components](https://github.com/Akergez/gpui-mobile-components),
the design system it shares with aiddiction and matterfast: one set of screens,
drawn for a pointer on a desktop and for a finger on a phone, on top of
[GPUI Kit](https://gpui-kit.com).

The ledger is a CRDT document (RDX, from [tresse](https://gitlab.com/ragusseven/tresse)).
It is a native `ledger.rdx` file in a Tresse repository. Tresse stores its
version history and synchronizes it with S3-compatible storage or an HTTP
server, with end-to-end encryption and CRDT merging across devices.

On upgrade, the application offers to convert the previous chunk format and
reset its sync settings. Records, identifiers, stamps and deletions are
preserved; the old directory is kept as a backup. Afterwards, configure Tresse
sync in the application's Settings. Old chunk remotes and `tresse1:` strings
are not compatible with the new repository protocol.

Storage written by an earlier Tresse can be replaced from the ledger itself:
**Settings → Storage → Rebuild Tresse storage** (or **Rebuild storage** on the
screen shown when the ledger cannot be opened). It writes `ledger.rdx` into a
fresh Tresse repository with one version: every record, stamp and deletion
stays; the version history, the old objects and the sync settings on this
device go, and the ledger stops syncing until a storage is set up again. The
storage on a server or in a bucket is not changed, so connect to a new one.

## What it does

- **Transactions** — the records of the current account: id, amount, category
  and date (and the comment where there is room). One switch turns the screen
  from expenses to income. Sortable by column; a row opens the record.
- **Categories** — expense and income categories, each with a colour and an
  icon. A category's row opens the page of its records, where it is edited
  and where **New expense…** makes a record in it.
- **Tags** — labels of your own across categories (a trip, a project), drawn
  like categories with a name, a colour and an icon, but a record carries any
  number of them. They are chips at the end of the record form and small
  pills on every record in the list. The Tags screen lists them with how many
  records each has; a tag's row opens the page of its records, where it is
  edited or deleted (it comes off its records; the records stay). Expenses and
  income each have tags of their own; transfers have none.
- **Charts** — one screen with all three: a pie by category (for the month or
  for all time), bars by month, and the running total through the chosen month.
- **Accounts** — chosen at the top of every screen, with the balance beside the
  name. Each has a currency and an opening balance.
- **Transfers** — between two accounts, with the two amounts entered
  separately and the exchange rate derived from them. A transfer shows as an
  expense on one account and an income on the other; deleting either side
  deletes both.
- **Sync** — on demand, through Tresse, with S3 or an HTTP server.
- **CSV** — import and export all accounts through the **More** menu (`⋯` on a
  desktop, `⋮` on a phone) in the screen's bar. Shortcuts: `Ctrl+Shift+I` and
  `Ctrl+Shift+E`.

## CSV import and export

Open **More** (`⋯`, or `⋮` on a phone) in the screen's bar and choose **Import CSV…** or
**Export CSV…**. The keyboard shortcuts are `Ctrl+Shift+I` and `Ctrl+Shift+E`.
On Android, import opens the system document picker and export opens the system
save dialog. Choosing a document grants access to that document; no storage
permission is requested. Cancelling a picker leaves the ledger unchanged.

Both commands use the same format: **UTF-8, comma-separated, nine columns in
the order below, and a tenth for tags**. The column names and operation types are Russian
regardless of the interface language. Empty fields still occupy a column, so
keep their commas, including the trailing comma for an empty comment.

| Column | Header | Value |
|--------|--------|-------|
| 1 | `дата` | Operation date. Export writes `YYYY-MM-DD`; import also accepts `DD.MM.YYYY`. |
| 2 | `тип` | `расход` (expense), `доход` (income), `перевод` (transfer), or `счёт` (an account). |
| 3 | `счёт` | Account name. For a transfer, this is the source account. |
| 4 | `категория или другой счет` | Expense/income category name, or the destination account name for a transfer. |
| 5 | `сумма в валюте из` | Positive amount debited from the account; empty for income. |
| 6 | `валюта из` | Source currency code, such as `RUB`; empty for income. |
| 7 | `сумма в валюте в` | Positive amount credited to the account; empty for expenses. |
| 8 | `валюта в` | Destination currency code, such as `USD`; empty for expenses. |
| 9 | `коммент` | Optional comment; may contain commas, quotes and line breaks when quoted. |
| 10 | `теги` | Optional, only in a file with this column: the record's tags, their names separated by `;`. Empty on transfers and account rows. |

Export writes the tenth column only when some record has a tag, so a ledger
without tags exports exactly the nine columns earlier versions read. Import
takes files of either width; a tag a row names that the ledger does not have
is made with the default style (expense tags and income tags are separate). A
tag's name cannot contain `;`.

Which fields to fill depends on the operation:

| Type | Column 4 | Source amount/currency (5–6) | Destination amount/currency (7–8) |
|------|----------|-----------------------------|----------------------------------|
| `расход` | Expense category | Required | Both empty |
| `доход` | Income category | Both empty | Required |
| `перевод` | Destination account | Required | Required |
| `счёт` | Empty | Both empty | Opening balance (may be zero or negative) and currency |

An account row has no date and no comment: `,счёт,Карта,,,,1500,RUB,`. It
states the account once, with its opening balance, and is how an account with
no operations travels too.

For transfers, the two accounts must differ. Both amounts are entered in their
respective account currencies; the exchange rate is implied by their ratio.
Currency codes must contain three ASCII letters. Import accepts lowercase
codes and normalizes them to uppercase. Amounts must be finite and greater
than zero; use a decimal point in exported files.

### Example

```csv
дата,тип,счёт,категория или другой счет,сумма в валюте из,валюта из,сумма в валюте в,валюта в,коммент
2026-10-01,доход,Карта,Зарплата,,,10000,RUB,
2026-10-02,расход,Карта,Еда,125.55,RUB,,,Обед
2026-10-03,перевод,Карта,Доллары,900,RUB,10,USD,Обмен
```

The example credits 10,000 RUB to `Карта`, debits 125.55 RUB for `Еда`, then
transfers 900 RUB from `Карта` to `Доллары`, crediting 10 USD there.

Use standard CSV quoting: enclose a field in double quotes if it contains a
comma, double quote or line break, and double any quote inside it. Import also
accepts a decimal comma inside a quoted amount:

```csv
2026-10-04,расход,Карта,Еда,"12,34",RUB,,,"Кофе, размер ""большой"""
```

### Import behavior

The header is optional on import; if present, it must be the first row and the
columns must retain the documented order. A UTF-8 BOM is accepted. Every
operation row must contain all nine columns, or ten with tags.

Import adds operations to the existing ledger. Accounts are matched by
name; categories are matched by name within their type (expense or income).
Names are case-sensitive, with surrounding whitespace removed from imported
names. Missing accounts are created in the specified currency with a zero
opening balance (or the one an account row gives); missing categories are
created with the default style. An account row sets the opening balance of the
account it names; account currencies and category styles are preserved.

An account currency that conflicts with the CSV, or multiple matching accounts
or categories with the same name, causes an error. Invalid dates, operation
types, amounts or field combinations also cause an error. The whole file is
validated before saving, so an invalid row leaves the ledger unchanged.

**Importing a file twice adds nothing twice.** A row is skipped while the
ledger holds an identical operation (same day, accounts, category, amounts and
comment) that no earlier row of the file has been matched with; identical rows
within one file are all added. The notice after an import says how many were
added and how many were already in the ledger.

### Export behavior

Export writes the header, one account row per account, and all stored
expenses, income and transfers from **all accounts**, ordered by date, regardless of the current account or screen.
Each transfer is written once, with both amounts, rather than as two separate
expense/income rows. Comments are escaped using standard CSV quoting.

Account/category/tag IDs, category and tag styles, categories and tags without operations, hidden
and main accounts, and sync settings are not included. Importing an export
into a fresh ledger restores its accounts with their opening balances and
currencies, and its operations.

## First run

With no ledger on the device the application asks which there should be, on
two tabs:

- **Server** — **Start without sync** makes a new ledger with its first account
  (rename it and change its currency like any other account). Or sign in on a
  Tresse server: the server the build names is filled in and can be changed;
  **Sign in** opens its page in the browser, which brings the repository back
  to the app. A **config string** (`tresse://…`, exported by another device,
  copied from the server's page or by `tresse config export`) can be pasted
  instead.
- **Custom storage** — every field of an S3 or HTTP storage, with an
  encryption key generated on request.

Settings has the same two tabs (without starting a new ledger), and
**Export config string** on the first for setting up the next device.
The build's default server comes from `TRESSE_SERVER_URL` at build time
(CI: the repository variable of that name).

The first-run choice is unchanged once made. The repository config is written to files,
not application preferences. Sync settings can also be changed using Tresse's
CLI; the GUI re-reads them when opening Settings and synchronizing.

## Where things are kept

The ledger is private to the application. CSV files are read and written only
at the locations chosen in the system file picker, so no storage permission is ever asked
for — on a phone the Android manifest declares the network permission only,
and the flatpak has no filesystem access at all.

| What | Where |
|------|-------|
| Tresse worktree: `ledger.rdx`, `tresse.toml`, `.tresse/` | `$XDG_DATA_HOME/app.akergez.MoneyManager/ledger-tresse/` |
| Original data after conversion | `$XDG_DATA_HOME/app.akergez.MoneyManager/ledger.pre-tresse/` |
| Settings: `settings.json` | `$XDG_CONFIG_HOME/app.akergez.MoneyManager/` |

On Android both are under the application's internal data directory.

`settings.json` contains interface preferences, `default_currency` and
`last_account` (legacy selection memory). Account visibility and the main
account are stored in the synced RDX ledger instead. Old `source` and `remote`
keys are removed after conversion.
Tresse configuration has one source of truth:

- `tresse.toml` at the worktree root: tracked language and ignore rules;
- `.tresse/remotes.toml`: device-local `[[remotes.origin]]` with coordinates,
  credentials, repository ID and encryption key (owner-only permissions on Unix);
- `.tresse/`: standard Tresse CAS, index, lock and recovery journal.

The application reads these files and honors their values without adding
in-memory language, ignore or remote settings. The ledger's internal RDX
stamps are produced by the application; Tresse version stamps are produced by
Tresse. The GUI and the CLI share the same repository format and locks.
For example, on a desktop with the usual XDG paths:

```sh
cd ~/.local/share/app.akergez.MoneyManager/ledger-tresse
tresse config validate
tresse log --changes
tresse commit
tresse sync
tresse restore
```

`money-tresse` embeds the filesystem adapters from `tresse-cli` at the commit
pinned in Cargo.lock. Upstream currently provides domain libraries but no CLI
library target. Diff, native RDX restore, CAS and object exchange remain in the
upstream crates. Adapter changes are covered by an interoperability test:

```sh
TRESSE_CLI_BIN=/path/to/tresse cargo test -p money-tresse \
  upstream_cli_and_embedded_client_share_config_history_locks_and_sync -- --ignored
```

### Category colours and icons

A category's name, colour and icon are all part of its record in the ledger
and sync with it. The colour and the icon are two optional fields after the
name: a record without them — every one the terminal version wrote — is an
unstyled category, drawn in a colour derived from its name, the same on every
device; and a client that knows nothing of them still finds the name where it
always was. A record is rewritten whole, so a client that rewrites a category
without these fields removes its colour and icon.

Styles that an earlier build kept in `settings.json` (`category_styles`) are
moved into the ledger the first time it is opened.

## The ledger format

The internal record schema is unchanged from the terminal version;
`crates/money-core/src/store.rs` describes it:

- The document is a tuple of eleven collections: categories, expenses, income
  categories, income, accounts, transfers, account preferences, then expense
  tags, the links that put them on expenses, income tags and their links. The
  trailing ones are absent in older ledgers. A record is a tuple whose first
  child is its key: a hex ID for domain records, a name for preferences, and
  `<record id>:<tag id>` for a tag's link.
- Tags are backward compatible both ways. A version from before them merges
  the document by position and keeps the collections it does not know, so
  tags travel through its syncs untouched; and since a tag is a link record of
  its own rather than a field of the expense, an older version rewriting an
  expense (it always writes all five fields) does not drop its tags. Two
  devices tagging one record at once both keep their tag; taking a tag off is
  the link's tombstone. `store.rs` and `tags.rs` have tests for both.
- Every write is stamped (source + Lamport time); a newer write replaces the
  record, and a deletion is a tombstone. This is what makes two devices'
  changes merge.
- Each local write atomically updates `ledger.rdx` under the shared repository
  lock, without creating a Tresse version or CAS objects. Reopening the
  application preserves these uncommitted edits. Sync or an explicit
  `tresse commit` commits the accumulated edits. Native RDX diff, content
  fragmentation and synchronization belong to Tresse. Sync then exchanges
  objects, restores incoming versions and reloads the ledger.

**Accounts…** in the account menu lists every account, including hidden ones.
**Hide** removes an account from the account menu and transfer selectors;
**Show** restores it. History, transfers, balances and CSV exports stay intact.
**Make main** chooses the account opened at startup and also shows it if hidden.
These choices sync with the ledger. Hiding the last visible account is refused.

Visibility uses separate `hidden:<account-id>` records, so renaming an account
cannot overwrite its visibility. The main account is one `main_account` record
containing an account ID. Concurrent main choices merge by the RDX stamp's
deterministic last-write-wins order. If that account is hidden or missing, the
visible default account, then the visible account with the smallest ID, is used.
If concurrent hides leave every account hidden, the default account (or the
smallest ID) stays visible as the effective main account. A later **Show** writes
a fresh stamp and restores the account normally. There is always exactly one
effective main account when the ledger has accounts.

A device joining a ledger syncs **first** and only then makes sure the default
account exists: made before the first sync, its fresh stamp would win over the
synced account's name, currency and opening balance. `store.rs` has a test
that shows both orders.

## Keyboard

| Key | Action |
|-----|--------|
| `Ctrl+1` / `Ctrl+2` / `Ctrl+3` / `Ctrl+4` | Transactions / Categories / Charts / Tags |
| `Ctrl+M` | Switch between expenses and income |
| `Ctrl+N` | New record (a new category on the Categories screen, a new tag on the Tags screen, a record in the category or with the tag on the page of one) |
| `Escape`, `Alt+Left` | Back from the page of a category's or a tag's records |
| `Up` / `Down` | Select the previous / next record |
| `Enter`, `Ctrl+E` | Open the selected record |
| `Ctrl+T` | New transfer |
| `Ctrl+S` | Sync |
| `Ctrl+,` | Settings |
| `Ctrl+Q` | Quit |

## Building

A recent stable Rust (the version is pinned in `rust-toolchain.toml`) and,
on Linux, the development packages for Wayland/X11, Vulkan and fonts:

```sh
# Debian / Ubuntu
sudo apt install pkg-config libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev \
  libxcb1-dev libvulkan-dev libfontconfig-dev libfreetype-dev

# Fedora
sudo dnf install wayland-devel libxkbcommon-devel libxkbcommon-x11-devel \
  libxcb-devel vulkan-loader-devel fontconfig-devel freetype-devel
```

```sh
cargo run -p money-manager             # debug
cargo build --release -p money-manager # optimised, quick to rebuild
cargo build --profile dist -p money-manager  # what ships: thin LTO, slow
cargo test --workspace
```

`MONEY_MANAGER_DEMO=1 cargo run -p money-manager` skips the first-run question
and fills an *empty* installation with sample records. Point `XDG_DATA_HOME`
and `XDG_CONFIG_HOME` somewhere disposable to try it without touching your own
ledger. `MONEY_MANAGER_INPUT=touch MONEY_MANAGER_SIZE=400x800` draws the window
the way a phone does: for a finger, with sheets, a navigation bar and a FAB.

The window is drawn with the design system alone: `tests/design_system.rs`
fails on any direct use of GPUI Kit's components in the application's sources.

## Packages

| Target | How | Result |
|--------|-----|--------|
| Linux tarball | `build-aux/package-tarball.sh` | `dist/money-manager-<v>-linux-<arch>.tar.gz` |
| Flatpak | `build-aux/publish-flatpak.sh` | `.flatpak-repo`; with `--site`, a signed repository and `.flatpakref` |
| Windows (cross-built on Linux) | `build-aux/build-windows.sh x86_64` | `dist/money-manager-<v>-windows-x86_64.zip` |
| Android | `build-aux/build-android.sh` | `dist/money-manager-<v>-android-aarch64.apk` |
| macOS | `build-aux/package-macos.sh` | `dist/money-manager-<v>-macos-<arch>.dmg` |

Each script's header says what it needs. `android/README.md` covers the
Android build in detail.

## Tests

- `cargo test --workspace` — the ledger (format, merge, sync against an
  in-memory remote) and everything in the application that needs no window.
- `build-aux/ui-tests.sh --headless` — the scenarios in
  `crates/money-manager/tests/ui/`, played into the real application under a
  headless sway. Each is a short script of keys, clicks and `expect:` checks
  against the application's state; see `crates/money-manager/src/script.rs`.

## Continuous integration

`.github/workflows/ci.yml` tests every push, builds every package as a
workflow artifact, and publishes only from a version tag (`v1.2.3`) whose
number every crate carries — `build-aux/check-version.sh` enforces it. The
version lives once, in `[workspace.package]` of `Cargo.toml`.

A release needs these repository secrets:

| Secret | For |
|--------|-----|
| `ANDROID_KEYSTORE_B64`, `ANDROID_KEYSTORE_PASSWORD` | Signing the Android package |
| `FLATPAK_GPG_ID`, `FLATPAK_GPG_KEY_B64`, `FLATPAK_GPG_PASSPHRASE` | Signing the flatpak repository |

### Signing the Android package

Android installs an update only over a package signed with the same key, so
every release is signed with one key, made once:

```sh
keytool -genkeypair -storetype PKCS12 -keystore release.p12 -alias money-manager \
  -keyalg RSA -keysize 4096 -validity 10000
base64 -w0 release.p12   # → ANDROID_KEYSTORE_B64
```

Keep the keystore somewhere safe and out of the repository: losing it means
nobody can update an installed copy.

## What the terminal version had that this does not

The terminal application's `import-csv` command is not available in the GUI.
Local chunk-format data from the previous GUI is converted on launch. A
terminal ledger can first be copied into the application's old `ledger/`
directory, or imported using the core library's `import_legacy_blob` API.
Remote-only old ledgers must be downloaded by an old client before conversion.
The new Tresse repository cannot sync with the old chunk protocol.

## License

GPL-3.0; see [LICENSE](LICENSE). The bundled Inter font is under the SIL Open
Font License 1.1 (`crates/money-manager/assets/fonts/Inter-LICENSE.txt`). The
icons are [Lucide](https://lucide.dev), ISC.
