#!/bin/bash

# Simple startup script for Money Manager TUI

cd "$(dirname "$0")"

# Check if database exists and has tables
if ! sqlite3 money_manager.db "SELECT name FROM sqlite_master WHERE type='table' AND name='categories';" | grep -q categories; then
    echo "Database tables not found. Creating tables..."
    for sql in migrations/*/up.sql; do
        sqlite3 money_manager.db < "$sql" 2>/dev/null || true
    done

    echo "Loading sample data..."
    sqlite3 money_manager.db < sample_data.sql
fi

# Set DATABASE_URL for the app
export DATABASE_URL=./money_manager.db

# Run the TUI
./target/release/money_manager 2>/dev/null || {
    echo "Release build not found. Building..."
    cargo build --release
    ./target/release/money_manager
}

