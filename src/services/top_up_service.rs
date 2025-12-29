use diesel::prelude::*;
use crate::models::TopUp;
use crate::schema;
use uuid::Uuid;
use chrono::NaiveDate;

impl TopUp {
    pub fn create(conn: &mut SqliteConnection, category_id: &[u8], amount: f64, comment: Option<&str>, date: NaiveDate) -> QueryResult<TopUp> {
        use schema::top_ups;
        let id = Uuid::new_v4().as_bytes().to_vec();
        let new_top_up = (
            top_ups::id.eq(&id),
            top_ups::category_id.eq(category_id),
            top_ups::amount.eq(amount),
            top_ups::comment.eq(comment),
            top_ups::date.eq(date),
        );
        diesel::insert_into(top_ups::table)
            .values(new_top_up)
            .execute(conn)?;
        top_ups::table.filter(top_ups::id.eq(&id)).first(conn)
    }

    pub fn read_all(conn: &mut SqliteConnection) -> QueryResult<Vec<TopUp>> {
        use schema::top_ups::dsl::*;
        top_ups.load::<TopUp>(conn)
    }

    pub fn update(conn: &mut SqliteConnection, top_up_id: &[u8], new_amount: f64, new_comment: Option<&str>, new_date: NaiveDate) -> QueryResult<usize> {
        use schema::top_ups::dsl::*;
        diesel::update(top_ups.filter(id.eq(top_up_id)))
            .set((amount.eq(new_amount), comment.eq(new_comment), date.eq(new_date)))
            .execute(conn)
    }

    pub fn delete(conn: &mut SqliteConnection, top_up_id: &[u8]) -> QueryResult<usize> {
        use schema::top_ups::dsl::*;
        diesel::delete(top_ups.filter(id.eq(top_up_id))).execute(conn)
    }
}

