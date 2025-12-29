use diesel::prelude::*;
use crate::models::Expense;
use crate::schema;
use uuid::Uuid;
use chrono::NaiveDate;

impl Expense {
    pub fn create(conn: &mut SqliteConnection, category_id: &[u8], amount: f64, comment: Option<&str>, date: NaiveDate) -> QueryResult<Expense> {
        use schema::expenses;
        let id = Uuid::new_v4().as_bytes().to_vec();
        let new_expense = (
            expenses::id.eq(&id),
            expenses::category_id.eq(category_id),
            expenses::amount.eq(amount),
            expenses::comment.eq(comment),
            expenses::date.eq(date),
        );
        diesel::insert_into(expenses::table)
            .values(new_expense)
            .execute(conn)?;
        expenses::table.filter(expenses::id.eq(&id)).first(conn)
    }

    pub fn read_all(conn: &mut SqliteConnection) -> QueryResult<Vec<Expense>> {
        use schema::expenses::dsl::*;
        expenses.load::<Expense>(conn)
    }

    pub fn update(conn: &mut SqliteConnection, expense_id: &[u8], new_amount: f64, new_comment: Option<&str>, new_date: NaiveDate) -> QueryResult<usize> {
        use schema::expenses::dsl::*;
        diesel::update(expenses.filter(id.eq(expense_id)))
            .set((amount.eq(new_amount), comment.eq(new_comment), date.eq(new_date)))
            .execute(conn)
    }

    pub fn delete(conn: &mut SqliteConnection, expense_id: &[u8]) -> QueryResult<usize> {
        use schema::expenses::dsl::*;
        diesel::delete(expenses.filter(id.eq(expense_id))).execute(conn)
    }
}

