-- Your SQL goes here
CREATE TABLE expenses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    category_id INTEGER NOT NULL,
    amount REAL NOT NULL,
    comment TEXT,
    date TEXT NOT NULL,
    FOREIGN KEY (category_id) REFERENCES categories (id)
);
