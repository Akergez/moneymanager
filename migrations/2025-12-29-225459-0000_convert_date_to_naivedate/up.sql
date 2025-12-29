-- Migration: Convert date TEXT columns to proper DATE format with validation
--
-- SQLite doesn't have a true DATE type, but we can:
-- 1. Ensure all dates are stored in ISO 8601 format (YYYY-MM-DD)
-- 2. Add CHECK constraints to validate date format
-- 3. Update schema metadata for Diesel to treat as Date type

-- Recreate expenses table with DATE column and CHECK constraint
CREATE TABLE expenses_new (
    id BLOB PRIMARY KEY NOT NULL,
    category_id BLOB NOT NULL,
    amount REAL NOT NULL,
    comment TEXT,
    date DATE NOT NULL CHECK(date IS date(date)),
    FOREIGN KEY (category_id) REFERENCES categories (id)
);

-- Copy data from old table (will fail if any date is invalid)
INSERT INTO expenses_new (id, category_id, amount, comment, date)
SELECT id, category_id, amount, comment, date FROM expenses;

-- Drop old table and rename new one
DROP TABLE expenses;
ALTER TABLE expenses_new RENAME TO expenses;

-- Recreate top_ups table with DATE column and CHECK constraint
CREATE TABLE top_ups_new (
    id BLOB PRIMARY KEY NOT NULL,
    category_id BLOB NOT NULL,
    amount REAL NOT NULL,
    comment TEXT,
    date DATE NOT NULL CHECK(date IS date(date)),
    FOREIGN KEY (category_id) REFERENCES top_up_categories (id)
);

-- Copy data from old table (will fail if any date is invalid)
INSERT INTO top_ups_new (id, category_id, amount, comment, date)
SELECT id, category_id, amount, comment, date FROM top_ups;

-- Drop old table and rename new one
DROP TABLE top_ups;
ALTER TABLE top_ups_new RENAME TO top_ups;

