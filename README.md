# Money Manager

A terminal-based money management application built with Rust, featuring expense tracking, income (top-ups) tracking, and visual analytics.

## Features

- 📊 **Expense Tracking**: Track your expenses with customizable categories
- 💰 **Income Management**: Record income/top-ups with categories
- 📈 **Visual Analytics**: View your spending with pie charts, bar charts, and line charts
- 🖥️ **Terminal UI**: Beautiful TUI built with Ratatui
- 🖱️ **Mouse Support**: Click on tabs to switch between views
- 📱 **Responsive Design**: Adapts to mobile-like narrow terminal resolutions
- 💾 **SQLite Database**: Persistent storage with SQLite

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
| `-d, --database <PATH>` | Path to the SQLite database file (default: `money_manager.db` in current directory) |
| `-h, --help` | Print help information |
| `-V, --version` | Print version information |

**Examples:**

```bash
# Use default database in current directory
money_manager

# Use a specific database file
money_manager --database ~/finances/my_budget.db

# Short form
money_manager -d /path/to/custom.db
```

### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `1-7` | Switch between tabs |
| `Tab` | Switch to next tab |
| `n` | Create new entry |
| `↑/↓` | Navigate list |
| `←/→` | Sort columns / Navigate months |
| `r` | Refresh data |
| `q` | Quit |

### Mouse Support

| Action | Effect |
|--------|--------|
| Click on tab | Switch to that tab |
| Scroll wheel | Scroll through lists |
| Click on table header | Sort by that column |
| Click category (Line Chart) | Toggle category selection |

### Tabs

1. **Expense Categories**: Manage expense categories
2. **Expenses**: View and add expenses
3. **Top-Up Categories**: Manage income categories
4. **Top-Ups**: View and add income
5. **Pie Chart**: Expense breakdown by category
6. **Bar Chart**: Monthly expense overview
7. **Line Chart**: Expense trends over time

## Responsive Design

The application automatically adapts to different terminal sizes:

| Width | Mode | Description |
|-------|------|-------------|
| < 60 cols | **Mobile** | Compact tabs, abbreviated labels, essential columns only |
| 60-80 cols | **Medium** | Moderate abbreviations, balanced layout |
| > 80 cols | **Wide** | Full labels and all columns displayed |

This makes the app usable on narrow terminals, mobile terminal emulators, or split-screen setups.

## Requirements

- SQLite 3.x

## Database

The application uses SQLite for data persistence. By default, it looks for `money_manager.db` in the current working directory. If the database file doesn't exist, it will be created automatically with all necessary tables.

You can specify a custom database location using the `--database` option.

## Development

### Prerequisites

- Rust 1.70 or later
- SQLite development libraries

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

