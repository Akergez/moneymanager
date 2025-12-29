-- Your SQL goes here
-- SQLite doesn't support ALTER COLUMN type, so we need to recreate tables

-- 1. Create new tables with UUID primary keys stored as BLOB
CREATE TABLE categories_new (
    id BLOB PRIMARY KEY NOT NULL,
    name TEXT NOT NULL
);

CREATE TABLE top_up_categories_new (
    id BLOB PRIMARY KEY NOT NULL,
    name TEXT NOT NULL
);

CREATE TABLE expenses_new (
    id BLOB PRIMARY KEY NOT NULL,
    category_id BLOB NOT NULL,
    amount REAL NOT NULL,
    comment TEXT,
    date TEXT NOT NULL,
    FOREIGN KEY (category_id) REFERENCES categories_new (id)
);

CREATE TABLE top_ups_new (
    id BLOB PRIMARY KEY NOT NULL,
    category_id BLOB NOT NULL,
    amount REAL NOT NULL,
    comment TEXT,
    date TEXT NOT NULL,
    FOREIGN KEY (category_id) REFERENCES top_up_categories_new (id)
);

-- 2. Drop old tables
DROP TABLE IF EXISTS expenses;
DROP TABLE IF EXISTS top_ups;
DROP TABLE IF EXISTS categories;
DROP TABLE IF EXISTS top_up_categories;

-- 3. Rename new tables
ALTER TABLE categories_new RENAME TO categories;
ALTER TABLE top_up_categories_new RENAME TO top_up_categories;
ALTER TABLE expenses_new RENAME TO expenses;
ALTER TABLE top_ups_new RENAME TO top_ups;

