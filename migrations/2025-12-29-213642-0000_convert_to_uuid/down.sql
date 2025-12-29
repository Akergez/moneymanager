-- This file should undo anything in `up.sql`
-- Reverting to INTEGER primary keys

CREATE TABLE categories_old (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL
);

CREATE TABLE top_up_categories_old (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL
);

CREATE TABLE expenses_old (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    category_id INTEGER NOT NULL,
    amount REAL NOT NULL,
    comment TEXT,
    date TEXT NOT NULL,
    FOREIGN KEY (category_id) REFERENCES categories_old (id)
);

CREATE TABLE top_ups_old (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    category_id INTEGER NOT NULL,
    amount REAL NOT NULL,
    comment TEXT,
    date TEXT NOT NULL,
    FOREIGN KEY (category_id) REFERENCES top_up_categories_old (id)
);

DROP TABLE IF EXISTS expenses;
DROP TABLE IF EXISTS top_ups;
DROP TABLE IF EXISTS categories;
DROP TABLE IF EXISTS top_up_categories;

ALTER TABLE categories_old RENAME TO categories;
ALTER TABLE top_up_categories_old RENAME TO top_up_categories;
ALTER TABLE expenses_old RENAME TO expenses;
ALTER TABLE top_ups_old RENAME TO top_ups;

