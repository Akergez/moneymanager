# Money Manager

A terminal-based money management application built with Rust, featuring expense tracking, income (top-ups) tracking, and visual analytics.

## Features

- 📊 **Expense Tracking**: Track your expenses with customizable categories
- 💰 **Income Management**: Record income/top-ups with categories
- 🏦 **Multiple Accounts**: Each account has its own currency, balance, lists and charts
- ⇄ **Transfers**: Move money between accounts, including currency exchange with an automatic rate
- 🗑️ **Deletion**: Delete expenses, top-ups and transfers (synced as CRDT tombstones)
- 📈 **Visual Analytics**: View your spending with pie charts, bar charts, and line charts
- 🖥️ **Terminal UI**: Beautiful TUI built with Ratatui
- 🖱️ **Mouse Support**: Click on tabs to switch between views
- 📱 **Responsive Design**: Adapts to mobile-like narrow terminal resolutions
- 💾 **CRDT Storage**: Persistent local storage as content-addressed RDX chunks, with optional end-to-end-encrypted S3 sync

## Installation

### From crates.io

```bash
cargo install money_manager
```

### From Source

```bash
git clone https://gitlab.com/your-username/money_manager.git
cd money_manager
cargo build --release
```

### Pre-built Binaries

Download pre-built binaries from the [Releases](https://gitlab.com/your-username/money_manager/-/releases) page.

Available platforms:
- Linux (AMD64/x86_64)
- Linux (ARM64/aarch64)
- Windows (AMD64/x86_64)

## Usage

```bash
money_manager
```

### Command Line Options

| Option | Description |
|--------|-------------|
| `-d, --data <PATH>` | Path to the RDX chunk directory (default: `money_manager.chunks` in current directory) |
| `-c, --config <PATH>` | Path to the config file (default: `money_manager.toml`) |
| `-h, --help` | Print help information |
| `-V, --version` | Print version information |

**Subcommands:**

| Command | Description |
|---------|-------------|
| `sync` | Synchronize the local data with the configured S3 remote |
| `keygen` | Print a fresh base64 encryption key for `money_manager.toml` |
| `config --template` | Print a commented config template (redirect into `money_manager.toml`) |
| `config --export` | Print the `[remote]` config as a single shareable string |
| `config --import <STRING>` | Parse a shareable string and write it into the config file |
| `import-csv --table <T> <FILE>` | Import records from a CSV file mirroring the original SQL table |

**Examples:**

```bash
# Use default data file in current directory
money_manager

# Use a specific data file
money_manager --data ~/finances/my_budget.chunks

# Sync with the configured S3 remote
money_manager sync

# Generate a starter config to fill in
money_manager config --template > money_manager.toml

# Share the S3 config (incl. secrets) as one string — paste into another
# machine or the Obsidian plugin (format is interchangeable)
money_manager config --export
money_manager config --import "tresse1:..."

# Import data from CSV files that mirror the original SQL tables
money_manager import-csv --table categories categories.csv
money_manager import-csv --table expenses  expenses.csv
```

### CSV import

`import-csv` loads records from a CSV file whose columns mirror the original
SQLite schema, **identifiers included**, so identity is preserved across the
import (re-importing the same file is idempotent, and foreign keys keep
pointing at the same rows). Columns are matched by header name, so their order
is free; the file must have a header row.

| `--table` value | Columns |
|-----------------|---------|
| `categories` | `id,name` |
| `top-up-categories` | `id,name` |
| `expenses` | `id,category_id,amount,comment,date[,account_id]` |
| `top-ups` | `id,category_id,amount,comment,date[,account_id]` |
| `accounts` | `id,name,currency,opening_balance` |
| `transfers` | `id,from_account_id,to_account_id,amount_from,amount_to,comment,date` |

- `id` and the `*_id` references are UUIDs (canonical `xxxxxxxx-…` or 32-char hex).
- A blank `id` cell is filled with a fresh UUID; a blank `comment` becomes empty.
- `date` is `YYYY-MM-DD`.
- `account_id` is optional: without the column (or with a blank cell) a record
  goes to the default account, whose id is the nil UUID
  `00000000-0000-0000-0000-000000000000`.
- `currency` is a 3-letter ISO code (`RUB`, `USD`, …); a blank
  `opening_balance` means 0.
- Transfers need two different accounts and positive amounts, each in its own
  account's currency (`amount_from` is debited, `amount_to` credited).
- Import categories and accounts before the records that reference them.

```csv
id,category_id,amount,comment,date
11111111-1111-4111-8111-111111111111,22222222-2222-4222-8222-222222222222,45.50,"Lunch, with tax",2024-12-01
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `0-9` | Switch between tabs |
| `Tab` | Switch to next tab |
| `n` | Create new entry |
| `↑/↓` | Select a row / navigate list |
| `←/→` | Sort columns / Navigate months / Scroll charts |
| `[` / `]` | Previous / next account |
| `t` | New transfer (from any tab) |
| `d` / `Delete` | Delete the selected expense, top-up or transfer (asks for confirmation) |
| `e` / `Enter` | Accounts tab: edit / make the selected account current |
| `x` | Pie charts: hide/show transfers |
| `r` | Refresh data |
| `s` | Sync with the S3 remote |
| `q` | Quit |

### Mouse Support

| Action | Effect |
|--------|--------|
| Click on tab | Switch to that tab |
| Click on a table row | Select it |
| Scroll wheel | Scroll through lists |
| Click on table header | Sort by that column |
| Click category (Line Chart) | Toggle category selection |

### Tabs

0. **Accounts**: All accounts with their currency, income, spending and balance
1. **Expense Categories**: Manage expense categories
2. **Expenses**: View and add expenses
3. **Top-Up Categories**: Manage income categories
4. **Top-Ups**: View and add income
5. **Expense Pie Chart**: Expense breakdown by category
6. **Top-Up Pie Chart**: Income breakdown by category
7. **Expense Bar Chart**: Monthly expense overview
8. **Top-Up Bar Chart**: Monthly income overview
9. **Line Chart**: Expense trends over time
- **⇄ Transfers**: All transfers between accounts, with the exchange rate

Tabs 2 and 4–9 show the **current account** only; its name and balance are in
the tab bar, and `[` / `]` switch accounts.

### Accounts and transfers

- Every account has a name, a currency and an opening balance. Categories are
  shared by all accounts. Existing data lives in the default account
  ("Основной", currency from `default_currency` in the config, `RUB` if unset).
- Balance = opening balance + top-ups + incoming transfers − expenses −
  outgoing transfers. Different currencies are never added together.
- A transfer records how much left account A (in A's currency) and how much
  arrived on account B (in B's currency); the rate is derived from the two
  amounts. It shows up as an expense in the "⇄ Перевод" category on A and as a
  top-up in the same category on B. Deleting either side deletes the whole
  transfer.
- The last selected account is remembered in `last_account` in
  `money_manager.toml` (local only, not synced).

**Compatibility with 0.3.x.** Older versions still open and sync the data: they
see every expense and top-up as if it belonged to a single account and do not
see transfers or accounts (so their totals ignore transfers). An older version
that *overwrites* an existing record — only possible through `import-csv` with
the same ids — moves that record back to the default account.

## Responsive Design

The application automatically adapts to different terminal sizes:

| Width | Mode | Description |
|-------|------|-------------|
| < 60 cols | **Mobile** | Compact tabs, abbreviated labels, essential columns only |
| 60-80 cols | **Medium** | Moderate abbreviations, balanced layout |
| > 80 cols | **Wide** | Full labels and all columns displayed |

This makes the app usable on narrow terminals, mobile terminal emulators, or split-screen setups.

## Storage & Sync

Data lives in a directory (`money_manager.chunks` by default, created on first
run), built on the [`rdx-sync`](https://gitlab.com/ragusseven/tresse) CRDT chunk
store (`rdx-sync-fs` backend). A **chunk is the delta of one sync**, not the
whole document and not one record:

- Local edits accumulate in a small mutable `staging.rdx` (a delta holding only
  the records changed since the last sync), rewritten on every write so a crash
  loses nothing.
- On sync the staged delta is *sealed* into a single immutable, content-addressed
  chunk (a file named by the blake3 hash of its encoding), then exchanged with
  the remote.
- Merging the sealed chunks plus staging reconstructs the full document.

So the chunk count grows with *syncs-that-had-changes*, not with the number of
records or writes. Storage is content-addressed and idempotent; sync is
**incremental** (only chunks a side is missing are transferred) and every
fetched chunk is **hash-verified** before it is trusted. A legacy
`money_manager.rdx` blob, if present, is migrated in as one baseline chunk on
first run.

Optionally, the data can be synchronized to any S3-compatible bucket via
`rdx-sync-s3`. Configure a `[remote]` section in `money_manager.toml`:

```toml
[remote]
endpoint = "https://s3.us-east-1.amazonaws.com"
region = "us-east-1"
bucket = "my-bucket"
access_key_id = "AKID..."
secret_access_key = "..."
prefix = "money-manager"
encryption_key = "base64-32-bytes"  # optional E2E key; run `money_manager keygen`
```

Then run `money_manager sync` (or press `s` in the TUI). A sync stages the local
state as a chunk, fetches the remote chunks, pushes the local one, merges
everything, and reloads the views — because the underlying format is a CRDT,
syncing multiple machines reconciles concurrent edits automatically.

Instead of editing the `[remote]` table by hand, you can move the whole config
(including the secret and encryption keys) as a single `tresse1:` string with
`config --export` / `config --import`. The format is identical to the Obsidian
plugin's "copy settings" string, so the same bucket can be shared between the
plugin and this app.

## Development

### Prerequisites

- Rust 1.85 or later (edition 2024)

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

## License

See [LICENSE](LICENSE) for details.

