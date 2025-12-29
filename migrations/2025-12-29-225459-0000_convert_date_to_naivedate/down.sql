-- This file should undo anything in `up.sql`
-- Migration rollback: Remove date validation constraints

-- Recreate expenses table without date CHECK constraint
CREATE TABLE expenses_old (
    id BLOB PRIMARY KEY NOT NULL,
    category_id BLOB NOT NULL,
    amount REAL NOT NULL,
    comment TEXT,
    date TEXT NOT NULL,
    FOREIGN KEY (category_id) REFERENCES categories (id)
);

-- Copy data back
INSERT INTO expenses_old (id, category_id, amount, comment, date)
SELECT id, category_id, amount, comment, date FROM expenses;

-- Drop new table and rename old one
DROP TABLE expenses;
ALTER TABLE expenses_old RENAME TO expenses;

-- Recreate top_ups table without date CHECK constraint
CREATE TABLE top_ups_old (
    id BLOB PRIMARY KEY NOT NULL,
    category_id BLOB NOT NULL,
    amount REAL NOT NULL,
    comment TEXT,
    date TEXT NOT NULL,
    FOREIGN KEY (category_id) REFERENCES top_up_categories (id)
);

-- Copy data back
INSERT INTO top_ups_old (id, category_id, amount, comment, date)
SELECT id, category_id, amount, comment, date FROM top_ups;

-- Drop new table and rename old one
DROP TABLE top_ups;
ALTER TABLE top_ups_old RENAME TO top_ups;

