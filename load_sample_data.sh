#!/bin/bash

# Script to initialize the database with sample data

DB_FILE="money_manager.db"

echo "Loading sample data into $DB_FILE..."

if [ ! -f "$DB_FILE" ]; then
    echo "Error: Database file $DB_FILE not found!"
    echo "Please run 'diesel migration run' first."
    exit 1
fi

sqlite3 "$DB_FILE" < sample_data.sql

echo "Sample data loaded successfully!"
echo "You can now run the application with: cargo run --release"

