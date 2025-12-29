use diesel::prelude::*;
use crate::models::TopUpCategory;
use crate::schema;
use uuid::Uuid;

impl TopUpCategory {
    pub fn create(conn: &mut SqliteConnection, name: &str) -> QueryResult<TopUpCategory> {
        use schema::top_up_categories;
        let id = Uuid::new_v4().as_bytes().to_vec();
        let new_category = (
            top_up_categories::id.eq(&id),
            top_up_categories::name.eq(name),
        );
        diesel::insert_into(top_up_categories::table)
            .values(new_category)
            .execute(conn)?;
        top_up_categories::table.filter(top_up_categories::id.eq(&id)).first(conn)
    }

    pub fn read_all(conn: &mut SqliteConnection) -> QueryResult<Vec<TopUpCategory>> {
        use schema::top_up_categories::dsl::*;
        top_up_categories.load::<TopUpCategory>(conn)
    }

    pub fn update(conn: &mut SqliteConnection, category_id: &[u8], new_name: &str) -> QueryResult<usize> {
        use schema::top_up_categories::dsl::*;
        diesel::update(top_up_categories.filter(id.eq(category_id)))
            .set(name.eq(new_name))
            .execute(conn)
    }

    pub fn delete(conn: &mut SqliteConnection, category_id: &[u8]) -> QueryResult<usize> {
        use schema::top_up_categories::dsl::*;
        diesel::delete(top_up_categories.filter(id.eq(category_id))).execute(conn)
    }
}

