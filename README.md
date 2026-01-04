# Money Manager

A terminal-based money management application built with Rust, featuring expense tracking, income (top-ups) tracking, and visual analytics.

## Features

- 📊 **Expense Tracking**: Track your expenses with customizable categories
- 💰 **Income Management**: Record income/top-ups with categories
- 📈 **Visual Analytics**: View your spending with pie charts, bar charts, and line charts
- 🖥️ **Terminal UI**: Beautiful TUI built with Ratatui
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

### Tabs

1. **Expense Categories**: Manage expense categories
2. **Expenses**: View and add expenses
3. **Top-Up Categories**: Manage income categories
4. **Top-Ups**: View and add income
5. **Pie Chart**: Expense breakdown by category
6. **Bar Chart**: Monthly expense overview
7. **Line Chart**: Expense trends over time

## Requirements

- SQLite 3.x

## Database

The application uses SQLite for data persistence. On first run, it will create a `money_manager.db` file in the current directory.

### Running Migrations

```bash
diesel migration run
```

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

MIT License - see [LICENSE](LICENSE) for details.

