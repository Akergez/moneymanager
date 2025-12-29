-- Your SQL goes here
CREATE TABLE top_ups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    category_id INTEGER NOT NULL,
    amount REAL NOT NULL,
    comment TEXT,
    date TEXT NOT NULL,
    FOREIGN KEY (category_id) REFERENCES top_up_categories (id)
);
