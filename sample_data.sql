-- Sample data for testing the Money Manager TUI

-- Insert expense categories
INSERT INTO categories (name) VALUES ('Food & Dining');
INSERT INTO categories (name) VALUES ('Transportation');
INSERT INTO categories (name) VALUES ('Entertainment');
INSERT INTO categories (name) VALUES ('Utilities');
INSERT INTO categories (name) VALUES ('Healthcare');
INSERT INTO categories (name) VALUES ('Shopping');

-- Insert top-up categories
INSERT INTO top_up_categories (name) VALUES ('Salary');
INSERT INTO top_up_categories (name) VALUES ('Freelance');
INSERT INTO top_up_categories (name) VALUES ('Investment Returns');
INSERT INTO top_up_categories (name) VALUES ('Gifts');

-- Insert sample expenses
INSERT INTO expenses (category_id, amount, comment, date) VALUES (1, 45.50, 'Lunch at restaurant', '2024-12-01');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (1, 120.75, 'Grocery shopping', '2024-12-05');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (2, 50.00, 'Gas station', '2024-12-03');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (3, 25.99, 'Movie tickets', '2024-12-10');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (4, 85.00, 'Electricity bill', '2024-12-15');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (1, 35.20, 'Coffee shop', '2024-12-20');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (2, 15.00, 'Parking', '2024-12-18');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (5, 150.00, 'Doctor visit', '2024-12-12');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (6, 89.99, 'New shoes', '2024-12-22');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (1, 65.30, 'Dinner with friends', '2024-12-25');

-- Some expenses from previous months for bar chart
INSERT INTO expenses (category_id, amount, comment, date) VALUES (1, 250.00, 'Monthly groceries', '2024-11-15');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (2, 120.00, 'Gas', '2024-11-10');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (4, 85.00, 'Utilities', '2024-11-01');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (1, 300.00, 'Monthly groceries', '2024-10-15');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (2, 100.00, 'Gas', '2024-10-10');
INSERT INTO expenses (category_id, amount, comment, date) VALUES (3, 75.00, 'Entertainment', '2024-10-20');

-- Insert sample top-ups
INSERT INTO top_ups (category_id, amount, comment, date) VALUES (1, 3500.00, 'Monthly salary', '2024-12-01');
INSERT INTO top_ups (category_id, amount, comment, date) VALUES (2, 500.00, 'Website project', '2024-12-15');
INSERT INTO top_ups (category_id, amount, comment, date) VALUES (3, 125.50, 'Dividend payment', '2024-12-10');
INSERT INTO top_ups (category_id, amount, comment, date) VALUES (1, 3500.00, 'Monthly salary', '2024-11-01');
INSERT INTO top_ups (category_id, amount, comment, date) VALUES (2, 750.00, 'Consulting work', '2024-11-20');
INSERT INTO top_ups (category_id, amount, comment, date) VALUES (4, 100.00, 'Birthday gift', '2024-12-28');

